use std::fmt::Debug;

use crate::{
    ChunkIter, Compression, CompressionError, CustomCompression, CustomDecompression, McaError,
    REGION_SIZE, RegionIter, RegionWriter, SECTOR_SIZE, header_offset,
};

/// A representation of a Minecraft **[Region](https://minecraft.wiki/w/Region_file_format)**, used to read and decompress the format.  
///
/// ## Example
/// ```
/// # use mca::RegionReader;
/// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
/// let mut region = RegionReader::new(REGION)?;
/// # Ok::<(), mca::McaError>(())
/// ```
#[derive(Clone)]
pub struct RegionReader<'a, D = ()>
where
    D: CustomDecompression,
{
    data: &'a [u8],
    decompress_buf: Vec<u8>,
    pub(crate) custom_decompression: D,
}

/// A reference to a piece of chunk data.  
///
/// Containing the compressed chunk data (wrapped in a type to really notify you that its not uncompressed yet).  
/// And which compression format is used.
///
/// Use [`RegionReader::decompress_data_ref`] to manually decompress the data.
/// You should prefer just [`RegionReader::chunk`] which decompresses the chunk automatically.  
///
/// But if you for some reason get chunk data externally or from [`RegionReader::chunk_data`], then this will be used.  
#[derive(Clone)]
pub struct ChunkDataRef<'a> {
    /// The chunks nbt data that is compressed
    ///
    /// can use [`CompressedChunk::as_ref`] to get the inner data.  
    pub data: CompressedChunk<'a>,
    /// The compression format that was used to compress the chunks data
    pub compression: Compression,
}

/// A compressed chunk.  
///
/// Use [`CompressedChunk::as_ref`] to get the inner referenced buffer.  
#[derive(Clone)]
pub struct CompressedChunk<'a>(&'a [u8]);
impl<'a> AsRef<[u8]> for CompressedChunk<'a> {
    fn as_ref(&self) -> &'a [u8] {
        self.0
    }
}

impl<'a> CompressedChunk<'a> {
    /// Wraps a buffer in a [`CompressedChunk`] to notify that the data isn't ready to be used yet.  
    pub fn new(data: &'a [u8]) -> CompressedChunk<'a> {
        Self(data)
    }
}

impl<'a> RegionReader<'a, ()> {
    /// Creates a new [`RegionReader`] with no [`CustomDecompression`] specified.  
    ///
    /// ```
    /// # use mca::RegionReader;
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let chunk = region.chunk(5, 1)?;
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    #[inline]
    pub fn new(data: &'a [u8]) -> Result<RegionReader<'a>, McaError> {
        Self::new_with_decompression(data, ())
    }
}

impl<'a, D: CustomDecompression> RegionReader<'a, D> {
    /// Creates a new [`RegionReader`] with a specified [`CustomDecompression`] scheme incase a non-vanilla compression byte appears.  
    ///
    /// Use [`RegionReader::new`] if you don't need a [`CustomDecompression`] and any non-vanilla compression byte will return [`CompressionError::Unsupported`].  
    ///
    /// ## Error
    /// Fails if the data given is less than **8192** bytes *(minimum header size)*.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionReader, CustomDecompression, CompressionError};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// struct Custom;
    /// impl CustomDecompression for Custom {
    ///     fn decompress(&self, data: &[u8], algorithm: &str, out: &mut Vec<u8>) -> Result<usize, CompressionError> {
    ///         // handle non-vanilla compression bytes ...
    ///         # todo!()
    ///     }
    /// }
    ///
    /// let mut region = RegionReader::new_with_decompression(REGION, Custom)?;
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    /// For a more concrete example, you can look at the tests in `src/custom_compression.rs`.
    #[inline]
    pub fn new_with_decompression(
        data: &'a [u8],
        custom_decompression: D,
    ) -> Result<RegionReader<'a, D>, McaError> {
        if data.len() < (SECTOR_SIZE * 2) {
            return Err(McaError::MissingHeaders(data.len()));
        }

        Ok(RegionReader {
            data,
            decompress_buf: Vec::with_capacity(1_048_576),
            custom_decompression,
        })
    }

