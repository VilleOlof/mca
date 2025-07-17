use crate::{compression::CompressionType, McaError, REGION_SIZE};

/// A raw compressed chunk, holds the compression type used.  
/// And the specific chunk byte slice from the region data
///
/// This is used when getting chunk data **from** a region file.  
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawChunk<'a> {
    pub raw_data: &'a [u8],
    compression_type: CompressionType,
}

impl<'a> RawChunk<'a> {
    /// Decompresses the raw chunk data depending on it's compression type
    ///
    /// ## Example
    /// ```ignore
    /// // ...
    ///
    /// let chunk = region.get_chunk(0, 0)?.unwrap();
    ///
    /// let data = chunk.decompress()?;
    /// ```
    pub fn decompress(&self) -> Result<Vec<u8>, McaError> {
        self.compression_type.decompress(&self.raw_data)
    }

    /// Get the chunks [`CompressionType`]
    pub fn get_compression_type(&self) -> CompressionType {
        self.compression_type.clone()
    }

    /// Creates a new raw chunk from it's bytes and compression type
    pub fn new(data: &'a [u8], compression: CompressionType) -> RawChunk<'a> {
        RawChunk {
            raw_data: data,
            compression_type: compression,
        }
    }
}

/// A `pending` chunk, holds all metadata used in region chunk payloads.  
///
/// This is used when **writing** region files.  
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PendingChunk {
    pub compressed_data: Vec<u8>,
    pub compression: CompressionType,
    pub timestamp: u32,
    pub coordinate: (u8, u8),
}

impl PendingChunk {
    /// Create a new pending chunk
    ///
    /// ## Example
    /// ```ignore
    /// use mca::{PendingChunk, CompressionType};
    ///
    /// let data: &[u8] = // ...
    ///
    /// let chunk = PendingChunk::new(&data, CompressionType::LZ4, 1724372177, (4, 6));
    /// ```
    pub fn new<B>(
        raw_data: &[u8],
        compression: CompressionType,
        timestamp: u32,
        coordinate: (B, B),
    ) -> Result<PendingChunk, McaError>
    where
        B: Into<u8>,
    {
        let coordinate = (coordinate.0.into(), coordinate.1.into());
        assert!(coordinate.0 < REGION_SIZE as u8);
        assert!(coordinate.1 < REGION_SIZE as u8);

        let compressed_data = compression.compress(&raw_data)?;

        Ok(PendingChunk {
            compressed_data,
            compression,
            timestamp,
            coordinate,
        })
    }

    /// Create a new pending chunk with already compressed data
    ///
    /// ## Example
    /// ```ignore
    /// use mca::{PendingChunk, CompressionType};
    ///
    /// let data: Vec<u8> = // ...
    ///
    /// let chunk = PendingChunk::new_compressed(&data, CompressionType::LZ4, 1724372177, (4, 6));
    /// ```
    pub fn new_compressed<B>(
        compressed_data: Vec<u8>,
        compression: CompressionType,
        timestamp: u32,
        coordinate: (B, B),
    ) -> Result<PendingChunk, McaError>
    where
        B: Into<u8>,
    {
        let coordinate = (coordinate.0.into(), coordinate.1.into());
        assert!(coordinate.0 < REGION_SIZE as u8);
        assert!(coordinate.1 < REGION_SIZE as u8);

        Ok(PendingChunk {
            compressed_data,
            compression,
            timestamp,
            coordinate,
        })
    }
}
