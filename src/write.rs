use std::{fmt::Debug, io::Write};

use crate::{
    Compression, CompressionError, McaError, REGION_SIZE, SECTOR_SIZE,
    custom_compression::CustomCompression, header_offset,
};

/// Used to write raw chunk data to and then format it into the region file format *(.mca)*  
///
/// ## Example
/// ```
/// # use mca::{RegionWriter, Compression};
/// let mut writer = RegionWriter::new();
///
/// writer.set_chunk(0, 0, Vec::new(), Compression::ZLib)?;
///
/// let mut region_file: Vec<u8> = Vec::new();
/// writer.write(&mut region_file)?;
/// # Ok::<(), mca::McaError>(())
/// ```
#[derive(Clone)]
pub struct RegionWriter<C: CustomCompression = ()> {
    chunks: [Option<ChunkData>; 1024],
    // this isnt used right now but i want to use this
    // to pre-allocate a buffer for the `write` iterator that compresses chunks
    // since right now it does a collect() which i think does 9 allocations too much
    // growth scale: 0,4,8,16,32,64,128,256,512,1024. if the region was filled with 1024 chunks
    changed_chunks: u16,
    custom_compression: C,
}

impl Default for RegionWriter<()> {
    fn default() -> Self {
        Self::new()
    }
}

/// A chunks raw nbt data and the needed data to write it to a region.  
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ChunkData {
    /// The chunks nbt data in bytes
    pub data: Vec<u8>,
    /// Which compression scheme should be used
    pub compression: Compression,
    /// The chunks region local coordinates
    pub chunk: (u8, u8),
}

/// A compressed chunk and some further metadata about the chunk needed to write it to a region.  
///
/// Can be used to have more control on what data is written with a chunks data.  
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PackedChunk {
    /// The chunks data compressed in whatever format is specified.  
    pub compressed: Vec<u8>,
    /// The compression format that was used to compress the chunk data
    pub compression: Compression,
    /// The chunks region local coordinates.  
    pub chunk: (u8, u8),
    /// How many 4096 byte sectors the chunk data and its metadata take up in the region file.  
    ///
    /// Never calculate this on your own and use [`RegionWriter::sector_size`]
    pub sector_size: usize,
    /// A timestamp of when the chunk was updated last
    /// This is optional since the normal way would be for the writer
    /// to use one singular timestamp for all chunks.  
    /// But this can be overwritten with this if you happen to create these PackedChunks yourself.  
    pub timestamp: Option<u32>,
}

/// A buffer of only zeros used to pad chunks to be sector aligned.  
static ZERO_BUF: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];

impl ChunkData {
    /// Compresses chunks data with whatever compression scheme was specified.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{ChunkData, Compression};
    /// let mut chunk = ChunkData {
    ///     data: Vec::new(),
    ///     compression: Compression::ZLib,
    ///     chunk: (0, 0)
    /// };
    ///
    /// // no custom compression specified
    /// let compressed = chunk.compress(&())?;
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn compress<C: CustomCompression>(
        self,
        custom_compression: &C,
    ) -> Result<Vec<u8>, McaError> {
        use flate2::write::{GzEncoder, ZlibEncoder};

        // skip any logic if already empty
        if self.data.is_empty() {
            return Ok(Vec::new());
        }

        let mut buf = Vec::with_capacity(SECTOR_SIZE * 2); // most commonly around 2 sectors per chunk
        match self.compression {
            Compression::Gzip => {
                let mut g = GzEncoder::new(&mut buf, flate2::Compression::new(4));
                g.write_all(&self.data)?;
            }
            Compression::ZLib => {
                let mut z = ZlibEncoder::new(&mut buf, flate2::Compression::new(4));
                z.write_all(&self.data)?;
            }
            Compression::None => buf = self.data,
            Compression::Lz4 => {
                let mut l = lz4_java_wrc::Lz4BlockOutput::new(&mut buf);
                l.write_all(&self.data)?;
            }
            Compression::Custom((_, id)) => {
                custom_compression.compress(self.data, &id, &mut buf)?
            }
        }

        Ok(buf)
    }
}

impl RegionWriter<()> {
    /// Creates a new [`RegionWriter`] with no custom compression specified.  
    pub fn new() -> Self {
        RegionWriter::new_with_compression(())
    }