    /// Returns the raw, uncompressed data for a chunk within a region.  
    ///
    /// Returns `Ok(None)` if the chunk isn't present in the region *(not generated yet)*.  
    ///
    /// ## Example
    /// ```
    /// # use mca::RegionReader;
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let chunk = region.chunk(5, 24)?;
    ///
    /// if let Some(chunk) = chunk {
    ///     // decode the raw chunk into nbt
    ///     // using something like "simdnbt" or "na_nbt"
    /// }
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    #[inline]
    pub fn chunk(&mut self, x: u8, z: u8) -> Result<Option<&[u8]>, McaError> {
        match self.chunk_data(x, z)? {
            Some(c) => Ok(Some(self.decompress_to_internal_buffer(c)?)),
            None => Ok(None),
        }
    }

    /// Returns the sector offset for the specified chunk in the region file.  
    pub(crate) fn sector_offset(&self, x: u8, z: u8) -> Result<Option<[u8; 4]>, McaError> {
        let chunk_offset = header_offset(x, z);

        let offset_data = match self.data.get(chunk_offset..chunk_offset + 4) {
            Some(offset) => offset,
            None => return Err(McaError::MissingOffset),
        };
        if offset_data.len() != 4 || offset_data == [0, 0, 0, 0] {
            return Ok(None);
        }

        Ok(Some(offset_data.try_into().unwrap()))
    }

    /// Returns the compressed data from the specified chunk.  
    ///
    /// Returns `Ok(None)` if the chunk isn't present in the region *(not generated yet)*.  
    ///
    /// You should prefer to use [`RegionReader::chunk`] instead as it also decompresses the data.  
    ///
    /// Since you cant use [`RegionReader::chunk`] in across threads, you'd have to use this and manually call [`RegionReader::decompress_data_ref`].
    /// See parallel example below
    ///
    /// ## Example
    /// ```
    /// # use mca::RegionReader;
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let chunk = region.chunk_data(5, 1)?;
    ///
    /// if let Some(chunk) = chunk {
    ///     println!("compression: {:?}, size: {}", chunk.compression, chunk.data.as_ref().len());
    /// }
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    ///
    /// ## Parallel Example
    /// ```ignore
    /// # // so kinda hard to run this doc since we have to disable rayon for other tests to work
    /// # // i mean this runs in normal tests soooo
    /// # use mca::{RegionReader, REGION_SIZE, McaError};
    /// # use rayon::iter::{IntoParallelIterator, ParallelIterator};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// // Get the total size of the uncompressed chunk data using rayon.  
    /// let sizes: Vec<usize> = (0..(REGION_SIZE * REGION_SIZE))
    ///     .into_par_iter()
    ///     .map(|s| {
    ///         let chunk = match region.chunk_data((s % 32) as u8, (s / 32) as u8)? {
    ///             Some(c) => c,
    ///             None => return Ok(0),
    ///         };
    ///
    ///         let mut uncompressed = Vec::new();
    ///         RegionReader::decompress_data_ref(
    ///             chunk.data,
    ///             chunk.compression,
    ///             &mut uncompressed,
    ///             &(),
    ///         )?;
    ///
    ///         Ok::<usize, McaError>(uncompressed.len())
    ///     })
    ///     .collect::<Result<Vec<usize>, McaError>>()
    ///     .unwrap();
    ///
    /// let total: usize = sizes.iter().sum();
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    #[inline]
    pub fn chunk_data(&self, x: u8, z: u8) -> Result<Option<ChunkDataRef<'a>>, McaError> {
        if x >= REGION_SIZE as u8 || z >= REGION_SIZE as u8 {
            return Err(McaError::InvalidChunkPosition(x, z));
        }

        let offset_data = match self.sector_offset(x, z)? {
            Some(od) => od,
            None => return Ok(None),
        };

        // with these two uncheckeds, they cant fail due to the checks before them
        // and that they must have x bytes in them, so we just format it to convert to u32
        let offset = unsafe {
            u32::from_be_bytes([
                0,
                *offset_data.get_unchecked(0),
                *offset_data.get_unchecked(1),
                *offset_data.get_unchecked(2),
            ])
        } as usize
            * SECTOR_SIZE;

