# mca

> Reader/Writer for Minecraft regions files *(.mca)*

This library fully implements the [Region File Format](https://minecraft.wiki/w/Region_file_format) from Minecraft **1.2.1+**  
Both reading and writing regions in anyway you like.  

Notably this library implements all compressions in vanilla (`GZip`, `Zlib`, `Uncompressed`, `LZ4`).  
As well as custom compression algorithms for both compressing and decompressing *(see below for examples)*.  

It's also ***the fastest*** `.mca` Rust library *(see benchmarks below)*.  

## Installation

Add this to your `cargo.toml`

```toml
[dependencies]
mca = "2.1"
```

## Quick start

### Read

Reading region files is a very simple process.  
Chunks are automatically decompressed *(tho non-compressed data can be get via `.chunk_data(x, z)`)*.  
You only have to handle if the chunk is generated or not (`Some(chunk)` or `None`).

```rust ignore
use mca::RegionReader;

let file = std::fs::read("r.0.0.mca")?;

let mut region = RegionReader::new(&file)?;
let chunk = region.chunk(5, 12)?;

if let Some(chunk) = chunk {
    // here you can bring your own nbt library to read the data
    // i recommend either `simdnbt` or `na_nbt`
}

Ok::<(), mca::McaError>(())
```

### Write

Writing is also quite easy, just pass it the coordinates, the nbt in bytes and what compression to use.  
Unsure what compression to use? use `Compression::default()` which will use `Zlib`, the default in Minecraft.

More advanced writing can be made with `RegionWriter::write_packed`.  

```rust ignore
use mca::{RegionWriter, Compression};

let mut region = RegionWriter::new();

// `Vec::new()` would be your nbt data in bytes
region.set_chunk(5, 12, Vec::new(), Compression::default())?;

let mut file = std::fs::File::create("r.0.0.mca")?;
region.write(&mut file)?;

Ok::<(), mca::McaError>(())
```

### Iterators

Often you might want to iterate over all chunk coordinates or all generated chunks within a region.  
This library comes with two different iterators to make this as easy as possible.  

```rust ignore
use mca::{RegionReader, ChunkIter};

let file = Vec::new();
let mut region = RegionReader::new(&file)?;

// Iterate over all generated chunks within a region
let mut iter = region.iter()?;
while let Some(chunk) = iter.next_available_chunk()? {
    // here you can convert `chunk` to nbt
}

// Iterate over all chunk coordinates within a region
for (x, z) in ChunkIter::new() {
    if let Some(chunk) = region.chunk(x, z)? {
        // here you can convert `chunk` to nbt
    }
}

Ok::<(), mca::McaError>(())
```

### Custom Compression

One thing that I haven't seen in any other `.mca` library is full support of the format.  
This includes the rather obscure feature of using custom compression schemes for chunks.  
The [wiki](https://minecraft.wiki/w/Region_file_format#Payload) states that a compression byte of `127` indicates a custom compression.  
Where it's then followed by a prefixed string containing the **id** of the compression algorithm.  

Below is a tiny example, for a fully working **lzma2** example. Look at [custom_compression.rs](https://github.com/VilleOlof/mca/blob/main/src/custom_compression.rs) and it's tests.  

Both `RegionReader` and `RegionWriter` defaults to `()` as its custom compression scheme.  
Which will return `Err(CompressionError::Unsupported)` if it's ever called.  

One single implementation can support multiple compression schemes,  
as one of the argument is the **id** itself. So you can match on it.  

```rust ignore
use mca::{CustomCompression, CustomDecompression, RegionWriter, RegionReader, CompressionError};

struct MyCompressionScheme;
impl CustomCompression for MyCompressionScheme {
    fn compress(&self, data: &[u8], algorithm: &str, out: &mut Vec<u8>) -> Result<(), CompressionError> {
        todo!("write the actual implementation")
    }
}

impl CustomDecompression for MyCompressionScheme {
    fn decompress(&self, data: &[u8], algorithm: &str, out: &mut Vec<u8>) -> Result<usize, CompressionError> {
        todo!("write the actual implementation")
    }
}

let mut reader = RegionReader::new_with_decompression(&Vec::new(), MyCompressionScheme)?;
let mut writer = RegionWriter::new_with_compression(MyCompressionScheme);

Ok::<(), mca::McaError>(())
```

### Parallel

The `.write` function on `RegionWriter` already compresses all chunks in parallel by default.  
This can be disabled via disabling default features in your `cargo.toml`
```toml
mca = { version = "2.1", default-features = false }
```

But reading chunks in parallel requires a bit more manual work for you to do.  
Mainly we can't use the normal `.chunk(x, z)` or `.decompress(chunk)` functions on the region.  
Both of these make use of some internal buffers to speed single threaded performance quite a bit.  
So we have to get each chunks data and decompress it ourself.  

```rust ignore
use mca::{RegionReader, REGION_SIZE};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

let region = RegionReader::new(&Vec::new())?;

let chunks = (0..(REGION_SIZE * REGION_SIZE)).into_par_iter().map(|c| {
    // Convert 0..1024 into x and z coordinates
    let data = match region.chunk_data((c % REGION_SIZE) as u8, (c / REGION_SIZE) as u8)? {
        Some(c) => c,
        None => return Ok(Vec::new())
    };

    let mut uncompressed = Vec::new();
    RegionReader::decompress_data_ref(
        data.data,
        data.compression,
        &mut uncompressed,
        &() // No custom compression
    )?;

    Ok(uncompressed)

}).collect::<Result<Vec<Vec<u8>>, McaError>>()?;

Ok::<(), mca::McaError>(())
```

### Reader to Writer

Sometimes you might want to read in a region file and modify it's existing data and write it back.  
To make this easier you can use `into_writer`, it converts a `ReginoReader` to a `RegionWriter`.  
And it only ever decompresses data that you modify with `set_chunk` or `chunk_mut`.  

Any unmodified chunk will remain compress and untouched, ensuring maximal performance.  

Important to note that you can use `PackedChunk`s with `RegionWriter::write_packed`,  
to gain even more control on the data written and how you handle the compression etc.  

```rust ignore
use mca::{RegionReader, Compression};

let file = Vec::new();
let region = RegionReader::new(&file)?;

// we pass `()` to specify no custom compression
let mut writer = region.into_writer(())?;

// change the chunks compression to Lz4
// here you would access `data.buf` and modify its nbt
if let Some(chunk) = writer.chunk_mut(1, 4)? {
    // Again, we pass &() to specify no custom compression
    let data = chunk.data.as_uncompressed_mut(&())?;
    data.compression = Compression::Lz4;
}

let mut buf = Vec::new();
writer.write(&mut buf)?;

Ok::<(), mca::McaError>(())
```

To find more examples and usage of the library, you can look at any tests  
at the bottom of any source file, `read.rs` and `write.rs` have some good examples in their tests.  

## Benchmarks

A [benchmark](https://github.com/VilleOlof/mca/blob/main/benches/compare.rs) comparing mca against all `.mca` parsers I could find.  
This is in **reading** a fully generated, zlib compressed [region file](https://github.com/VilleOlof/mca/blob/main/data/full.mca).  
As some of these don't support writing region files.  

| Library                                               | Throughput   | Ms *(mean)* |
| ----------------------------------------------------- | ------------ | ----------- |
| [mca *(2.0.0)*](https://crates.io/crates/mca)         | 310.71 MiB/s | 24.440 ms   |
| [anvil-nbt](https://crates.io/crates/anvil-nbt)       | 261.26 MiB/s | 29.066 ms   |
| [mca *(1.1.0)*](https://crates.io/crates/mca/1.1.0)   | 216.61 MiB/s | 35.057 ms   |
| [mca-parser](https://crates.io/crates/mca-parser)     | 87.721 MiB/s | 86.567 ms   |
| [simple-anvil](https://crates.io/crates/simple-anvil) | 13.599 MiB/s | 558.41 ms   |

When it comes to writing regions, this library can write it at `147.71 MiB/s`,  
and or `51.374 ms` for it to write a filled region with 1024 chunks.  

*All benchmarks ran on a Hetzner AX52 dedicated server*

## Tests

Many of the tests are also some really good examples on how to properly use the crate.  
Like how to read chunks in parallel, how to easily iterate over chunks,  
How to write a custom compression implementation and more.

But to actually run the tests and doctests, refer to the commands below.  
As rayon can't be enabled for doctests  
and some compression libtests can take a while so we use release mode.  

```sh
# Run libtests
cargo test --lib --release

# Run doctests
cargo test --no-default-features --doc
```

There is about *50+* total tests that covers most if not all code in the crate.  
And plenty of different scenarios across 13 test region files.  
Spanning 8 different versions, different compression schemes and even corrupt regions.  

## Old versions

This library was fully rewritten in version `2.0.0`.  
And users who may have used this before in `1.x` versions may see  
quite the different exposed API, but the functionality is the same.

This rewrite was made partially for fun but also to support  
custom compression algorithms and more QoL functions.  

---

Made by an actual human with no AI involved