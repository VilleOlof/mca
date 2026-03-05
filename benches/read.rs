use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mca::REGION_SIZE;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

const REGION_PATH: &'static str = "data/full.mca";

fn old_mca(buf: &[u8]) {
    let region = old_mca::RegionReader::new(&buf).unwrap();

    for x in 0..32 {
        for z in 0..32 {
            if let Some(chunk) = region.get_chunk(x, z).unwrap() {
                let chunk = chunk.decompress().unwrap();
                black_box(chunk);
            }
        }
    }
}

fn mca_data(buf: &[u8]) {
    let mut region = mca::RegionReader::new(&buf).unwrap();

    for x in 0..32 {
        for z in 0..32 {
            let chunk = region.chunk(x, z).unwrap();
            black_box(chunk);
        }
    }
}

fn mca_parallel() {
    let buf = std::fs::read(REGION_PATH).unwrap();
    let region = mca::RegionReader::new(&buf).unwrap();

    let _ = (0..(REGION_SIZE * REGION_SIZE)).into_par_iter().map(|s| {
        let chunk = region
            .chunk_data((s % 32) as u8, (s / 32) as u8)
            .unwrap()
            .unwrap();

        let mut uncompressed = Vec::new();
        mca::RegionReader::decompress_data_ref(
            chunk.data,
            chunk.compression,
            &mut uncompressed,
            &(),
        )
        .unwrap();

        black_box(uncompressed)
    });
}

fn mca_one_chunk(buf: &[u8]) {
    let mut region = mca::RegionReader::new(&buf).unwrap();
    let chunk = region.chunk(0, 0).unwrap();
    black_box(chunk);
}

fn criterion_benchmark(c: &mut Criterion) {
    let region = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("read/data/full_region");
    group.throughput(Throughput::Bytes(region.len() as u64));

    group.bench_function("old_mca", |b| b.iter(|| old_mca(&region)));
    group.bench_function("mca-data", |b| b.iter(|| mca_data(&region)));
    group.bench_function("mca-parallel", |b| b.iter(|| mca_parallel()));
    group.bench_function("mca-1_chunk", |b| b.iter(|| mca_one_chunk(&region)));

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