        let dlb = self
            .data
            .get(offset..offset + 4)
            .ok_or(McaError::MissingDataLen)?;
        let data_len = unsafe {
            u32::from_be_bytes([
                *dlb.get_unchecked(0),
                *dlb.get_unchecked(1),
                *dlb.get_unchecked(2),
                *dlb.get_unchecked(3),
            ]) as usize
        };

        let view = &self
            .data
            .get(offset + 4..offset + 4 + data_len)
            .ok_or(McaError::FailedToCreateChunkView)?;
        if view.is_empty() {
            return Err(McaError::NoChunkView);
        }

        let compression = *view.first().ok_or(McaError::MissingCompression)?;
        let (compression, extra_c_bytes) = match compression {
            127.. => {
                let str_len = u16::from_be_bytes([view[1], view[2]]) as usize;

                let algorithm_id = String::from_utf8(view[3..3 + str_len].to_vec())?;
                (
                    Compression::Custom((compression, algorithm_id)),
                    size_of::<u16>() + str_len,
                )
            }
            v => (Compression::from_u8(v), 0),
        };

        let data = match view.get(1 + extra_c_bytes..) {
            Some(data) => data,
            None => return Err(McaError::MissingChunkData),
        };

        Ok(Some(ChunkDataRef {
            data: CompressedChunk(data),
            compression,
        }))
    }

