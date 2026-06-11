#![no_main]

use libfuzzer_sys::fuzz_target;
use mca::{ChunkIter, RegionReader};

fuzz_target!(|data: &[u8]| {
    if let Ok(mut region) = RegionReader::new(data) {
        for (x, z) in ChunkIter::new() {
            let _ = region.chunk(x, z);
        }
    }
});
