use crate::{chunk::RawChunk, compression::CompressionType, McaError, REGION_SIZE, SECTOR_SIZE};

/// A Minecraft region
///
/// This struct is just a wrapper around `&'a [u8]`   
/// but provides methods to get chunk byte slices & header data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionReader<'a> {
    data: &'a [u8],
}

impl<'a> RegionReader<'a> {
    /// Initializes a new region  
    /// Validates that the region size is at least the size of the header
    pub fn new(data: &'a [u8]) -> Result<RegionReader<'a>, McaError> {
        if data.len() < (SECTOR_SIZE * 2) {
            return Err(McaError::MissingHeader);
        }

        Ok(RegionReader { data })
    }

    /// Get the inner data of the region
    pub fn inner(&self) -> &'a [u8] {
        self.data
    }

    /// Get a offset depending on the chunk coordinates.  
    /// Used in getting byte offsets for chunk location & timestamp in headers
    #[inline(always)]
    pub fn chunk_offset(x: usize, z: usize) -> usize {
        assert!(x < REGION_SIZE);
        assert!(z < REGION_SIZE);

        4 * ((x & (REGION_SIZE - 1)) + (z & (REGION_SIZE - 1)) * REGION_SIZE)
    }

    /// Get a single [`RawChunk`] based of it's chunk coordinates relative to the region itself.  
    /// Will return [`None`] if chunk hasn't been generated yet.
    pub fn get_chunk(&'a self, x: usize, z: usize) -> Result<Option<RawChunk<'a>>, McaError> {
        // just so we dont have to call .len() more than needed, data len stays the same
        let data_len = self.data.len();

        let offset = RegionReader::chunk_offset(x, z);

        let chunk_location = match self.get_location(offset) {
            Some(loc) => loc,
            None => return Ok(None),
        };

        // fill the first byte when doing the 3 first chunk location offset
        let endians =
            u32::from_be_bytes([0, chunk_location[0], chunk_location[1], chunk_location[2]])
                as usize;

        let payload_offset: usize = endians * SECTOR_SIZE;

        if data_len < (payload_offset + 4) {
            return Err(McaError::InvalidChunkPayload(
                "Not enough data for chunk payload".to_string(),
            ));
        }

        #[cfg(feature = "unsafe")]
        let byte_length = u32::from_be_bytes(unsafe {
            [
                *self.data.get_unchecked(payload_offset),
                *self.data.get_unchecked(payload_offset + 1),
                *self.data.get_unchecked(payload_offset + 2),
                *self.data.get_unchecked(payload_offset + 3),
            ]
        }) as usize;

        #[cfg(not(feature = "unsafe"))]
        let byte_length = {
            let byte_length = self
                .data
                .get(payload_offset..payload_offset + 4)
                .ok_or(McaError::OutOfBoundsByte)?;

            let byte_length = [
                byte_length[0],
                byte_length[1],
                byte_length[2],
                byte_length[3],
            ];

            u32::from_be_bytes(byte_length) as usize
        };

        if data_len < payload_offset + byte_length {
            return Err(McaError::InvalidChunkPayload(
                "Not enough data for chunk bytes".to_string(),
            ));
        }

        let payload_offset = payload_offset + 4;

        #[cfg(feature = "unsafe")]
        let compression_type =
            CompressionType::from(unsafe { *self.data.get_unchecked(payload_offset) });

        #[cfg(not(feature = "unsafe"))]
        let compression_type = CompressionType::from(
            *self
                .data
                .get(payload_offset)
                .ok_or(McaError::OutOfBoundsByte)?,
        );

        let raw_data = &self.data[payload_offset + 1..payload_offset + byte_length];

        Ok(Some(RawChunk::new(raw_data, compression_type)))
    }

    #[cfg(feature = "unsafe")]
    /// Get the chunk payload location based off chunk coordinate byte offsets
    #[inline]
    pub fn get_location(&self, offset: usize) -> Option<[u8; 4]> {
        unsafe {
            let first = *self.data.get_unchecked(offset);
            let last = *self.data.get_unchecked(offset + 3);

            // Empty chunk locations, hasnt been generated if None
            if first == 0 && last == 0 {
                return None;
            }

            let loc = [
                first,
                *self.data.get_unchecked(offset + 1),
                *self.data.get_unchecked(offset + 2),
                last,
            ];

            Some(loc)
        }
    }

    #[cfg(not(feature = "unsafe"))]
    /// Get the chunk payload location based off chunk coordinate byte offsets
    #[inline]
    pub fn get_location(&self, offset: usize) -> Option<[u8; 4]> {
        let bytes = self.data.get(offset..offset + 4);

        if let Some(bytes) = bytes {
            if bytes[0] == 0 && bytes[3] == 0 {
                return None;
            }

            Some([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            None
        }
    }

    #[cfg(feature = "unsafe")]
    /// Get the timestamp big endian bytes for the chunk based off chunk coordinate byte offsets
    #[inline]
    pub fn get_timestamp(&self, offset: usize) -> [u8; 4] {
        unsafe {
            [
                *self.data.get_unchecked(SECTOR_SIZE + offset),
                *self.data.get_unchecked(SECTOR_SIZE + offset + 1),
                *self.data.get_unchecked(SECTOR_SIZE + offset + 2),
                *self.data.get_unchecked(SECTOR_SIZE + offset + 3),
            ]
        }
    }

    #[cfg(not(feature = "unsafe"))]
    /// Get the timestamp big endian bytes for the chunk based off chunk coordinate byte offsets
    #[inline]
    pub fn get_timestamp(&self, offset: usize) -> Result<[u8; 4], McaError> {
        let offset = SECTOR_SIZE + offset;

        let bytes = self
            .data
            .get(offset..offset + 4)
            .ok_or(McaError::OutOfBoundsByte)?;

        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// Converts the timestamp bytes to u32 unix epoch seconds
    #[inline]
    pub fn get_u32_timestamp(&self, timestamp_bytes: [u8; 4]) -> u32 {
        u32::from_be_bytes(timestamp_bytes)
    }

    pub fn iter(&'a self) -> RegionIter<'a> {
        RegionIter {
            region: self,
            index: 0,
        }
    }
}

/// An iterator over all chunks inside a region
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionIter<'a> {
    region: &'a RegionReader<'a>,
    index: usize,
}

impl<'a> RegionIter<'a> {
    /// The max size of chunks inside one region
    pub const MAX: usize = REGION_SIZE * REGION_SIZE;

    /// Get the chunk coordinate based off (index / [`RegionIter::MAX`])
    pub fn get_chunk_coordinate(index: usize) -> (usize, usize) {
        (index % REGION_SIZE, index / REGION_SIZE)
    }
}

impl<'a> Iterator for RegionIter<'a> {
    type Item = Result<Option<RawChunk<'a>>, McaError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < RegionIter::MAX {
            let (x, z) = RegionIter::get_chunk_coordinate(self.index);
            self.index += 1;

            let chunk = self.region.get_chunk(x, z);

            Some(chunk)
        } else {
            None
        }
    }
}
