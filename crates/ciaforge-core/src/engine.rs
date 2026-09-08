use std::io::{Read, Seek, SeekFrom, Write};

use sha2::{Digest, Sha256};

use super::{
    CciHeader, CiaHeader, CiaPlan, ConversionError, PreparedGame, ProgressSink, RetailTemplates,
    writer::CONTENT_RECORDS_OFFSET,
};

/// Converts a seekable input stream into a seekable CIA stream.
///
/// File paths, temporary files, and overwrite policy belong to the host.
pub fn convert<R: Read + Seek, W: Write + Seek, P: ProgressSink>(
    reader: &mut R,
    input_name: &str,
    input_len: u64,
    output: &mut W,
    progress: &mut P,
) -> Result<(), ConversionError> {
    let header = CciHeader::parse(reader, input_name, input_len)?;
    let game = PreparedGame::load(reader, header.game, header.encryption, input_name)?;
    let plan = CiaPlan::from_header(&header);
    let templates = RetailTemplates::load()?;
    let mut cia_header = CiaHeader::new(
        &plan,
        &templates.certificate_chain,
        &templates.ticket_and_tmd,
    )
    .map_err(ConversionError::Template)?;
    patch_metadata(&mut cia_header.0, &plan, &game);
    output
        .write_all(&cia_header.0)
        .map_err(|e| ConversionError::io(input_name, e))?;

    let mut hashes = Vec::with_capacity(plan.contents.len());
    hashes.push(copy_game(
        reader,
        output,
        plan.contents[0].partition,
        &game,
        input_name,
        progress,
    )?);
    for content in plan.contents.iter().skip(1) {
        hashes.push(copy_range(
            reader,
            output,
            content.partition.offset,
            content.partition.size,
            0,
            content.partition.size,
            input_name,
            progress,
            Sha256::new(),
        )?);
    }
    let mut records = plan.content_records();
    for (index, hash) in hashes.iter().enumerate() {
        records[index * 0x30 + 0x10..index * 0x30 + 0x30].copy_from_slice(hash);
        output
            .seek(SeekFrom::Start(
                CONTENT_RECORDS_OFFSET as u64 + index as u64 * 0x30 + 0x10,
            ))
            .and_then(|_| output.write_all(hash))
            .map_err(|e| ConversionError::io(input_name, e))?;
    }
    let record_hash = Sha256::digest(&records);
    output
        .seek(SeekFrom::Start(0x2fc7))
        .and_then(|_| output.write_all(&[plan.contents.len() as u8]))
        .and_then(|_| output.write_all(&record_hash))
        .map_err(|e| ConversionError::io(input_name, e))?;
    let mut info = vec![0; 3];
    info.push(plan.contents.len() as u8);
    info.extend_from_slice(&record_hash);
    info.resize(info.len() + 0x8dc, 0);
    output
        .seek(SeekFrom::Start(0x2fa4))
        .and_then(|_| output.write_all(&Sha256::digest(&info)))
        .map_err(|e| ConversionError::io(input_name, e))?;
    output
        .seek(SeekFrom::End(0))
        .and_then(|_| output.write_all(&game.dependency_list))
        .and_then(|_| output.write_all(&[0; 0x180]))
        .and_then(|_| output.write_all(&2u32.to_le_bytes()))
        .and_then(|_| output.write_all(&[0; 0xfc]))
        .and_then(|_| output.write_all(&game.smdh))
        .map_err(|e| ConversionError::io(input_name, e))
}

fn copy_game<R: Read + Seek, W: Write, P: ProgressSink>(
    reader: &mut R,
    output: &mut W,
    part: super::cci::Partition,
    game: &PreparedGame,
    name: &str,
    progress: &mut P,
) -> Result<[u8; 32], ConversionError> {
    let mut hash = Sha256::new();
    hash.update(game.ncch_header);
    hash.update(game.extheader);
    output
        .write_all(&game.ncch_header)
        .and_then(|_| output.write_all(&game.extheader))
        .map_err(|e| ConversionError::io(name, e))?;
    copy_range(
        reader,
        output,
        part.offset + 0x600,
        part.size - 0x600,
        0x600,
        part.size,
        name,
        progress,
        hash,
    )
}

fn copy_range<R: Read + Seek, W: Write, P: ProgressSink>(
    reader: &mut R,
    output: &mut W,
    offset: u64,
    mut left: u64,
    mut copied: u64,
    total: u64,
    name: &str,
    progress: &mut P,
    mut hash: Sha256,
) -> Result<[u8; 32], ConversionError> {
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|e| ConversionError::io(name, e))?;
    let mut buffer = vec![0; 1024 * 1024];
    while left > 0 {
        let take = buffer.len().min(left as usize);
        reader
            .read_exact(&mut buffer[..take])
            .map_err(|e| ConversionError::io(name, e))?;
        output
            .write_all(&buffer[..take])
            .map_err(|e| ConversionError::io(name, e))?;
        hash.update(&buffer[..take]);
        left -= take as u64;
        copied += take as u64;
        progress.report(copied, total);
    }
    Ok(hash.finalize().into())
}

fn patch_metadata(header: &mut [u8], plan: &CiaPlan, game: &PreparedGame) {
    header[0x2f9f] = plan.contents.len() as u8;
    header[0x2c1c..0x2c24].copy_from_slice(&plan.title_id.to_be_bytes());
    header[0x2f4c..0x2f54].copy_from_slice(&plan.title_id.to_be_bytes());
    header[0x2f5a..0x2f5e].copy_from_slice(&game.save_size);
}
