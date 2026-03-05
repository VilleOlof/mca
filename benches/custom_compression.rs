use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mca::{
    ChunkIter, Compression, CompressionError, CustomCompression, CustomDecompression, REGION_SIZE,
    RegionReader, RegionWriter,
};

const REGION_PATH: &'static str = "data/full.mca";

struct Raw;
impl Raw {
    const ID: &'static str = "raw";

    fn compression() -> Compression {
        Compression::new_custom(Self::ID)
    }
}

impl CustomDecompression for Raw {
    fn decompress(
        &self,
        data: &[u8],
        algorithm: &str,
        out: &mut Vec<u8>,
    ) -> Result<usize, CompressionError> {
        if algorithm != Self::ID {
            return Err(CompressionError::Unsupported);
        }

        *out = data.to_vec();

        Ok(data.len())
    }
}

impl CustomCompression for Raw {
    fn compress(
        &self,
        data: Vec<u8>,
        algorithm: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), CompressionError> {
        if algorithm != Self::ID {
            return Err(CompressionError::Unsupported);
        }

        *out = data;

        Ok(())
    }
}

fn read_custom(r: &mut RegionReader<'_, Raw>, count: usize) {
    for i in 0..count {
        let (x, z) = (i / REGION_SIZE, i % REGION_SIZE);
        let chunk = r.chunk(x as u8, z as u8).unwrap();
        black_box(chunk);
    }
}

fn write_custom(data: Vec<((u8, u8), Vec<u8>)>, count: usize) {
    let mut w = RegionWriter::new_with_compression(Raw);

    for ((x, z), chunk) in data.into_iter().take(count) {
        w.set_chunk(x, z, chunk, Raw::compression()).unwrap();
    }

    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();

    black_box(buf);
}

fn convert_reader_to_custom(r: &mut RegionReader) -> Vec<u8> {
    let mut w = RegionWriter::new_with_compression(Raw);

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.chunk_data(x, z).unwrap() {
            let compression = chunk.compression.clone();
            let uncomp = r.decompress_to_internal_buffer(chunk).unwrap();
            w.set_chunk(x, z, uncomp.to_vec(), compression).unwrap();
        }
    }

    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();

    buf
}

fn convert_default_to_uncompressed_vec(r: &mut RegionReader) -> Vec<((u8, u8), Vec<u8>)> {
    let mut chunks = Vec::with_capacity(1024);

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.chunk(x, z).unwrap() {
            chunks.push(((x, z), chunk.to_vec()));
        }
    }

    chunks
}

fn criterion_benchmark(c: &mut Criterion) {
    let region = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("custom_compression/data/full_region");

    let mut region = RegionReader::new(&region).unwrap();
    let custom_region = convert_reader_to_custom(&mut region);

    let mut r = RegionReader::new_with_decompression(&custom_region, Raw).unwrap();

    let uncompressed_vec = convert_default_to_uncompressed_vec(&mut region);

    for num in [1, 64, 512, 1024] {
        group.bench_function(format!("read/{num}_chunks"), |b| {
            b.iter(|| read_custom(&mut r, num))
        });

        group.bench_function(format!("write/{num}_chunks"), |b| {
            b.iter(|| write_custom(uncompressed_vec.clone(), num))
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
