use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mca::ChunkIter;

const REGION_PATH: &'static str = "data/full.mca";

fn old_mca(r: &mut mca::RegionReader) {
    let mut w = old_mca::RegionWriter::new();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.chunk_data(x as u8, z as u8).unwrap() {
            let comp_type = chunk.compression.clone();
            let decompressed = r.decompress_to_internal_buffer(chunk).unwrap();

            w.push_chunk_with_compression(
                decompressed,
                (x as u8, z as u8),
                old_mca::CompressionType::from_u8(comp_type.to_u8()),
            )
            .unwrap();
        }
    }

    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();
}

fn mca(r: &mut mca::RegionReader) {
    let mut w = mca::RegionWriter::new();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.chunk_data(x as u8, z as u8).unwrap() {
            let comp_type = chunk.compression.clone();
            let decompressed = r.decompress_to_internal_buffer(chunk).unwrap();

            w.set_chunk(x as u8, z as u8, decompressed.to_vec(), comp_type)
                .unwrap();
        }
    }

    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let region_len = std::fs::read(REGION_PATH).unwrap().len();

    let mut group = c.benchmark_group("write/data/full_region");
    group.throughput(Throughput::Bytes(region_len as u64));
    group.measurement_time(std::time::Duration::from_secs(60));

    let region = std::fs::read(REGION_PATH).unwrap();

    let mut r = mca::RegionReader::new(&region).unwrap();
    group.bench_function("old_mca", |b| b.iter(|| old_mca(&mut r)));

    let mut r = mca::RegionReader::new(&region).unwrap();
    group.bench_function("mca", |b| b.iter(|| mca(&mut r)));

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
