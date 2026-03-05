use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mca::{PackedChunk, REGION_SIZE, RegionReader, RegionWriter};

const REGION_PATH: &'static str = "data/full.mca";

fn mca(chunks: Vec<PackedChunk>, len: usize) {
    let mut writer = Vec::with_capacity(len);
    RegionWriter::<()>::write_packed(&mut writer, chunks).unwrap();
    black_box(writer);
}

fn criterion_benchmark(c: &mut Criterion) {
    let region = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("write_packed/data/full_region");

    let r = RegionReader::new(&region).unwrap();

    for num in [1, 64, 512, 1024] {
        let mut chunks = Vec::with_capacity(num);

        for i in 0..num {
            let (x, z) = ((i / REGION_SIZE) as u8, (i % REGION_SIZE) as u8);
            let chunk = r.chunk_data(x, z).unwrap().unwrap();

            let packed = PackedChunk {
                compressed: chunk.data.as_ref().to_vec(),
                compression: chunk.compression.clone(),
                chunk: (x, z),
                sector_size: RegionWriter::sector_size(
                    chunk.data.as_ref().len(),
                    &chunk.compression,
                ),
                timestamp: None,
            };
            chunks.push(packed);
        }

        group.bench_function(format!("{num}_chunks"), |b| {
            b.iter(|| mca(chunks.clone(), region.len()))
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
