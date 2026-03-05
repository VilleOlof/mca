#![doc = include_str!("../readme.md")]

mod compression;
mod custom_compression;
mod error;
mod iter;
mod read;
mod write;

pub use compression::Compression;
pub use custom_compression::{CustomCompression, CustomDecompression};
pub use error::{CompressionError, McaError};
pub use iter::{AvailableChunk, ChunkIter, RegionIter};
pub use read::{ChunkDataRef, CompressedChunk, RegionReader};
pub use write::{ChunkData, PackedChunk, RegionWriter, current_timestamp};

/// How many chunks exist in a direction, one region has a total of [`REGION_SIZE`] * [`REGION_SIZE`] chunks.  
pub const REGION_SIZE: usize = 32;

pub(crate) const SECTOR_SIZE: usize = 4096;

/// Returns the byte offset in each header based off a chunk position
#[inline(always)]
pub(crate) const fn header_offset(x: u8, z: u8) -> usize {
    4 * ((x as usize & (REGION_SIZE - 1)) + (z as usize & (REGION_SIZE - 1)) * REGION_SIZE)
}

// Regions used for tests
#[cfg(test)]
pub(crate) mod regions {
    /// FULL should be used for majority of tests and benchmarks.
    /// Its a vanilla 1.21.7 region with all chunks present.
    pub const FULL: &'static [u8] = include_bytes!("../data/full.mca");
    /// A region with around half of the chunks not generated at all
    pub const HALF: &'static [u8] = include_bytes!("../data/half.mca");
    /// A full region that was compressed with Lz4
    pub const LZ4: &'static [u8] = include_bytes!("../data/lz4.mca");
    /// A full region with no compression enabled
    pub const NONE: &'static [u8] = include_bytes!("../data/raw.mca");
    /// A region with no chunks generated it in, just empty headers
    pub const EMPTY: &'static [u8] = include_bytes!("../data/empty.mca");
    /// The corrupt region file is based off the full.mca file
    /// Taking the headers and then throwing 512000 random bytes after it
    pub const CORRUPT: &'static [u8] = include_bytes!("../data/corrupt.mca");
    /// A full region that was compressed with ZLib
    pub const ZLIB: &'static [u8] = FULL;
}

#[cfg(test)]
mod test {
    use crate::header_offset;

    #[test]
    fn offset() {
        // (x, z, index)
        let coords = vec![
            (0, 0, 0),
            (1, 4, 516),
            (5, 1, 148),
            (13, 21, 2740),
            (31, 8, 1148),
            (31, 31, 4092),
        ];

        for (x, z, should_be) in coords {
            let idx = header_offset(x, z);
            assert_eq!(idx, should_be, "({x}, {z})");
        }
    }
}
