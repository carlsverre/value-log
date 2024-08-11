// Copyright (c) 2024-present, fjall-rs
// This source code is licensed under both the Apache 2.0 and MIT License
// (found in the LICENSE-* files in the repository)

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// Disk format version
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Version {
    /// Version for 1.x.x releases
    V1,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", u16::from(*self))
    }
}

impl From<Version> for u16 {
    fn from(value: Version) -> Self {
        match value {
            Version::V1 => 1,
        }
    }
}

impl TryFrom<u16> for Version {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            _ => Err(()),
        }
    }
}

const MAGIC_BYTES: [u8; 3] = [b'V', b'L', b'G'];

impl Version {
    // NOTE: Used in tests
    #[allow(unused)]
    pub(crate) fn len() -> u8 {
        5
    }

    pub(crate) fn parse_file_header(bytes: &[u8]) -> Option<Self> {
        let first_three = bytes.get(0..3)?;

        if first_three == MAGIC_BYTES {
            let next_two = bytes.get(3..5)?;

            let mut bytes = [0; 2];
            bytes.copy_from_slice(next_two);
            let mut bytes: &[u8] = &bytes;

            let value = bytes.read_u16::<BigEndian>().ok()?;
            let version = Self::try_from(value).ok()?;

            Some(version)
        } else {
            None
        }
    }

    pub(crate) fn write_file_header<W: std::io::Write>(
        self,
        writer: &mut W,
    ) -> std::io::Result<usize> {
        writer.write_all(&MAGIC_BYTES)?;
        writer.write_u16::<BigEndian>(u16::from(self))?;
        Ok(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    #[allow(clippy::expect_used)]
    pub fn version_serialize() -> crate::Result<()> {
        let mut bytes = vec![];
        Version::V1.write_file_header(&mut bytes)?;
        assert_eq!(bytes, &[b'V', b'L', b'G', 0, 1]);
        Ok(())
    }

    #[test]
    #[allow(clippy::expect_used)]
    pub fn version_deserialize_success() {
        let version = Version::parse_file_header(&[b'V', b'L', b'G', 0, 1]);
        assert_eq!(version, Some(Version::V1));
    }

    #[test]
    #[allow(clippy::expect_used)]
    pub fn version_deserialize_fail() {
        let version = Version::parse_file_header(&[b'F', b'J', b'X', 0, 1]);
        assert!(version.is_none());
    }

    #[test]
    #[allow(clippy::expect_used)]
    pub fn version_serde_round_trip() {
        let mut buf = vec![];
        Version::V1.write_file_header(&mut buf).expect("can't fail");

        let version = Version::parse_file_header(&buf);
        assert_eq!(version, Some(Version::V1));
    }

    #[test]
    #[allow(clippy::expect_used)]
    pub fn version_len() {
        let mut buf = vec![];
        let size = Version::V1.write_file_header(&mut buf).expect("can't fail");
        assert_eq!(Version::len() as usize, size);
    }
}
