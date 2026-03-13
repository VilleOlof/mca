use crate::{CustomDecompression, McaError, REGION_SIZE, RegionReader};

/// A custom iterator that iterates over all given chunk coordinates and gets their chunk data.  
///
/// Assumes that `chunks` is filled with generated, valid chunks.  
#[derive(Debug, Clone)]
pub struct RegionIter<'a, D: CustomDecompression> {
    region: &'a RegionReader<'a, D>,
    current: usize,
    chunks: Vec<(u8, u8)>,
}

/// A chunks raw nbt data and it's coordinates
pub type AvailableChunk = ((u8, u8), Vec<u8>);

impl<'a, D: CustomDecompression> RegionIter<'a, D> {
    /// Creates a new [`RegionIter`] that iterates over all generated chunks within a region.  
    ///
    /// Look at the documentation on [`RegionReader::iter`] for examples
    pub fn new(region: &'a RegionReader<'a, D>) -> Result<Self, McaError> {
        let generated_chunks = region.generated_chunks()?;
        Ok(Self {
            region,
            current: 0,
            chunks: generated_chunks,
        })
    }

    /// Iterates the iterator and returns the next generated chunk within the region.  
    ///
    /// Look at the documentation on [`RegionReader::iter`] for examples
    pub fn next_available_chunk(&mut self) -> Result<Option<AvailableChunk>, McaError> {
        if self.current == self.chunks.len() {
            return Ok(None);
        }

        let (x, z) = self.chunks[self.current];
        let data = self
            .region
            .chunk_data(x, z)?
            .ok_or(McaError::InvalidRegionIterChunk(self.chunks.clone(), x, z))?;
        let mut buf = Vec::new();
        RegionReader::decompress_data_ref(
            data.data,
            data.compression,
            &mut buf,
            &self.region.custom_decompression,
        )?;

        self.current += 1;
        Ok(Some(((x, z), buf)))
    }

    /// Returns the inner region that is referenced in this iter
    pub fn region(&self) -> &'a RegionReader<'a, D> {
        self.region
    }
}

/// A simple iterator that iterates over all chunk coordinates in a region.  
///
/// - x = 0..=31  
/// - z = 0..=31
///
/// ## Example
/// ```no_run
/// # // stack overflow?
/// # use mca::{ChunkIter, RegionReader};
/// # const REGION: &'static [u8] = include_bytes!("../data/full.mca");
/// # let mut region = RegionReader::new(REGION)?;
/// for (x, z) in ChunkIter::new() {
///     if let Some(chunk) = region.chunk(x, z)? {
///         // ...
///     }
/// }
/// # Ok::<(), mca::McaError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIter(u16);
impl ChunkIter {
    const MAX: usize = REGION_SIZE * REGION_SIZE;

    /// Creates a new chunk coordinate iterator
    pub fn new() -> Self {
        ChunkIter(0)
    }
}

impl Default for ChunkIter {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for ChunkIter {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == Self::MAX as u16 {
            None
        } else {
            let coordinate = (
                (self.0 / REGION_SIZE as u16) as u8,
                (self.0 % REGION_SIZE as u16) as u8,
            );
            self.0 += 1;
            Some(coordinate)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        McaError, REGION_SIZE, RegionReader,
        iter::ChunkIter,
        regions::{FULL, HALF},
    };

    #[test]
    fn iter_all_generated() -> Result<(), McaError> {
        let region = RegionReader::new(FULL)?;

        let generated = region.chunk_count()?;
        let mut iterations = 0;

        let mut iter = region.iter()?;
        while let Some(_) = iter.next_available_chunk()? {
            iterations += 1;
        }

        assert_eq!(generated, iterations);

        Ok(())
    }

    #[test]
    fn iter_half() -> Result<(), McaError> {
        let region = RegionReader::new(HALF)?;

        let mut iterations = 0;

        let mut iter = region.iter()?;
        while let Some(_) = iter.next_available_chunk()? {
            iterations += 1;
        }

        assert_ne!(iterations, REGION_SIZE * REGION_SIZE);

        Ok(())
    }

    #[test]
    fn chunk_iter() -> Result<(), McaError> {
        let mut region = RegionReader::new(FULL)?;

        let mut iterations = 0;
        for (x, z) in ChunkIter::new() {
            let chunk = region.chunk(x, z)?;
            std::hint::black_box(chunk);
            iterations += 1;
        }

        assert_eq!(iterations, region.chunk_count()?);

        Ok(())
    }
}
