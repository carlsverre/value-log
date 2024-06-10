/// Compression type
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CompressionType {
    /// No compression
    None,

    /// LZ4 compression (speed-optimized)
    #[cfg(feature = "lz4")]
    Lz4,

    // TODO: compression level
    /// Zlib/DEFLATE compression (space-optimized)
    #[cfg(feature = "miniz")]
    Miniz,
}

impl From<CompressionType> for u8 {
    fn from(val: CompressionType) -> Self {
        match val {
            CompressionType::None => 0,

            #[cfg(feature = "lz4")]
            CompressionType::Lz4 => 1,

            #[cfg(feature = "miniz")]
            CompressionType::Miniz => 2,
        }
    }
}

impl TryFrom<u8> for CompressionType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),

            #[cfg(feature = "lz4")]
            1 => Ok(Self::Lz4),

            #[cfg(feature = "miniz")]
            2 => Ok(Self::Miniz),

            _ => Err(()),
        }
    }
}

impl std::fmt::Display for CompressionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "no compression",

                #[cfg(feature = "lz4")]
                Self::Lz4 => "lz4",

                #[cfg(feature = "miniz")]
                Self::Miniz => "miniz",
            }
        )
    }
}
