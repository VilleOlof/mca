//! # mca
//! A simple but effective & fast writer / reader for Minecrafts Region Files (mca).
//!
//! Reading a chunk and decompressing it's data
//! ```
//! use std::{fs::File, io::Read};
//! use mca::RegionReader;
//!
//! let mut data = Vec::new();
//! File::open("r.0.0.mca")?.read_to_end(&mut data)?;
//!
//! // Initialize the region
//! // This mostly just validates the header
//! let region = RegionReader::new(&data)?;
//!
//! // Get a specific chunk based of it's chunk coordinates
//! let chunk = region.get_chunk(0, 0)?.unwrap();
//!
//! // Decompress the chunk data
//! // This will most commonly be either ZLib or LZ4 compressed
//! let decompressed = chunk.decompress()?;
//!
//! // You can now bring your own NBT parser to parse the actual chunk data here
//! // I recommend either `simdnbt` or `fastnbt` for this.
//! ```
//!
//! And you can very easily write back the data
//! ```
//! use std::{fs::File};
//! use mca::RegionWriter;
//!
//! let data = vec![]; // some chunk data to write
//!
//! // Initialize the region writer
//! let mut writer = RegionWriter::new();
//!
//! // Push a chunk to the writer
//! writer.push_chunk(&data, (0, 0))?;
//!
//! // Write the writer to a buffer
//! let mut buf = vec![];
//! writer.write(&mut buf)?;
//!
//! // Write the buffer to a file
//! File::create("r.0.0.mca")?.write_all(&buf)?;
//! ```
//!
//! Theres also a [`RegionIter`] that you can use to easily iterate over all possible chunks in a region.
//! ```
//! use mca::RegionReader;
//!
//! // Here you will actually get the region data
//! let data = vec![];
//! let region = RegionReader::new(&data)?;
//!
//! // Then just call `.iter()` on the region
//! for chunk in region.iter() {
//!     // Iterator item is a Result<Option<RawChunk>>
//!     // So after the first unwrap we check if
//!     // there's actually a chunk or not
//!     if let Some(chunk) = chunk.unwrap() {
//!         // found chunk
//!     } else {
//!         // no chunk
//!     }    
//! }
//! ```

mod chunk;
mod compression;
mod error;
mod reader;
mod writer;

pub use chunk::{PendingChunk, RawChunk};
pub use compression::CompressionType;
pub use error::McaError;
pub use reader::{RegionIter, RegionReader};
pub use writer::RegionWriter;

/// How wide / tall a region is.  
///
/// *`(REGION_SIZE * REGION_SIZE)`*  
pub const REGION_SIZE: usize = 32;
const SECTOR_SIZE: usize = 4096;

#[cfg(test)]
mod tests {
    use super::*;

    const REGION: &[u8] = include_bytes!("../benches/r.0.0.mca");

    #[test]
    fn new_region() {
        let region = RegionReader::new(REGION).unwrap();

        assert_eq!(region.inner().len(), REGION.len());
    }

    #[test]
    fn chunk_parse() {
        let region = RegionReader::new(REGION).unwrap();
        let chunk = region.get_chunk(0, 0).unwrap().unwrap();

        assert_eq!(chunk.get_compression_type(), CompressionType::Zlib);
        assert!(chunk.raw_data.len() >= 4096);
    }

    #[test]
    fn entire_region() {
        let region = RegionReader::new(REGION).unwrap();

        for chunk in region.iter() {
            let _ = chunk.unwrap();
        }
    }

    #[test]
    fn parse_nbt() {
        let region = RegionReader::new(REGION).unwrap();
        let chunk = region.get_chunk(18, 17).unwrap().unwrap();

        let data = chunk.decompress().unwrap();
        let _ = sculk::chunk::Chunk::from_bytes(&data).unwrap();
    }

    #[test]
    fn decompress() {
        let region = RegionReader::new(REGION).unwrap();
        let chunk = region.get_chunk(18, 17).unwrap().unwrap();

        let _ = chunk.decompress().unwrap();
    }

    #[test]
    fn get_location() {
        let region = RegionReader::new(REGION).unwrap();
        let location = region
            .get_location(RegionReader::chunk_offset(0, 0))
            .unwrap();

        assert_eq!(location, [0, 3, 22, 2]);
    }

    #[test]
    fn get_timestamp() {
        let region = RegionReader::new(REGION).unwrap();
        #[cfg(feature = "unsafe")]
        let timestamp = reader.get_timestamp(RegionReader::chunk_offset(0, 0));

        #[cfg(not(feature = "unsafe"))]
        let timestamp = region
            .get_timestamp(RegionReader::chunk_offset(0, 0))
            .unwrap();

        assert_eq!(timestamp, [102, 128, 130, 115]);
    }

    #[test]
    fn no_chunk() {
        let mut bytes = vec![0, 0, 2, 1]; // offset 2 * SECTOR_SIZE
        bytes.extend_from_slice(&[0; 8188]);

        let region = RegionReader::new(&bytes).unwrap();

        let chunk = region.get_chunk(0, 0);

        if let Err(McaError::InvalidChunkPayload(_)) = chunk {
            assert!(true)
        } else {
            assert!(false)
        }
    }
}
