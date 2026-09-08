use sha2::{Digest, Sha256};

use super::{
    CiaHeader, CiaPlan, ConversionError, ProgressSink, RandomAccessInput, RandomAccessOutput,
    RetailTemplates, inspect_async, prepare_async, writer::CONTENT_RECORDS_OFFSET,
};

const BUFFER_SIZE: usize = 1024 * 1024;

pub async fn convert_async<I: RandomAccessInput, O: RandomAccessOutput, P: ProgressSink>(
    input: &mut I,
    input_name: &str,
    input_len: u64,
    output: &mut O,
    progress: &mut P,
) -> Result<(), ConversionError> {
    let header = inspect_async(input, input_name, input_len).await?;
    let game = prepare_async(input, header.game, header.encryption, input_name).await?;
    let plan = CiaPlan::from_header(&header);
    let templates = RetailTemplates::load()?;
    let mut cia = CiaHeader::new(
        &plan,
        &templates.certificate_chain,
        &templates.ticket_and_tmd,
    )
    .map_err(ConversionError::Template)?;
    cia.0[0x2f9f] = plan.contents.len() as u8;
    cia.0[0x2c1c..0x2c24].copy_from_slice(&plan.title_id.to_be_bytes());
    cia.0[0x2f4c..0x2f54].copy_from_slice(&plan.title_id.to_be_bytes());
    cia.0[0x2f5a..0x2f5e].copy_from_slice(&game.save_size);
    output
        .write_at(0, &cia.0)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    let mut output_offset = cia.0.len() as u64;
    let mut hashes = Vec::with_capacity(plan.contents.len());
    let mut hash = Sha256::new();
    hash.update(game.ncch_header);
    hash.update(game.extheader);
    output
        .write_at(output_offset, &game.ncch_header)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 0x200;
    output
        .write_at(output_offset, &game.extheader)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 0x400;
    hashes.push(
        copy_range(
            input,
            output,
            header.game.offset + 0x600,
            header.game.size - 0x600,
            0x600,
            header.game.size,
            output_offset,
            input_name,
            progress,
            hash,
        )
        .await?,
    );
    output_offset += header.game.size - 0x600;
    for content in plan.contents.iter().skip(1) {
        hashes.push(
            copy_range(
                input,
                output,
                content.partition.offset,
                content.partition.size,
                0,
                content.partition.size,
                output_offset,
                input_name,
                progress,
                Sha256::new(),
            )
            .await?,
        );
        output_offset += content.partition.size;
    }
    let mut records = plan.content_records();
    for (index, hash) in hashes.iter().enumerate() {
        records[index * 0x30 + 0x10..index * 0x30 + 0x30].copy_from_slice(hash);
        output
            .write_at(
                CONTENT_RECORDS_OFFSET as u64 + index as u64 * 0x30 + 0x10,
                hash,
            )
            .await
            .map_err(|e| ConversionError::io(input_name, e))?;
    }
    let record_hash = Sha256::digest(&records);
    output
        .write_at(0x2fc7, &[plan.contents.len() as u8])
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output
        .write_at(0x2fc8, &record_hash)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    let mut info = vec![0; 3];
    info.push(plan.contents.len() as u8);
    info.extend_from_slice(&record_hash);
    info.resize(info.len() + 0x8dc, 0);
    output
        .write_at(0x2fa4, &Sha256::digest(&info))
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output
        .write_at(output_offset, &game.dependency_list)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 0x180;
    output
        .write_at(output_offset, &[0; 0x180])
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 0x180;
    output
        .write_at(output_offset, &2u32.to_le_bytes())
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 4;
    output
        .write_at(output_offset, &[0; 0xfc])
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output_offset += 0xfc;
    output
        .write_at(output_offset, &game.smdh)
        .await
        .map_err(|e| ConversionError::io(input_name, e))?;
    output
        .finish()
        .await
        .map_err(|e| ConversionError::io(input_name, e))
}

async fn copy_range<I: RandomAccessInput, O: RandomAccessOutput, P: ProgressSink>(
    input: &mut I,
    output: &mut O,
    mut input_offset: u64,
    mut left: u64,
    mut copied: u64,
    total: u64,
    mut output_offset: u64,
    name: &str,
    progress: &mut P,
    mut hash: Sha256,
) -> Result<[u8; 32], ConversionError> {
    let mut buffer = vec![0; BUFFER_SIZE];
    while left > 0 {
        let take = buffer.len().min(left as usize);
        input
            .read_at(input_offset, &mut buffer[..take])
            .await
            .map_err(|e| ConversionError::io(name, e))?;
        output
            .write_at(output_offset, &buffer[..take])
            .await
            .map_err(|e| ConversionError::io(name, e))?;
        hash.update(&buffer[..take]);
        input_offset += take as u64;
        output_offset += take as u64;
        left -= take as u64;
        copied += take as u64;
        progress.report(copied, total);
    }
    Ok(hash.finalize().into())
}
