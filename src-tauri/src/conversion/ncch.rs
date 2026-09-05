use std::io::{Read, Seek, SeekFrom};

use sha2::{Digest, Sha256};

use super::{
    ConversionError,
    cci::{EncryptionMode, Partition},
};

const HEADER_SIZE: usize = 0x200;
const EXTHEADER_SIZE: usize = 0x400;
const SMDH_SIZE: usize = 0x36c0;
const MEDIA_UNIT: u64 = 0x200;

#[derive(Debug, Clone)]
pub struct PreparedGame {
    pub ncch_header: [u8; HEADER_SIZE],
    pub extheader: [u8; EXTHEADER_SIZE],
    pub dependency_list: [u8; 0x180],
    pub save_size: [u8; 4],
    pub smdh: [u8; SMDH_SIZE],
}

impl PreparedGame {
    pub fn load<R: Read + Seek>(
        reader: &mut R,
        game: Partition,
        encryption: EncryptionMode,
        path: &str,
    ) -> Result<Self, ConversionError> {
        if encryption != EncryptionMode::Unencrypted {
            return Err(ConversionError::UnsupportedEncryption {
                path: path.into(),
                mode: encryption,
            });
        }
        let mut ncch_header = [0; HEADER_SIZE];
        seek(reader, game.offset, path)?;
        read(reader, &mut ncch_header, path)?;
        let mut extheader = [0; EXTHEADER_SIZE];
        read(reader, &mut extheader, path)?;
        let expected: [u8; 32] = ncch_header[0x160..0x180].try_into().unwrap();
        if Sha256::digest(extheader).as_slice() != expected {
            return Err(ConversionError::InvalidExtHeaderHash { path: path.into() });
        }
        extheader[0x0d] |= 0x02;
        ncch_header[0x160..0x180].copy_from_slice(&Sha256::digest(extheader));
        let dependency_list: [u8; 0x180] = extheader[0x40..0x1c0].try_into().unwrap();
        let save_size: [u8; 4] = extheader[0x1c0..0x1c4].try_into().unwrap();
        let exefs_offset =
            u32::from_le_bytes(ncch_header[0x1a0..0x1a4].try_into().unwrap()) as u64 * MEDIA_UNIT;
        let smdh = read_smdh(reader, game, exefs_offset, path)?;
        Ok(Self {
            ncch_header,
            extheader,
            dependency_list,
            save_size,
            smdh,
        })
    }
}

fn read_smdh<R: Read + Seek>(
    reader: &mut R,
    game: Partition,
    exefs_offset: u64,
    path: &str,
) -> Result<[u8; SMDH_SIZE], ConversionError> {
    let mut exefs = [0; 0x200];
    seek(reader, game.offset + exefs_offset, path)?;
    read(reader, &mut exefs, path)?;
    for entry in exefs.chunks_exact(0x10) {
        if entry[..8]
            .iter()
            .take_while(|&&byte| byte != 0)
            .copied()
            .eq(b"icon".iter().copied())
        {
            let icon_offset = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
            let mut smdh = [0; SMDH_SIZE];
            seek(
                reader,
                game.offset + exefs_offset + 0x200 + icon_offset,
                path,
            )?;
            read(reader, &mut smdh, path)?;
            return Ok(smdh);
        }
    }
    Err(ConversionError::MissingIcon { path: path.into() })
}

fn seek<R: Seek>(reader: &mut R, offset: u64, path: &str) -> Result<(), ConversionError> {
    reader
        .seek(SeekFrom::Start(offset))
        .map(|_| ())
        .map_err(|source| ConversionError::Io {
            path: path.into(),
            source,
        })
}
fn read<R: Read>(reader: &mut R, buffer: &mut [u8], path: &str) -> Result<(), ConversionError> {
    reader
        .read_exact(buffer)
        .map_err(|source| ConversionError::Io {
            path: path.into(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture() -> Vec<u8> {
        let mut data = vec![0; 0x6000];
        let game = 0x400usize;
        data[game + 0x100..game + 0x104].copy_from_slice(b"NCCH");
        let mut extheader = [0; EXTHEADER_SIZE];
        extheader[0x40] = 0x77;
        extheader[0x1c0..0x1c4].copy_from_slice(&0x10203040u32.to_le_bytes());
        data[game + 0x200..game + 0x600].copy_from_slice(&extheader);
        data[game + 0x160..game + 0x180].copy_from_slice(&Sha256::digest(extheader));
        data[game + 0x1a0..game + 0x1a4].copy_from_slice(&4u32.to_le_bytes());
        let exefs = game + 0x800;
        data[exefs..exefs + 4].copy_from_slice(b"icon");
        data[exefs + 8..exefs + 12].copy_from_slice(&0u32.to_le_bytes());
        data[exefs + 0x200..exefs + 0x200 + SMDH_SIZE].fill(0x5a);
        data
    }

    #[test]
    fn validates_patches_and_reads_unencrypted_game_metadata() {
        let data = fixture();
        let prepared = PreparedGame::load(
            &mut Cursor::new(&data),
            Partition {
                offset: 0x400,
                size: 0x5000,
            },
            EncryptionMode::Unencrypted,
            "fixture.3ds",
        )
        .unwrap();
        assert_eq!(prepared.extheader[0x0d] & 0x02, 0x02);
        assert_eq!(prepared.dependency_list[0], 0x77);
        assert_eq!(prepared.save_size, 0x10203040u32.to_le_bytes());
        assert_eq!(prepared.smdh[0], 0x5a);
    }
}
