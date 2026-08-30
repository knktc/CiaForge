use std::{fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, path::Path};

use sha2::{Digest, Sha256};

use super::{writer::CONTENT_RECORDS_OFFSET, CciHeader, CiaHeader, CiaPlan, ConversionError, PreparedGame, ProgressSink, RetailTemplates};

pub fn convert_unencrypted<P: ProgressSink>(input: &Path, output: &Path, progress: &mut P) -> Result<(), ConversionError> {
    if output.exists() { return Err(ConversionError::AlreadyExists { path: output.display().to_string() }); }
    if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?; }
    let file = File::open(input).map_err(|source| io_error(input, source))?;
    let len = file.metadata().map_err(|source| io_error(input, source))?.len();
    let mut reader = std::io::BufReader::new(file);
    let header = CciHeader::parse(&mut reader, &input.display().to_string(), len)?;
    let game = PreparedGame::load(&mut reader, header.game, header.encryption, &input.display().to_string())?;
    let plan = CiaPlan::from_header(&header);
    let templates = RetailTemplates::load()?;
    let mut cia_header = CiaHeader::new(&plan, &templates.certificate_chain, &templates.ticket_and_tmd).map_err(ConversionError::Template)?;
    patch_metadata(&mut cia_header.0, &plan, &game);
    let temporary = output.with_extension(format!("{}.ciaforge-partial", output.extension().and_then(|value| value.to_str()).unwrap_or("cia")));
    if temporary.exists() { let _ = fs::remove_file(&temporary); }
    let result = write_file(&mut reader, &temporary, &plan, &game, cia_header.0, progress);
    if result.is_ok() { fs::rename(&temporary, output).map_err(|source| io_error(output, source))?; } else { let _ = fs::remove_file(&temporary); }
    result
}

fn write_file<R: Read + Seek, P: ProgressSink>(reader: &mut R, temporary: &Path, plan: &CiaPlan, game: &PreparedGame, header: Vec<u8>, progress: &mut P) -> Result<(), ConversionError> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(temporary).map_err(|source| io_error(temporary, source))?;
    output.write_all(&header).map_err(|source| io_error(temporary, source))?;
    let mut hashes = Vec::with_capacity(plan.contents.len());
    hashes.push(copy_game(reader, &mut output, plan.contents[0].partition, game, temporary, progress)?);
    for content in plan.contents.iter().skip(1) { hashes.push(copy_content(reader, &mut output, content.partition, temporary, progress)?); }
    let mut records = plan.content_records();
    for (index, hash) in hashes.iter().enumerate() { records[index * 0x30 + 0x10..index * 0x30 + 0x30].copy_from_slice(hash); }
    for (index, hash) in hashes.iter().enumerate() { output.seek(SeekFrom::Start(CONTENT_RECORDS_OFFSET as u64 + index as u64 * 0x30 + 0x10)).map_err(|source| io_error(temporary, source))?; output.write_all(hash).map_err(|source| io_error(temporary, source))?; }
    let record_hash = Sha256::digest(&records);
    output.seek(SeekFrom::Start(0x2fc7)).map_err(|source| io_error(temporary, source))?;
    output.write_all(&[plan.contents.len() as u8]).and_then(|_| output.write_all(&record_hash)).map_err(|source| io_error(temporary, source))?;
    let mut info = vec![0; 3]; info.push(plan.contents.len() as u8); info.extend_from_slice(&record_hash); info.resize(info.len() + 0x8dc, 0);
    output.seek(SeekFrom::Start(0x2fa4)).map_err(|source| io_error(temporary, source))?;
    output.write_all(&Sha256::digest(&info)).map_err(|source| io_error(temporary, source))?;
    output.seek(SeekFrom::End(0)).map_err(|source| io_error(temporary, source))?;
    output.write_all(&game.dependency_list).and_then(|_| output.write_all(&[0; 0x180])).and_then(|_| output.write_all(&2u32.to_le_bytes())).and_then(|_| output.write_all(&[0; 0xfc])).and_then(|_| output.write_all(&game.smdh)).map_err(|source| io_error(temporary, source))?;
    output.sync_all().map_err(|source| io_error(temporary, source))
}

fn copy_game<R: Read + Seek, W: Write, P: ProgressSink>(reader: &mut R, output: &mut W, partition: super::cci::Partition, game: &PreparedGame, path: &Path, progress: &mut P) -> Result<[u8; 32], ConversionError> {
    let mut hash = Sha256::new(); hash.update(game.ncch_header); hash.update(game.extheader);
    output.write_all(&game.ncch_header).and_then(|_| output.write_all(&game.extheader)).map_err(|source| io_error(path, source))?;
    copy_range(reader, output, partition.offset + 0x600, partition.size - 0x600, partition.size, 0x600, path, progress, &mut hash)?;
    Ok(hash.finalize().into())
}