    /// Decompresses a compressed chunk using the regions internal buffer to avoid allocation for decompression.  
    ///
    /// Look at [`RegionReader::decompress_data_ref`] for a standalone decompression function that this internally calls.  
    #[inline]
    pub fn decompress_to_internal_buffer<'c>(
        &'c mut self,
        data: ChunkDataRef<'a>,
    ) -> Result<&'c [u8], McaError> {
        self.decompress_buf.clear();

        Self::decompress_data_ref(
            data.data,
            data.compression,
            &mut self.decompress_buf,
            &self.custom_decompression,
        )?;

        Ok(&self.decompress_buf)
    }

    /// Returns the `timestamp` located within the region header for a specific chunk.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionReader, current_timestamp};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let timestamp = region.timestamp(2, 8)?;
    /// let one_week: u32 = 604_800;
    /// let recently_updated = timestamp > (current_timestamp() - one_week);
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn timestamp(&self, x: u8, z: u8) -> Result<u32, McaError> {
        let chunk_offset = SECTOR_SIZE + header_offset(x, z);
        let timestamp_data = match self.data.get(chunk_offset..chunk_offset + 4) {
            Some(timestamp) => timestamp,
            None => return Err(McaError::MissingTimestamp),
        };

        Ok(u32::from_be_bytes(
            timestamp_data
                .try_into()
                .expect("Failed to create timestamp"),
        ))
    }

    /// Decompresses `data` into `buf`, selecting a different decompression scheme depending on the `compression` byte
    ///
    /// If the byte doesn't match any of the *[vanilla minecraft compressions](https://minecraft.wiki/w/Region_file_format#Payload)*, it will call [`CustomDecompression::decompress`].  
    ///
    /// By default, [`CustomDecompression`] is set to `()` and will immediately return [`CompressionError::Unsupported`] if the byte doesn't match anything.  
    pub fn decompress_data_ref(
        data: CompressedChunk,
        compression: Compression,
        buf: &mut Vec<u8>,
        custom_decompression: &D,
    ) -> Result<(), CompressionError> {
        use std::io::Read;

        let data = data.as_ref();
        match compression {
            Compression::Gzip => {
                let mut g = flate2::read::GzDecoder::new(data);
                g.read_to_end(buf).map_err(CompressionError::Gzip)?;
            }
            Compression::ZLib => {
                let mut z = flate2::read::ZlibDecoder::new(data);
                z.read_to_end(buf).map_err(CompressionError::ZLib)?;
            }
            Compression::None => buf.extend(data),
            Compression::Lz4 => {
                let mut l = lz4_java_wrc::Lz4BlockInput::new(data);
                l.read_to_end(buf).map_err(CompressionError::LZ4)?;
            }
            Compression::Custom((_, id)) => {
                custom_decompression.decompress(data, &id, buf)?;
            }
        }

        Ok(())
    }

    /// Returns all chunk coordinates where data exists within this region.  
    ///
    /// ## Example
    /// ```
    /// # use mca::RegionReader;
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let generated_chunks = region.generated_chunks()?;
    /// for ((x, z)) in generated_chunks {
    ///     let chunk = region.chunk(x, z)?
    ///         .expect("Chunk must exist since we iterate over generated chunks");
    /// }
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn generated_chunks(&self) -> Result<Vec<(u8, u8)>, McaError> {
        let mut chunks = Vec::with_capacity(REGION_SIZE * REGION_SIZE);

        for x in 0..(REGION_SIZE as u8) {
            for z in 0..(REGION_SIZE as u8) {
                if self.sector_offset(x, z)?.is_some() {
                    chunks.push((x, z));
                }
            }
        }

        Ok(chunks)
    }

    /// Returns how many chunks has been generated / has data within this region.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionReader, REGION_SIZE};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let mut region = RegionReader::new(REGION)?;
    ///
    /// let chunk_count = region.chunk_count()?;
    /// println!("{}/{} chunks generated", chunk_count, REGION_SIZE * REGION_SIZE);
    ///
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn chunk_count(&self) -> Result<u16, McaError> {
        let mut count = 0;

        for x in 0..(REGION_SIZE as u8) {
            for z in 0..(REGION_SIZE as u8) {
                if self.sector_offset(x, z)?.is_some() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Creates a [`RegionIter`] that iterates over all generated chunks within a region.  
    ///
    /// Makes it easy to generate over all valid chunks instead of having to manually index into all chunks,
    /// and then having to check if the chunk exists or not and only then use it.  
    ///
    /// ## Example
    /// ```
    /// # use mca::{RegionReader, REGION_SIZE};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// # let mut region = RegionReader::new(REGION)?;
    /// let mut iter = region.iter()?;
    /// while let Some(((x, z), chunk)) = iter.next_available_chunk()? {
    ///     println!("({x}, {z}) is {} bytes big", chunk.len());
    /// }
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn iter(&'a self) -> Result<RegionIter<'a, D>, McaError> {
        RegionIter::new(self)
    }

    /// Reads all chunks in this [`RegionReader`], decompresses it and sets it inside a [`RegionWriter`]
    ///
    /// Useful if you want to read in a region file and modify it, see example below on how.  
    ///
    /// ## Example
    /// ```ignore
    /// # // again we got a fucking stack overflow here but this code runs in a test so were like fine
    /// # use mca::{RegionReader, Compression};
    /// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
    /// let region = RegionReader::new(REGION)?;
    /// // we give `()` to not specify any custom compression
    /// let mut writer = region.into_writer(())?;
    ///
    /// // switch the compression on chunk 8, 24 to Lz4
    /// if let Some(chunk) = writer.chunk_mut(8, 24)? {
    ///     chunk.compression = Compression::Lz4;
    /// }
    ///
    /// let mut file = Vec::new();
    /// writer.write(&mut file)?;
    /// # Ok::<(), mca::McaError>(())
    /// ```
    pub fn into_writer<C: CustomCompression>(
        &self,
        custom_compression: C,
    ) -> Result<RegionWriter<C>, McaError> {
        let mut w = RegionWriter::new_with_compression(custom_compression);

        for (x, z) in ChunkIter::new() {
            if let Some(data) = self.chunk_data(x, z)? {
                let mut uncompressed = Vec::with_capacity(256_000);
                RegionReader::decompress_data_ref(
                    data.data,
                    data.compression.clone(),
                    &mut uncompressed,
                    &self.custom_decompression,
                )?;

                w.set_chunk(x, z, uncompressed, data.compression)?;
            }
        }

        Ok(w)
    }
}

impl<D: CustomDecompression + Debug> Debug for RegionReader<'_, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RegionReader {{ data_len: {}, decompression_buf: {} len/{} capacity, custom_decompression: {:?} }}",
            self.data.len(),
            self.decompress_buf.len(),
            self.decompress_buf.capacity(),
            self.custom_decompression
        )
    }
}

