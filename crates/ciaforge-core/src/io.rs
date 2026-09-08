//! The host-I/O seam used by the browser and desktop adapters.
//!
//! Operations are positional rather than sequential: CCI metadata is read at
//! fixed offsets and CIA hashes are written back into an earlier header.

use std::io;

/// Supplies bounded byte ranges without requiring the whole source in memory.
#[allow(async_fn_in_trait)]
pub trait RandomAccessInput {
    async fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()>;
}

/// Persists bytes at a specific location in the destination.
#[allow(async_fn_in_trait)]
pub trait RandomAccessOutput {
    async fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>;
    async fn finish(&mut self) -> io::Result<()>;
}