    /// Calculates how many 4096 byte sectors a complete chunk data view is.  
    ///
    /// *data length + compression byte + ?custom compression id + compressed data* / 4096
    pub fn sector_size(compressed_len: usize, compression: &Compression) -> usize {
        Self::chunk_view_len(compressed_len, compression).div_ceil(SECTOR_SIZE)
    }

    /// Calculates how many bytes a complete chunk data view is.  
    ///
    /// *data length + compression byte + ?custom compression id + compressed data*
    pub fn chunk_view_len(compressed_len: usize, compression: &Compression) -> usize {
        // data len (4) + compresson(1 or 1+prefixed string) + compresson data
        compressed_len + 4 + compression.size()
    }

    /// Writes a list of [`PackedChunk`]s into the specified writer.  
    /// Used internally by [`RegionWriter::write`] after it has compressed and gathered metadata per chunk.  
    ///
    /// This may be useful if you need more control, like skipping the libraries compression handling.  
    /// Look more at [`PackedChunk`] for info and important things to know
    ///
    /// ## Example
    /// ```
    /// # use mca::{PackedChunk, RegionWriter, Compression};
    /// let chunks = vec![PackedChunk::default(), PackedChunk::default()];
    ///
    /// let mut buf = Vec::new();
    /// RegionWriter::<()>::write_packed(&mut buf, chunks)?;
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn write_packed<W: Write>(
        writer: &mut W,
        chunks: Vec<PackedChunk>,
    ) -> Result<usize, McaError> {
        // write_packed can sit in "C = ()" since it doesnt use custom compression
        // since that has been already dealt with inside of each PackedChunk
        let mut locations = [0u8; SECTOR_SIZE];
        let mut timestamps = [0u8; SECTOR_SIZE];
        let mut sectors = 2;

        let time = current_timestamp().to_be_bytes();
        for chunk in &chunks {
            let (x, z) = chunk.chunk;
            let header_i = header_offset(x, z);

            let offset = (sectors as u32).to_be_bytes();
            locations[header_i..=header_i + 2].copy_from_slice(&offset[1..4]);
            locations[header_i + 3] = chunk.sector_size as u8;

            let timestamp = match chunk.timestamp {
                Some(t) => t.to_be_bytes(),
                None => time,
            };
            timestamps[header_i..=header_i + 3].copy_from_slice(&timestamp);

            sectors += chunk.sector_size;
        }

        writer.write_all(&locations)?;
        writer.write_all(&timestamps)?;

        let mut chunk_len = 0;
        for chunk in chunks {
            // important to take custom id into the entire length of the chunk view
            let mut data_len: u32 = chunk.compressed.len() as u32 + 1u32;
            if let Compression::Custom((_, id)) = &chunk.compression {
                data_len += (size_of::<u16>() + id.len()) as u32;
            }

            writer.write_all(&data_len.to_be_bytes())?;
            writer.write_all(&[chunk.compression.to_u8()])?;

            if let Compression::Custom((_, id)) = &chunk.compression {
                let buf = id.clone().into_bytes();

                if buf.len() > u8::MAX as usize {
                    return Err(McaError::Compression(CompressionError::CustomIdTooBig(
                        id.clone(),
                    )));
                }
                let num = (buf.len() as u16).to_be_bytes();

                writer.write_all(&num)?;
                writer.write_all(&buf)?;
            }

            writer.write_all(&chunk.compressed)?;
            let len = RegionWriter::chunk_view_len(chunk.compressed.len(), &chunk.compression);

            let padded_size = (chunk.sector_size * SECTOR_SIZE).saturating_sub(len);
            if padded_size > 0 {
                writer.write_all(&ZERO_BUF[..padded_size])?;
                chunk_len += padded_size;
            }

            chunk_len += len;
        }

        Ok(locations.len() + timestamps.len() + chunk_len)
    }
}

impl<C: CustomCompression> RegionWriter<C> {
    /// Creates a new [`RegionWriter`] with a specified custom compression to use when writing chunks.  
    pub fn new_with_compression(custom_compression: C) -> Self {
        Self {
            chunks: [const { None }; 1024],
            changed_chunks: 0,
            custom_compression,
        }
    }

    /// Returns a reference to a chunk entry within the [`RegionWriter`]
    ///
    /// ## Error
    /// Fails if the specified chunk coordinates are outside of the region
    pub fn chunk(&self, x: u8, z: u8) -> Result<&Option<ChunkData>, McaError> {
        match self.chunks.get((x as usize * REGION_SIZE) + z as usize) {
            Some(chunk) => Ok(chunk),
            None => Err(McaError::InvalidChunkPosition(x, z)),
        }
    }

