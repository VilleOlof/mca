//! Custom compression is a weird feature because I haven't ever seen someone use it.  
//! But from the wiki and the pages about the region file format.  
//!
//! If a compression byte is 127(or higher?), it indicates a custom compression format.  
//! And the byte is then followed by a length-prefixed string and THEN is the compressed chunk data.  
//! Which breaks the format sequence a bit but its ok.
//!
//! I just havent myself tested any of this custom decompression/compression tomfoolery because  
//! you would need a server that actually implements a custom format in its pipeline.  
//!
//! But in **theory**, this should support user defined code to modify the input data based on its id.  
//!
//! Because this is such a new obscure feature, 99% of library consumers wont use it and it shouldnt hinder their usage.  
//! Hence why most stuff default to `()` which implements both of these custom traits but errors if it comes across a custom format

use crate::CompressionError;

/// Easily allows for custom compression formats to define how the chunk data should be uncompressed.  
pub trait CustomDecompression: Send + Sync {
    /// Takes in the compressed chunk data (`data`), the algorithm used and the final uncompressed data should be written to `out`.
    ///
    /// Can return the amount of bytes written but is not used.   
    fn decompress(
        &self,
        data: &[u8],
        algorithm: &str,
        out: &mut Vec<u8>,
    ) -> Result<usize, CompressionError>;
}

impl CustomDecompression for () {
    fn decompress(&self, _: &[u8], _: &str, _: &mut Vec<u8>) -> Result<usize, CompressionError> {
        Err(CompressionError::Unsupported)
    }
}

/// Easily allows for custom compression formats to define how the chunk data should be compressed.  
pub trait CustomCompression: Send + Sync {
    /// Takes in a raw uncompressed chunk data (`data`), the algorithm used and the final compressed data should be written to `out`.  
    fn compress(
        &self,
        data: Vec<u8>,
        algorithm: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), CompressionError>;
}

impl CustomCompression for () {
    fn compress(&self, _: Vec<u8>, _: &str, _: &mut Vec<u8>) -> Result<(), CompressionError> {
        Err(CompressionError::Unsupported)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        ChunkIter, Compression, CompressionError, CustomCompression, CustomDecompression, McaError,
        RegionReader, RegionWriter, regions::*,
    };
    use lzma_rust2::{Lzma2Options, Lzma2Reader, Lzma2Writer, LzmaOptions};
    use na_nbt::ValueRef;
    use std::io::{Read, Write};

    // This test can be a useful resource on how to write custom compression formats
    // and how to use them with the RegionReader and Writer.
    // This is a pretty simple example compressing the data into lzma2.
    // The entire roundtrip test beneath does zlib > raw > lzma2 > raw and then verifies it so it actually works.

    struct LZMA2Format;

    impl LZMA2Format {
        const ID: &'static str = "lzma";

        pub fn compression() -> Compression {
            Compression::new_custom(Self::ID)
        }
    }

    impl CustomCompression for LZMA2Format {
        fn compress(
            &self,
            data: Vec<u8>,
            algorithm: &str,
            out: &mut Vec<u8>,
        ) -> Result<(), crate::CompressionError> {
            if algorithm != Self::ID {
                return Err(CompressionError::Unsupported);
            }

            let mut writer = Lzma2Writer::new(out, Lzma2Options::default());
            writer.write_all(&data).unwrap();
            writer.finish().unwrap();

            Ok(())
        }
    }

    impl CustomDecompression for LZMA2Format {
        fn decompress(
            &self,
            data: &[u8],
            algorithm: &str,
            out: &mut Vec<u8>,
        ) -> Result<usize, CompressionError> {
            if algorithm != Self::ID {
                return Err(CompressionError::Unsupported);
            }

            let mut reader = Lzma2Reader::new(data, LzmaOptions::DICT_SIZE_DEFAULT, None);

            Ok(reader
                .read_to_end(out)
                .map_err(|e| CompressionError::Custom(Box::new(e)))?)
        }
    }

    #[test]
    fn custom_compression_roundtrip() -> Result<(), McaError> {
        // first we need to unpack a vanilla compressed region and recompress it
        // then decompress it and verify that it still works
        let mut region = RegionReader::new(FULL)?;

        let mut writer = RegionWriter::new_with_compression(LZMA2Format);

        for (x, z) in ChunkIter::new() {
            if let Some(chunk) = region.chunk(x, z)? {
                writer.set_chunk(x, z, chunk.to_vec(), LZMA2Format::compression())?;
            };
        }

        let mut lzma_region = Vec::new();
        writer.write(&mut lzma_region)?;

        let mut new_lzma_region = RegionReader::new_with_decompression(&lzma_region, LZMA2Format)?;

        for (x, z) in ChunkIter::new() {
            if let Some(chunk) = new_lzma_region.chunk(x, z)? {
                // read it as nbt to confirm that the data is intact
                let nbt = na_nbt::read_borrowed::<na_nbt::BE>(chunk).unwrap();
                let nbt = nbt.root();
                let status = nbt
                    .get_::<na_nbt::tag::String>("Status")
                    .unwrap()
                    .to_utf8_string();

                assert!(status.starts_with("minecraft:"));
            }
        }

        Ok(())
    }
}
