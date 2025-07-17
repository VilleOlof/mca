use std::{
    collections::HashMap,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{chunk::PendingChunk, CompressionType, McaError, REGION_SIZE, SECTOR_SIZE};

/// A writer used to write chunks to a region (`mca`) file.  
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionWriter {
    chunks: Vec<PendingChunk>,
}

impl RegionWriter {
    /// Gets the current time in unix epoch
    fn get_current_timestamp() -> u32 {
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
        since_the_epoch.to_be()
    }

    /// Creates a new region writer
    pub fn new() -> RegionWriter {
        RegionWriter { chunks: vec![] }
    }

    /// Pushes a raw chunk into the writer  
    /// Defaults to `LZ4` compression, use [`push_chunk_with_compression`] for other compression types.  
    ///
    /// Timestamp will be current time since [`UNIX_EPOCH`], use [`push_pending_chunk`] to override it.  
    pub fn push_chunk<B>(&mut self, raw_data: &[u8], coordinate: (B, B)) -> Result<(), McaError>
    where
        B: Into<u8>,
    {
        let chunk = PendingChunk::new(
            &raw_data,
            CompressionType::LZ4,
            RegionWriter::get_current_timestamp(),
            coordinate,
        )?;
        self.chunks.push(chunk);

        Ok(())
    }

    /// Pushes a raw chunk into the writer  
    /// This specifies the compression type used  
    pub fn push_chunk_with_compression<B>(
        &mut self,
        raw_data: &[u8],
        coordinate: (B, B),
        compression_type: CompressionType,
    ) -> Result<(), McaError>
    where
        B: Into<u8>,
    {
        let chunk = PendingChunk::new(
            &raw_data,
            compression_type,
            RegionWriter::get_current_timestamp(),
            coordinate,
        )?;
        self.chunks.push(chunk);

        Ok(())
    }

    /// Just pushes a [`PendingChunk`] to the writer
    pub fn push_pending_chunk(&mut self, chunk: PendingChunk) {
        self.chunks.push(chunk)
    }

    /// Writes all chunks into one region file.  
    ///
    /// ## Example
    /// ```ignore
    /// use mca::{RegionWriter};
    ///
    /// let mut writer = RegionWriter::new();
    ///
    /// // Push some chunk data
    /// // ...
    ///
    /// let mut buf: Vec<u8> = vec![];
    /// writer.write(&mut buf).unwrap();
    ///
    /// std::fs::File::write("r.0.0.mca", &buf).unwrap();
    /// ```
    pub fn write<'a, W>(&self, w: &'a mut W) -> Result<(), McaError>
    where
        W: Write,
    {
        // payload prepping, needed for location header, hence it first
        let mut chunk_offsets: HashMap<(u8, u8), usize> = HashMap::new();
        let mut chunk_map: HashMap<(u8, u8), &PendingChunk> = HashMap::new(); // dont know the perf hit for this but this can for sure be removed

        let mut curr_chunk_offset: usize = SECTOR_SIZE * 2; // init pos for chunks
        let mut payloads: Vec<u8> = vec![];

        for chunk in self.chunks.iter() {
            let len_b = (chunk.compressed_data.len() as u32 + 1).to_be_bytes(); // this little +1 accounts for the compression byte
            let len = [len_b[0], len_b[1], len_b[2], len_b[3]];

            let compression = chunk.compression.to_u8();

            let mut payload_len = 0;
            payload_len += payloads.write(&len)?;
            payload_len += payloads.write(&[compression])?;
            payload_len += payloads.write(&chunk.compressed_data)?;

            // pad the chunk so its always in sector chunks
            let remaining = SECTOR_SIZE - (payload_len % SECTOR_SIZE);
            let padding = std::iter::repeat(0u8).take(remaining).collect::<Vec<u8>>();
            payload_len += payloads.write(&padding)?;

            chunk_offsets.insert(chunk.coordinate, curr_chunk_offset);
            chunk_map.insert(chunk.coordinate, chunk);

            // offset it by current + how many bytes we just wrote
            curr_chunk_offset = curr_chunk_offset + payload_len;
        }

        // location header
        for x in 0..REGION_SIZE {
            for z in 0..REGION_SIZE {
                let offset = match chunk_offsets.get(&(z as u8, x as u8)) {
                    Some(offset) => offset,
                    None => {
                        w.write(&[0, 0, 0, 0])?;
                        continue;
                    }
                };

                let chunk = chunk_map.get(&(z as u8, x as u8)).unwrap(); // handle this unwrap but this shouldnt be possible when we have the above statement

                let offset_bytes = {
                    let be = ((*offset / SECTOR_SIZE) as u32).to_be_bytes();
                    [be[1], be[2], be[3]]
                };

                w.write(&offset_bytes)?;

                let sector_count = ((chunk.compressed_data.len() + 4 + 1) as f32
                    / SECTOR_SIZE as f32)
                    .ceil() as u8;

                w.write(&[sector_count])?;
            }
        }

        // timestamp header
        for x in 0..REGION_SIZE {
            for z in 0..REGION_SIZE {
                match &self.chunks.get(x * REGION_SIZE + z) {
                    Some(chunk) => {
                        let timestamp = {
                            let b = chunk.timestamp.to_be_bytes();
                            [b[0], b[1], b[2], b[3]]
                        };
                        w.write(&timestamp)?
                    }
                    None => w.write(&[0, 0, 0, 0])?,
                };
            }
        }

        w.write(&payloads)?;

        w.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegionReader;

    const REGION: &[u8] = include_bytes!("../benches/r.0.0.mca");

    #[test]
    fn round_trip() {
        let region = RegionReader::new(REGION).unwrap();
        let mut writer = RegionWriter::new();

        for (idx, chunk) in region.iter().enumerate() {
            let chunk = match chunk.unwrap() {
                Some(data) => data,
                None => continue,
            };

            let data = chunk.decompress().unwrap();
            writer
                .push_chunk_with_compression(
                    &data,
                    ((idx % REGION_SIZE) as u8, (idx / REGION_SIZE) as u8),
                    CompressionType::Zlib,
                )
                .unwrap();
        }

        let mut buf = vec![];
        writer.write(&mut buf).unwrap();

        let new_region = RegionReader::new(&buf).unwrap();
        let chunk = new_region.get_chunk(0, 0).unwrap().unwrap();

        let data = chunk.decompress().unwrap();
        let _ = sculk::chunk::Chunk::from_bytes(&data).unwrap();
    }
}
