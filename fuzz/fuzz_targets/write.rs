#![no_main]

use libfuzzer_sys::fuzz_target;
use mca::{Compression, RegionWriter};

fuzz_target!(|data: &[u8]| {
    let mut region = RegionWriter::new();
    if let Ok(_) = region.set_chunk(0, 0, data.to_vec(), Compression::default()) {
        let mut buf = Vec::new();
        let _ = region.write(&mut buf);
    }
});
