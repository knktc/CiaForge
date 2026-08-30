use std::io::{Read, Seek, SeekFrom, Write};

use sha2::{Digest, Sha256};

use super::{cci::Partition, ConversionError};

const BUFFER_SIZE: usize = 1024 * 1024;

pub trait ProgressSink {
    fn report(&mut self, completed: u64, total: u64);
}

pub fn copy_partition<R: Read + Seek, W: Write, P: ProgressSink>(
    reader: &mut R,
    writer: &mut W,
    partition: Partition,
    path: &str,
    progress: &mut P,
) -> Result<[u8; 32], ConversionError> {
    reader.seek(SeekFrom::Start(partition.offset)).map_err(|source| ConversionError::Io { path: path.into(), source })?;
    let mut left = partition.size;
    let mut copied = 0;
    let mut buffer = vec![0; BUFFER_SIZE.min(left as usize)];
    let mut hash = Sha256::new();
    while left > 0 {
        let take = buffer.len().min(left as usize);
        reader.read_exact(&mut buffer[..take]).map_err(|source| ConversionError::Io { path: path.into(), source })?;
        writer.write_all(&buffer[..take]).map_err(|source| ConversionError::Io { path: path.into(), source })?;
        hash.update(&buffer[..take]);
        left -= take as u64;
        copied += take as u64;
        progress.report(copied, partition.size);
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Recorder(Vec<(u64, u64)>);
    impl ProgressSink for Recorder { fn report(&mut self, completed: u64, total: u64) { self.0.push((completed, total)); } }

    #[test]
    fn copies_exact_partition_bytes_and_reports_completion() {
        let mut input = Cursor::new(b"abcdefghij".to_vec());
        let mut output = Cursor::new(Vec::new());
        let mut progress = Recorder(Vec::new());
        let digest = copy_partition(&mut input, &mut output, Partition { offset: 2, size: 5 }, "fixture.3ds", &mut progress).unwrap();
        assert_eq!(output.into_inner(), b"cdefg");
        assert_eq!(progress.0.last(), Some(&(5, 5)));
        assert_eq!(hex(&digest), "ff7834266e9e68caf1ca05fd2f11d469f6599abab3a62508cb645fde65d30dc3");
    }

    fn hex(bytes: &[u8]) -> String { bytes.iter().map(|byte| format!("{byte:02x}")).collect() }
}
