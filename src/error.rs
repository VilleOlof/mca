use crate::REGION_SIZE;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McaError {
    #[error("Region is too small to even include the headers: {0}")]
    MissingHeaders(usize),
    #[error("No chunk offset found in location header")]
    MissingOffset,
    #[error("No data length found in chunk data")]
    MissingDataLen,
    #[error("Failed to create a few containing an entire chunk data referenced by it's data length")]
    FailedToCreateChunkView,
    #[error("Chunk view was empty")]
    NoChunkView,
    #[error("No compression byte found")]
    MissingCompression,
    #[error("No compressed chunk data found")]
    MissingChunkData,
    #[error("No timestamp for specified chunk in timestamp header")]
    MissingTimestamp,
    #[error(
        "Chunk coordinates is outside of region, both {0}, {1} must be below {size}",
        
        size = REGION_SIZE
    )]
    InvalidChunkPosition(u8, u8),
    #[error("Got an invalid chunk coordinate while iterating a region: ({1}, {2}) is not in {0:?}")]
    InvalidRegionIterChunk(Vec<(u8, u8)>, u8, u8),
    #[error("Failed to write data: {0:#?}")]
    WriteError(#[from] std::io::Error),
    #[error("Failed to create custom compression id: {0:#?}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("{0}")]
    Compression(#[from] CompressionError),
}

#[derive(Debug, Error)]
pub enum CompressionError {
    #[error("Failed to compress/uncompress Gzip: {0}")]
    Gzip(std::io::Error),
    #[error("Failed to compress/uncompress ZLib: {0}")]
    ZLib(std::io::Error),
    #[error("Failed to compress/uncompress LZ4: {0}")]
    LZ4(std::io::Error),
    #[error("Failed to compress/uncompress a custom format: {0}")]
    Custom(Box<dyn std::error::Error + Send>),
    #[error("Custom compression id can't be bigger than {}", u8::MAX)]
    CustomIdTooBig(String),
    /// To use a custom compression format, use `Region::with_decompression(x)` and something that implements `CustomDecompression` that can handle the different compression bytes that suits your needs
    #[error("Found a unspported compression format")]
    Unsupported,
}