impl Debug for ChunkDataRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ChunkDataRef {{ data_len: {}, compression: {:?} }}",
            self.data.as_ref().len(),
            self.compression
        )
    }
}

impl Debug for CompressedChunk<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CompressedChunk: {{ len: {} }}", self.0.len())
    }
}

impl From<RegionReader<'_, ()>> for RegionWriter<()> {
    fn from(value: RegionReader<'_, ()>) -> Self {
        value.into_writer(()).unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::{ChunkIter, McaError, REGION_SIZE, RegionReader, RegionWriter, regions::*};
    use na_nbt::{CompoundRef, ValueRef};
    use rayon::iter::{IntoParallelIterator, ParallelIterator};

    fn region() -> RegionReader<'static> {
        RegionReader::new(FULL).unwrap()
    }

    #[test]
    fn read_entire_region() -> Result<(), McaError> {
        let mut region = region();

        for (x, z) in ChunkIter::new() {
            if let Some(chunk) = region.chunk(x as u8, z as u8)? {
                std::hint::black_box(chunk);
            }
        }

        Ok(())
    }

    #[test]
    fn read_entire_region_parallel() -> Result<(), McaError> {
        let region = region();

        let sizes: Vec<usize> = (0..(REGION_SIZE * REGION_SIZE))
            .into_par_iter()
            .map(|s| {
                let chunk = match region.chunk_data((s % 32) as u8, (s / 32) as u8)? {
                    Some(c) => c,
                    None => return Ok(0),
                };

                let mut uncompressed = Vec::new();
                RegionReader::decompress_data_ref(
                    chunk.data,
                    chunk.compression,
                    &mut uncompressed,
                    &(),
                )?;

                Ok::<usize, McaError>(uncompressed.len())
            })
            .collect::<Result<Vec<usize>, McaError>>()?;

        assert_eq!(sizes.len(), REGION_SIZE * REGION_SIZE);

        Ok(())
    }

    #[test]
    fn chunk_count() -> Result<(), McaError> {
        let region = region();

        let count = region.chunk_count()?;
        let generated = region.generated_chunks()?;

        assert_eq!(count, generated.len() as u16);

        Ok(())
    }

    #[test]
    fn read_into_nbt() -> Result<(), McaError> {
        use na_nbt::{
            BE, read_borrowed,
            tag::{Int, String},
        };

        let mut region = region();

        let chunk = region.chunk(0, 0)?.unwrap();

        let nbt = read_borrowed::<BE>(chunk).unwrap();
        let nbt = nbt.root();
        let data_version = nbt.get_::<Int>("DataVersion").unwrap();
        let status = nbt.get_::<String>("Status").unwrap().to_utf8_string();

        assert!(data_version > 0);
        assert_eq!(status, "minecraft:full");

        Ok(())
    }

    #[test]
    fn valid_timestamp() -> Result<(), McaError> {
        let region = region();

        let time = region.timestamp(0, 0)?;
        assert!(time > 0);

        // this sometimes just fail, no clue how since saved regions cant be past right now
        // assert!(time < crate::write::current_timestamp() + 86_400);

        Ok(())
    }

    #[test]
    fn all_regions() -> Result<(), McaError> {
        use na_nbt::{
            BE, read_borrowed,
            tag::{Compound, Int},
        };
        use std::fs::{read, read_dir};

        let regions = read_dir("./data")?;

        for file in regions {
            let file = file?;

            // uhm, maybe dont use the corrupt one for this edge case
            if file.file_name().to_string_lossy() == "corrupt.mca" {
                continue;
            }

            let region_data = read(file.path())?;
            let region = RegionReader::new(&region_data)?;
            let mut iter = region.iter()?;

            while let Some((_, chunk)) = iter.next_available_chunk()? {
                let nbt = read_borrowed::<BE>(&chunk).unwrap();
                let root = nbt.root();
                // xPos & zPos is one of the only fields that has remained since 1.2.1
                // in some middle versions, these fields are nested in a Level compound
                let _ = match root.get_::<Int>("xPos") {
                    Some(x) => x,
                    None => root
                        .get_::<Compound>("Level")
                        .unwrap()
                        .get_::<Int>("xPos")
                        .unwrap(),
                };
                let _ = match root.get_::<Int>("zPos") {
                    Some(x) => x,
                    None => root
                        .get_::<Compound>("Level")
                        .unwrap()
                        .get_::<Int>("zPos")
                        .unwrap(),
                };
            }
        }

        Ok(())
    }

    #[test]
    fn multiple_roundtrips() -> Result<(), McaError> {
        let mut region = region();

        let mut w = RegionWriter::new();
        for (x, z) in ChunkIter::new() {
            let data = region.chunk_data(x, z)?.unwrap();
            let compression = data.compression.clone();
            let uncomp = region.decompress_to_internal_buffer(data)?;

            w.set_chunk(x, z, uncomp.to_vec(), compression)?;
        }
        let mut current = Vec::new();
        w.write(&mut current)?;

        // a fully written, compressed region
        let compare_data = current.clone();

        for i in 0..10 {
            let mut region = RegionReader::new(&current)?;
            let mut w = RegionWriter::new();

            for (x, z) in ChunkIter::new() {
                let data = region.chunk_data(x, z)?.unwrap();
                let compression = data.compression.clone();
                let uncomp = region.decompress_to_internal_buffer(data)?;

                w.set_chunk(x, z, uncomp.to_vec(), compression)?;
            }

            let mut cmp = Vec::with_capacity(current.len());
            w.write(&mut cmp)?;

            // cant compare raw data since the timestamps within are different
            // could handle PackedChunks directly and control the timestamp too
            assert_eq!(compare_data.len(), cmp.len(), "{i}");
            current = cmp;
        }

        Ok(())
    }

    #[test]
    fn reader_to_writer() -> Result<(), McaError> {
        use na_nbt::{BE, Writable, read_borrowed, read_owned, tag};

        let region = region();
        let mut writer = region.into_writer(())?;

        if let Some(chunk) = writer.chunk_mut(8, 24)? {
            let mut nbt = read_owned::<BE, BE>(&chunk.data).unwrap();
            let last_update = nbt.get_mut_::<tag::Long>("LastUpdate").unwrap();
            last_update.set(5555);

            let bytes = nbt.write_to_vec::<BE>();
            chunk.data = bytes;
        }

        let mut file = Vec::new();
        writer.write(&mut file)?;

        let mut new_region = RegionReader::new(&file)?;
        let chunk = new_region.chunk(8, 24)?.unwrap();
        let nbt = read_borrowed::<BE>(&chunk).unwrap();
        let new_last_update = nbt.root().get_::<tag::Long>("LastUpdate").unwrap();

        assert_eq!(new_last_update, 5555);

        Ok(())
    }

    #[test]
    fn empty_reader() -> Result<(), McaError> {
        let region = RegionReader::new(EMPTY)?;

        // iterator shouldnt yield anything
        let mut iterations = 0;
        let mut iter = region.iter()?;
        while let Some(_) = iter.next_available_chunk()? {
            iterations += 1;
        }

        assert_eq!(iterations, 0);
        assert_eq!(region.chunk_count()?, 0);
        assert_eq!(region.timestamp(30, 9)?, 0);

        Ok(())
    }

    #[test]
    fn corrupt_chunk() -> Result<(), McaError> {
        let mut region = RegionReader::new(CORRUPT)?;

        assert!(region.chunk_count()? > 0);
        assert!(region.chunk(4, 16).is_err());

        Ok(())
    }

    // enable "-- --nocapture" on the tests to view these
    #[test]
    fn debug() -> Result<(), McaError> {
        let r = RegionReader::new(FULL)?;
        println!("{r:?}");
        println!("{:?}", r.chunk_data(8, 1)?.unwrap());
        println!("{:?}", r.chunk_data(31, 4)?.map(|c| c.data).unwrap());
        Ok(())
    }
}