    /// Returns a mutable reference to a chunk entry within the [`RegionWriter`]
    ///
    /// ## Error
    /// Fails if the specified chunk coordinates are outside of the region
    pub fn chunk_mut(&mut self, x: u8, z: u8) -> Result<&mut Option<ChunkData>, McaError> {
        match self.chunks.get_mut((x as usize * REGION_SIZE) + z as usize) {
            Some(chunk) => Ok(chunk),
            None => Err(McaError::InvalidChunkPosition(x, z)),
        }
    }

    /// Set the data of a specific chunk along side it's compression.  
    ///
    /// Can be called multiple times on the same coordinates, but will overwrite previous data.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionWriter, Compression};
    /// let mut region = RegionWriter::new();
    ///
    /// region.set_chunk(5, 1, Vec::new(), Compression::default())?;
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn set_chunk(
        &mut self,
        x: u8,
        z: u8,
        data: Vec<u8>,
        compression: Compression,
    ) -> Result<(), McaError> {
        *self.chunk_mut(x, z)? = Some(ChunkData {
            data,
            compression,
            chunk: (x, z),
        });

        self.changed_chunks += 1;

        Ok(())
    }

    /// Marks this chunk as empty and will skip writing this chunk.
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionWriter, Compression};
    /// let mut region = RegionWriter::new();
    ///
    /// region.set_chunk(5, 1, Vec::new(), Compression::default())?;
    /// region.set_chunk(21, 8, Vec::new(), Compression::default())?;
    ///
    /// // 5, 1 will not get written now, but 21, 8 will
    /// region.clear_chunk(5, 1)?;
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn clear_chunk(&mut self, x: u8, z: u8) -> Result<(), McaError> {
        *self.chunk_mut(x, z)? = None;

        self.changed_chunks -= 1;

        Ok(())
    }

    /// Writes the chunks to a destination  
    ///
    /// This will compress and format the chunks into the region file format *(.mca)*  
    ///
    /// By default, this compresses all chunks in parallel, but can be disabled with `default-features = false` in your cargo.toml
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionWriter, Compression};
    /// # use std::fs::File;
    /// let mut region = RegionWriter::new();
    ///
    /// region.set_chunk(5, 8, Vec::new(), Compression::ZLib)?;
    ///
    /// // region.write can take in anything that impl `Write`
    /// let mut file = Vec::new();
    /// region.write(&mut file)?;
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn write<W: Write>(self, writer: &mut W) -> Result<usize, McaError> {
        #[cfg(feature = "rayon")]
        use rayon::iter::{IntoParallelIterator, ParallelIterator};

        #[cfg(feature = "rayon")]
        let iter = self.chunks.into_par_iter();
        #[cfg(not(feature = "rayon"))]
        let iter = self.chunks.into_iter();

        let chunks = iter
            .filter_map(|s| s)
            .map(|chunk| {
                let compression = chunk.compression.clone();
                let (x, z) = chunk.chunk;
                let compressed = chunk.compress(&self.custom_compression)?;
                let sector_size = RegionWriter::sector_size(compressed.len(), &compression);

                Ok::<PackedChunk, McaError>(PackedChunk {
                    compressed,
                    compression,
                    chunk: (x, z),
                    sector_size,
                    timestamp: None,
                })
            })
            .collect::<Result<Vec<PackedChunk>, McaError>>()?;

        RegionWriter::write_packed(writer, chunks)
    }
}

/// Gets the current time in unix epoch
pub fn current_timestamp() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
    since_the_epoch.to_be()
}

impl<C: CustomCompression + Debug> Debug for RegionWriter<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RegionWriter {{ changed_chunks: {}, custom_compression: {:?} }}",
            self.changed_chunks, self.custom_compression
        )
    }
}

impl Debug for ChunkData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ChunkData {{ data_len: {}, compression: {:?}, chunk: {:?} }}",
            self.data.len(),
            self.compression,
            self.chunk
        )
    }
}

impl Debug for PackedChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PackedChunk {{ compressed_len: {}, compression: {:?}, chunk: {:?}, sector_size: {}, timestamp: {:?} }}",
            self.compressed.len(),
            self.compression,
            self.chunk,
            self.sector_size,
            self.timestamp
        )
    }
}

