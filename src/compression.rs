/// Compression values used for writing chunks.  
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compression {
    Gzip,
    ZLib,
    None,
    Lz4,
    /// A namespaced string representing the algorithm used.  
    ///
    /// the value should be `127`(?)
    Custom((u8, String)),
}

impl Compression {
    /// The custom compression value
    // note that i **THINK** that all custom ids have 127 as their value, a bit unsure but Compression::new_custom defaults to this
    // since i think this is how the wiki says it: https://minecraft.wiki/w/Region_file_format#Payload
    pub const CUSTOM_COMPRESSION_VALUE: u8 = 127;

    /// Converts the [`Compression`] to it's `u8` variant.  
    ///
    /// Custom compressions returns their specified value and discards their id.  
    pub fn to_u8(&self) -> u8 {
        match self {
            Compression::Gzip => 1,
            Compression::ZLib => 2,
            Compression::None => 3,
            Compression::Lz4 => 4,
            Compression::Custom((val, _)) => *val,
        }
    }

    /// Converts a `u8` into it's [`Compression`] variant.  
    ///
    /// Anything other than the vanilla specified compression variants becomes a [`Compression::Custom`] variant with an empty id
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Compression::Gzip,
            2 => Compression::ZLib,
            3 => Compression::None,
            4 => Compression::Lz4,
            _ => Compression::Custom((val, String::new())),
        }
    }

    /// A namespaced string representing the algorithm used
    pub fn new_custom(id: impl Into<String>) -> Self {
        Compression::Custom((127, id.into()))
    }

    /// Returns the size of the compression as it appears in the region file format.  
    ///
    /// Accounts for the custom compression and its prefixed string
    pub fn size(&self) -> usize {
        // the compression byte (1) is always present + whatever custom id is included
        1 + match self {
            Compression::Custom((_, id)) => size_of::<u16>() + id.len(),
            _ => 0,
        }
    }
}

impl Default for Compression {
    fn default() -> Self {
        // zlib is the default for singeplayer worlds
        // multiplayer servers default to Lz4 nowdays
        Self::ZLib
    }
}

#[cfg(test)]
mod test {
    use crate::{
        McaError, RegionReader,
        regions::{LZ4, NONE, ZLIB},
    };

    use super::*;

    #[test]
    fn to_u8() {
        assert!(Compression::None.to_u8() < 127);
        assert!(Compression::ZLib.to_u8() < 127);
        assert!(Compression::new_custom("custom").to_u8() >= 127);
    }

    #[test]
    fn from_u8() {
        assert!(Compression::from_u8(2) != Compression::new_custom("custom"));
        assert!(Compression::from_u8(4) != Compression::new_custom("custom"));
    }

    // zlib, lz4 and none are the only used compression schemes used by modern Minecraft.
    // gzip is old and custom compression already have their own testing in `custom_compression.rs`.

    fn verify_chunk(data: &[u8]) {
        use na_nbt::{BE, ValueRef, read_borrowed, tag::String};

        let nbt = read_borrowed::<BE>(data).unwrap();
        let nbt = nbt.root();
        let status = nbt.get_::<String>("Status").unwrap().to_utf8_string();

        if !status.starts_with("minecraft:") {
            panic!("invalid status")
        }
    }

    #[test]
    fn zlib() -> Result<(), McaError> {
        let region = RegionReader::new(ZLIB)?;
        let mut iter = region.iter()?;

        while let Some((_, chunk)) = iter.next_available_chunk()? {
            verify_chunk(&chunk);
        }

        Ok(())
    }

    #[test]
    fn lz4() -> Result<(), McaError> {
        let region = RegionReader::new(LZ4)?;
        let mut iter = region.iter()?;

        while let Some((_, chunk)) = iter.next_available_chunk()? {
            verify_chunk(&chunk);
        }

        Ok(())
    }

    #[test]
    fn none() -> Result<(), McaError> {
        let region = RegionReader::new(NONE)?;
        let mut iter = region.iter()?;

        while let Some((_, chunk)) = iter.next_available_chunk()? {
            verify_chunk(&chunk);
        }

        Ok(())
    }

    #[test]
    fn size_zlib() {
        assert_eq!(Compression::ZLib.size(), 1);
    }

    #[test]
    fn size_none() {
        assert_eq!(Compression::None.size(), 1);
    }

    #[test]
    fn size_custom() {
        assert_eq!(
            Compression::new_custom("an id for a compression").size(),
            26
        );
    }
}
