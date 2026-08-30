use std::io::{Read, Seek, SeekFrom};

use super::ConversionError;

const MEDIA_UNIT: u64 = 0x200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMode { Unencrypted, ZeroKey, OriginalNcch }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partition { pub offset: u64, pub size: u64 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CciHeader {
    pub title_id: u64,
    pub game: Partition,
    pub manual: Option<Partition>,
    pub download_play_child: Option<Partition>,
    pub encryption: EncryptionMode,
}

impl CciHeader {
    pub fn parse<R: Read + Seek>(reader: &mut R, path: &str, file_len: u64) -> Result<Self, ConversionError> {
        let mut magic = [0; 4];
        seek(reader, 0x100, path)?;
        read(reader, &mut magic, path)?;
        if &magic != b"NCSD" { return Err(ConversionError::InvalidMagic { path: path.into(), expected: "NCSD" }); }
        seek(reader, 0x108, path)?;
        let mut title = [0; 8];
        read(reader, &mut title, path)?;
        // NCSD stores the title ID little-endian.  CIA metadata uses its
        // canonical big-endian representation when it is written back out.
        let title_id = u64::from_le_bytes(title);
        seek(reader, 0x120, path)?;
        let game = partition(reader, path, "game", file_len, true)?.ok_or_else(|| ConversionError::MissingGamePartition { path: path.into() })?;
        let manual = partition(reader, path, "manual", file_len, false)?;
        let download_play_child = partition(reader, path, "Download Play child", file_len, false)?;
        seek(reader, game.offset + 0x100, path)?;
        read(reader, &mut magic, path)?;
        if &magic != b"NCCH" { return Err(ConversionError::InvalidMagic { path: path.into(), expected: "NCCH" }); }
        let mut flag = [0; 1];
        seek(reader, game.offset + 0x18f, path)?;
        read(reader, &mut flag, path)?;
        let encryption = if flag[0] & 0x04 != 0 { EncryptionMode::Unencrypted } else if flag[0] & 0x01 != 0 { EncryptionMode::ZeroKey } else { EncryptionMode::OriginalNcch };
        Ok(Self { title_id, game, manual, download_play_child, encryption })
    }
}

fn partition<R: Read>(reader: &mut R, path: &str, name: &'static str, file_len: u64, required: bool) -> Result<Option<Partition>, ConversionError> {
    let mut entry = [0; 8];
    read(reader, &mut entry, path)?;
    let offset = u32::from_le_bytes(entry[..4].try_into().unwrap()) as u64 * MEDIA_UNIT;
    let size = u32::from_le_bytes(entry[4..].try_into().unwrap()) as u64 * MEDIA_UNIT;
    if !required && offset == 0 && size == 0 { return Ok(None); }
    if offset == 0 || size < 0x200 || offset.checked_add(size).is_none_or(|end| end > file_len) { return Err(ConversionError::InvalidPartition { path: path.into(), name }); }
    Ok(Some(Partition { offset, size }))
}

fn seek<R: Seek>(reader: &mut R, offset: u64, path: &str) -> Result<(), ConversionError> { reader.seek(SeekFrom::Start(offset)).map(|_| ()).map_err(|source| ConversionError::Io { path: path.into(), source }) }
fn read<R: Read>(reader: &mut R, buffer: &mut [u8], path: &str) -> Result<(), ConversionError> { reader.read_exact(buffer).map_err(|source| ConversionError::Io { path: path.into(), source }) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture(flag: u8) -> Vec<u8> {
        let mut data = vec![0; 0x1000];
        data[0x100..0x104].copy_from_slice(b"NCSD");
        data[0x108..0x110].copy_from_slice(&0x1122334455667788u64.to_le_bytes());
        data[0x120..0x124].copy_from_slice(&2u32.to_le_bytes());
        data[0x124..0x128].copy_from_slice(&6u32.to_le_bytes());
        data[0x500..0x504].copy_from_slice(b"NCCH");
        data[0x58f] = flag;
        data
    }
    #[test]
    fn parses_an_unencrypted_game_partition() {
        let data = fixture(0x04);
        let parsed = CciHeader::parse(&mut Cursor::new(&data), "fixture.3ds", data.len() as u64).unwrap();
        assert_eq!(parsed.title_id, 0x1122334455667788);
        assert_eq!(parsed.game, Partition { offset: 0x400, size: 0xc00 });
        assert_eq!(parsed.encryption, EncryptionMode::Unencrypted);
    }
    #[test]
    fn rejects_a_missing_ncsd_magic() {
        let data = vec![0; 0x1000];
        assert!(matches!(CciHeader::parse(&mut Cursor::new(&data), "broken.3ds", data.len() as u64), Err(ConversionError::InvalidMagic { expected: "NCSD", .. })));
    }
}
