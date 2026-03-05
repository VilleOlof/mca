use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mca::{REGION_SIZE, RegionReader};

const REGION_PATH: &'static str = "data/full.mca";

fn mca(r: &RegionReader, count: usize) {
    for i in 0..count {
        let (x, z) = (i / REGION_SIZE, i % REGION_SIZE);
        let chunk = r.chunk_data(x as u8, z as u8).unwrap();
        black_box(chunk);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let region = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("get_chunk_data/data/full_region");

    let region = RegionReader::new(&region).unwrap();

    for num in [1, 64, 512, 1024] {
        group.bench_function(format!("{num}_chunks"), |b| b.iter(|| mca(&region, num)));
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
