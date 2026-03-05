use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mca::ChunkIter;

const REGION_PATH: &'static str = "data/full.mca";

// NOTE: only like 3/5 also implements a writer so cant really compare writing for all of them
// maybe worth making a smaller test with only those who can write.
//
// Also, all of these also include the decompression since how the crates does it
// and the backing crates for it is quite important to the timing.

fn old_mca(data: &[u8]) {
    let r = old_mca::RegionReader::new(data).unwrap();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.get_chunk(x as usize, z as usize).unwrap() {
            let data = chunk.decompress().unwrap();
            black_box(data);
        }
    }
}

fn mca(data: &[u8]) {
    let mut r = mca::RegionReader::new(data).unwrap();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.chunk(x as u8, z as u8).unwrap() {
            black_box(chunk);
        }
    }
}

fn mca_parser(data: &[u8]) {
    let r = mca_parser::Region::from_slice(data).unwrap();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.get_chunk(x as u32, z as u32).unwrap() {
            // this also converts to nbt but like, theres no fn to just decompress
            let data = chunk.parse().unwrap();
            black_box(data);
        }
    }
}

fn simple_anvil(_: &[u8]) {
    // you cant provide a buffer to this so have to read file io everytime
    let r = simple_anvil::region::Region::from_file(REGION_PATH.to_string());

    for (x, z) in ChunkIter::new() {
        let data = r.chunk_data(x as u32, z as u32).unwrap();
        black_box(data);
    }
}

fn anvil_nbt(_: &[u8]) {
    // again, cant provide an in memory buffer, grrr
    let r = anvil_nbt::anvil::access::Region::open(REGION_PATH).unwrap();

    for (x, z) in ChunkIter::new() {
        if let Some(chunk) = r.get_chunk_data(x as i32, z as i32).unwrap() {
            black_box(chunk);
        }
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let region = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("compare/data/full_region");
    group.throughput(Throughput::Bytes(region.len() as u64));

    group.bench_function("old_mca", |f| f.iter(|| old_mca(&region)));
    group.bench_function("mca", |f| f.iter(|| mca(&region)));
    group.bench_function("mca_parser", |f| f.iter(|| mca_parser(&region)));
    group.bench_function("simple_anvil", |f| f.iter(|| simple_anvil(&region)));
    group.bench_function("anvil_nbt", |f| f.iter(|| anvil_nbt(&region)));

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
