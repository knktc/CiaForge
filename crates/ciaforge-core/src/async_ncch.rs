use sha2::{Digest, Sha256};

use super::{
    ConversionError, RandomAccessInput,
    cci::{EncryptionMode, Partition},
    ncch::PreparedGame,
};

const HEADER_SIZE: usize = 0x200;
const EXTHEADER_SIZE: usize = 0x400;
const SMDH_SIZE: usize = 0x36c0;
const MEDIA_UNIT: u64 = 0x200;

pub async fn prepare<I: RandomAccessInput>(
    input: &mut I,
    game: Partition,
    encryption: EncryptionMode,
    input_name: &str,
) -> Result<PreparedGame, ConversionError> {
    if encryption != EncryptionMode::Unencrypted {
        return Err(ConversionError::UnsupportedEncryption {
            path: input_name.into(),
            mode: encryption,
        });
    }
    let mut ncch_header = [0; HEADER_SIZE];
    input
        .read_at(game.offset, &mut ncch_header)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    let mut extheader = [0; EXTHEADER_SIZE];
    input
        .read_at(game.offset + HEADER_SIZE as u64, &mut extheader)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    let expected: [u8; 32] = ncch_header[0x160..0x180].try_into().unwrap();
    if Sha256::digest(extheader).as_slice() != expected {
        return Err(ConversionError::InvalidExtHeaderHash {
            path: input_name.into(),
        });
    }
    extheader[0x0d] |= 0x02;
    ncch_header[0x160..0x180].copy_from_slice(&Sha256::digest(extheader));
    let dependency_list = extheader[0x40..0x1c0].try_into().unwrap();
    let save_size = extheader[0x1c0..0x1c4].try_into().unwrap();
    let exefs_offset =
        u32::from_le_bytes(ncch_header[0x1a0..0x1a4].try_into().unwrap()) as u64 * MEDIA_UNIT;
    let mut exefs = [0; 0x200];
    input
        .read_at(game.offset + exefs_offset, &mut exefs)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    for entry in exefs.chunks_exact(0x10) {
        if entry[..8]
            .iter()
            .take_while(|&&byte| byte != 0)
            .copied()
            .eq(b"icon".iter().copied())
        {
            let icon_offset = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
            let mut smdh = [0; SMDH_SIZE];
            input
                .read_at(game.offset + exefs_offset + 0x200 + icon_offset, &mut smdh)
                .await
                .map_err(|e| ConversionError::io(input_name, e))?;
            return Ok(PreparedGame {
                ncch_header,
                extheader,
                dependency_list,
                save_size,
                smdh,
            });
        }
    }
    Err(ConversionError::MissingIcon {
        path: input_name.into(),
    })
}
