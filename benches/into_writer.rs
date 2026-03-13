use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mca::write::PendingData;
use na_nbt::Writable;

const REGION_PATH: &'static str = "data/full.mca";

fn simple(r: &mut mca::RegionReader) {
    let w = r.into_writer(()).unwrap();
    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();
}

fn one_change(r: &mut mca::RegionReader) {
    let mut w = r.into_writer(()).unwrap();

    if let Some(chunk) = w.chunk_mut(5, 1).unwrap() {
        // turn off the light in the chunk
        let uncomp = chunk.data.as_uncompressed_mut(&()).unwrap();
        let mut nbt = na_nbt::read_owned::<na_nbt::BE, na_nbt::BE>(&uncomp.buf).unwrap();
        *nbt.get_mut_::<na_nbt::tag::Byte>("isLightOn").unwrap() = 0;
        chunk.data = PendingData::new_uncompressed(
            nbt.write_to_vec::<na_nbt::BE>(),
            uncomp.compression.clone(),
        );
    }

    let mut buf = Vec::new();
    w.write(&mut buf).unwrap();
}

#[allow(unused)]
fn criterion_benchmark(c: &mut Criterion) {
    let region_data = std::fs::read(REGION_PATH).unwrap();

    let mut group = c.benchmark_group("into_writer/data/full_region");
    group.throughput(Throughput::Bytes(region_data.len() as u64));

    let mut r = mca::RegionReader::new(&region_data).unwrap();

    group.bench_function("simple", |b| b.iter(|| simple(&mut r)));
    group.bench_function("one_change", |b| b.iter(|| one_change(&mut r)));

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