fn copy_content<R: Read + Seek, W: Write, P: ProgressSink>(reader: &mut R, output: &mut W, partition: super::cci::Partition, path: &Path, progress: &mut P) -> Result<[u8; 32], ConversionError> {
    let mut hash = Sha256::new(); copy_range(reader, output, partition.offset, partition.size, partition.size, 0, path, progress, &mut hash)?; Ok(hash.finalize().into())
}

fn copy_range<R: Read + Seek, W: Write, P: ProgressSink>(reader: &mut R, output: &mut W, offset: u64, mut left: u64, total: u64, mut copied: u64, path: &Path, progress: &mut P, hash: &mut Sha256) -> Result<(), ConversionError> {
    reader.seek(SeekFrom::Start(offset)).map_err(|source| io_error(path, source))?;
    let mut buffer = vec![0; 1024 * 1024];
    while left > 0 { let take = buffer.len().min(left as usize); reader.read_exact(&mut buffer[..take]).map_err(|source| io_error(path, source))?; output.write_all(&buffer[..take]).map_err(|source| io_error(path, source))?; hash.update(&buffer[..take]); left -= take as u64; copied += take as u64; progress.report(copied, total); }
    Ok(())
}

fn patch_metadata(header: &mut [u8], plan: &CiaPlan, game: &PreparedGame) { header[0x2f9f] = plan.contents.len() as u8; header[0x2c1c..0x2c24].copy_from_slice(&plan.title_id.to_be_bytes()); header[0x2f4c..0x2f54].copy_from_slice(&plan.title_id.to_be_bytes()); header[0x2f5a..0x2f5e].copy_from_slice(&game.save_size); }
fn io_error(path: &Path, source: std::io::Error) -> ConversionError { ConversionError::Io { path: path.display().to_string(), source } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, time::{SystemTime, UNIX_EPOCH}};

    struct Recorder(Vec<(u64, u64)>);
    impl ProgressSink for Recorder { fn report(&mut self, completed: u64, total: u64) { self.0.push((completed, total)); } }

    fn fixture() -> Vec<u8> {
        let mut data = vec![0; 0x6000];
        let game = 0x400usize;
        data[0x100..0x104].copy_from_slice(b"NCSD");
        data[0x108..0x110].copy_from_slice(&0x1122334455667788u64.to_le_bytes());
        data[0x120..0x124].copy_from_slice(&2u32.to_le_bytes());
        data[0x124..0x128].copy_from_slice(&0x28u32.to_le_bytes());
        data[game + 0x100..game + 0x104].copy_from_slice(b"NCCH");
        data[game + 0x18f] = 0x04;
        let mut extheader = [0; 0x400];
        extheader[0x40] = 0x77;
        extheader[0x1c0..0x1c4].copy_from_slice(&0x10203040u32.to_le_bytes());
        data[game + 0x200..game + 0x600].copy_from_slice(&extheader);
        data[game + 0x160..game + 0x180].copy_from_slice(&Sha256::digest(extheader));
        data[game + 0x1a0..game + 0x1a4].copy_from_slice(&4u32.to_le_bytes());
        let exefs = game + 0x800;
        data[exefs..exefs + 4].copy_from_slice(b"icon");
        data[exefs + 0x200..exefs + 0x200 + 0x36c0].fill(0x5a);
        data
    }

    #[test]
    fn converts_an_unencrypted_cci_to_a_patched_cia() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("ciaforge-engine-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("sample.3ds");
        let output = root.join("nested/sample.cia");
        fs::write(&input, fixture()).unwrap();
        let mut progress = Recorder(Vec::new());
        convert_unencrypted(&input, &output, &mut progress).unwrap();
        let cia = fs::read(&output).unwrap();
        assert_eq!(&cia[..4], &0x2020u32.to_le_bytes());
        assert_eq!(&cia[0x2c1c..0x2c24], &0x1122334455667788u64.to_be_bytes());
        assert_eq!(cia[0x2f9f], 1);
        assert_eq!(&cia[cia.len() - 0x36c0..], &[0x5a; 0x36c0]);
        assert_eq!(progress.0.last(), Some(&(0x5000, 0x5000)));
        fs::remove_dir_all(root).unwrap();
    }
}