#[cfg(test)]
mod test {
    use crate::{
        ChunkData, Compression, McaError, PackedChunk, RegionReader, RegionWriter, SECTOR_SIZE,
        current_timestamp, regions::*,
    };

    #[test]
    fn write_full() -> Result<(), McaError> {
        let reader = RegionReader::new(FULL)?;
        let region = reader.into_writer(())?;

        let mut out = Vec::new();
        region.write(&mut out)?;

        Ok(())
    }

    #[test]
    fn write_lz4() -> Result<(), McaError> {
        let reader = RegionReader::new(LZ4)?;
        let region = reader.into_writer(())?;

        let mut out = Vec::new();
        region.write(&mut out)?;

        Ok(())
    }

    #[test]
    fn write_none() -> Result<(), McaError> {
        let reader = RegionReader::new(NONE)?;
        let region = reader.into_writer(())?;

        let mut out = Vec::new();
        region.write(&mut out)?;

        Ok(())
    }

    #[test]
    fn write_empty() -> Result<(), McaError> {
        let reader = RegionReader::new(EMPTY)?;
        let region = reader.into_writer(())?;

        let mut out = Vec::new();
        region.write(&mut out)?;

        Ok(())
    }

    #[test]
    fn empty_writer() -> Result<(), McaError> {
        let writer = RegionWriter::new();

        let mut buf = Vec::new();
        writer.write(&mut buf)?;

        assert_eq!(buf.len(), SECTOR_SIZE * 2);

        Ok(())
    }

    #[test]
    fn sector_size() {
        assert_eq!(RegionWriter::sector_size(0, &Compression::None), 1);
        assert_eq!(RegionWriter::sector_size(1842, &Compression::None), 1);
        assert_eq!(RegionWriter::sector_size(8180, &Compression::None), 2);
        assert_eq!(RegionWriter::sector_size(11421, &Compression::None), 3);
    }

    #[test]
    fn chunk_view_len() {
        assert_eq!(RegionWriter::chunk_view_len(0, &Compression::None), 5);
        assert_eq!(RegionWriter::chunk_view_len(6180, &Compression::None), 6185);
        assert_eq!(
            RegionWriter::chunk_view_len(1000, &Compression::new_custom("test")),
            1011
        );
    }

    #[test]
    fn chunk_none() -> Result<(), McaError> {
        let writer = RegionWriter::new();
        assert_eq!(*writer.chunk(5, 12)?, None);
        Ok(())
    }

    #[test]
    fn chunk_mut() -> Result<(), McaError> {
        let mut writer = RegionReader::new(FULL)?.into_writer(())?;

        *writer.chunk_mut(8, 1)? = Some(ChunkData {
            data: vec![5, 4, 3, 2, 1],
            compression: Compression::None,
            chunk: (8, 1),
        });

        assert_eq!(
            writer.chunk(8, 1)?.as_ref().unwrap().data,
            vec![5, 4, 3, 2, 1]
        );

        Ok(())
    }

    #[test]
    fn set_chunk() -> Result<(), McaError> {
        let mut w = RegionWriter::new();
        assert_eq!(w.changed_chunks, 0);
        w.set_chunk(4, 21, Vec::new(), Compression::None)?;
        assert_eq!(w.changed_chunks, 1);
        w.clear_chunk(4, 21)?;
        assert_eq!(w.changed_chunks, 0);
        Ok(())
    }

    #[test]
    fn write_packed() -> Result<(), McaError> {
        let chunks = vec![
            PackedChunk::default(),
            PackedChunk {
                compressed: vec![1, 0, 1, 0, 0, 1],
                compression: Compression::ZLib,
                chunk: (31, 23),
                sector_size: 1,
                timestamp: Some(current_timestamp()),
            },
        ];

        let mut buf = Vec::new();
        RegionWriter::write_packed(&mut buf, chunks)?;

        Ok(())
    }

    #[test]
    fn debug() -> Result<(), McaError> {
        println!("{:?}", RegionReader::new(HALF)?.into_writer(())?);
        println!(
            "{:?}",
            RegionReader::new(FULL)?
                .into_writer(())?
                .chunk(1, 15)?
                .as_ref()
                .unwrap()
        );
        println!(
            "{:?}",
            PackedChunk {
                compressed: vec![0, 1, 1, 0, 0, 1, 1, 0],
                compression: Compression::Lz4,
                chunk: (1, 4),
                sector_size: 1,
                timestamp: None
            }
        );
        Ok(())
    }
}
