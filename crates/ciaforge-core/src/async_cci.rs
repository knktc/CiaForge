use super::{
    ConversionError, RandomAccessInput,
    cci::{CciHeader, EncryptionMode, Partition},
};

const MEDIA_UNIT: u64 = 0x200;

/// Parses the CCI metadata through the asynchronous host-I/O seam.
pub async fn inspect<I: RandomAccessInput>(
    input: &mut I,
    input_name: &str,
    input_len: u64,
) -> Result<CciHeader, ConversionError> {
    let mut header = [0; 0x200];
    input
        .read_at(0, &mut header)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    if &header[0x100..0x104] != b"NCSD" {
        return Err(ConversionError::InvalidMagic {
            path: input_name.into(),
            expected: "NCSD",
        });
    }
    let title_id = u64::from_le_bytes(header[0x108..0x110].try_into().unwrap());
    let partition = |index: usize,
                     name: &'static str,
                     required: bool|
     -> Result<Option<Partition>, ConversionError> {
        let start = 0x120 + index * 8;
        let offset =
            u32::from_le_bytes(header[start..start + 4].try_into().unwrap()) as u64 * MEDIA_UNIT;
        let size = u32::from_le_bytes(header[start + 4..start + 8].try_into().unwrap()) as u64
            * MEDIA_UNIT;
        if !required && offset == 0 && size == 0 {
            return Ok(None);
        }
        if offset == 0
            || size < MEDIA_UNIT
            || offset.checked_add(size).is_none_or(|end| end > input_len)
        {
            return Err(ConversionError::InvalidPartition {
                path: input_name.into(),
                name,
            });
        }
        Ok(Some(Partition { offset, size }))
    };
    let game =
        partition(0, "game", true)?.ok_or_else(|| ConversionError::MissingGamePartition {
            path: input_name.into(),
        })?;
    let manual = partition(1, "manual", false)?;
    let download_play_child = partition(2, "Download Play child", false)?;
    let mut ncch = [0; 4];
    input
        .read_at(game.offset + 0x100, &mut ncch)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    if &ncch != b"NCCH" {
        return Err(ConversionError::InvalidMagic {
            path: input_name.into(),
            expected: "NCCH",
        });
    }
    let mut flag = [0; 1];
    input
        .read_at(game.offset + 0x18f, &mut flag)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    let encryption = if flag[0] & 0x04 != 0 {
        EncryptionMode::Unencrypted
    } else if flag[0] & 0x01 != 0 {
        EncryptionMode::ZeroKey
    } else {
        EncryptionMode::OriginalNcch
    };
    Ok(CciHeader {
        title_id,
        game,
        manual,
        download_play_child,
        encryption,
    })
}
