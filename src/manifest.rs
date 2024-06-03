use crate::{id::SegmentId, segment::stats::Stats, Segment, SegmentWriter as MultiWriter};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
    collections::HashMap,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, Arc, RwLock},
};

pub const VLOG_MARKER: &str = ".vlog";
pub const SEGMENTS_FOLDER: &str = "segments";
const MANIFEST_FILE: &str = "vlog_manifest";

/// Atomically rewrites a file
fn rewrite_atomic<P: AsRef<Path>>(path: P, content: &[u8]) -> std::io::Result<()> {
    let path = path.as_ref();
    let folder = path.parent().expect("should have a parent");

    let mut temp_file = tempfile::NamedTempFile::new_in(folder)?;
    temp_file.write_all(content)?;
    temp_file.persist(path)?;

    #[cfg(not(target_os = "windows"))]
    {
        // TODO: Not sure if the fsync is really required, but just for the sake of it...
        // TODO: also not sure why it fails on Windows...
        let file = std::fs::File::open(path)?;
        file.sync_all()?;
    }

    Ok(())
}

#[allow(clippy::module_name_repetitions)]
pub struct SegmentManifestInner {
    path: PathBuf,
    pub segments: RwLock<HashMap<SegmentId, Arc<Segment>>>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct SegmentManifest(Arc<SegmentManifestInner>);

impl std::ops::Deref for SegmentManifest {
    type Target = SegmentManifestInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SegmentManifest {
    fn remove_unfinished_segments<P: AsRef<Path>>(
        folder: P,
        registered_ids: &[u64],
    ) -> crate::Result<()> {
        for dirent in std::fs::read_dir(folder)? {
            let dirent = dirent?;

            if dirent.file_type()?.is_dir() {
                let segment_id = dirent
                    .file_name()
                    .to_str()
                    .expect("should be valid utf-8")
                    .parse::<u64>()
                    .expect("should be valid segment ID");

                if !registered_ids.contains(&segment_id) {
                    log::trace!("Deleting unfinished v-log segment {segment_id}");
                    std::fs::remove_dir_all(dirent.path())?;
                }
            }
        }

        Ok(())
    }

    /// Parses segment IDs from manifest file
    fn load_ids_from_disk<P: AsRef<Path>>(path: P) -> crate::Result<Vec<SegmentId>> {
        let path = path.as_ref();
        log::debug!("Loading manifest from {}", path.display());

        let bytes = std::fs::read(path)?;

        let mut ids = vec![];

        let mut cursor = Cursor::new(bytes);

        let cnt = cursor.read_u64::<BigEndian>()?;

        for _ in 0..cnt {
            ids.push(cursor.read_u64::<BigEndian>()?);
        }

        Ok(ids)
    }

    /// Recovers a value log from disk
    pub(crate) fn recover<P: AsRef<Path>>(folder: P) -> crate::Result<Self> {
        let folder = folder.as_ref();
        let path = folder.join(MANIFEST_FILE);

        let ids = Self::load_ids_from_disk(&path)?;

        let segments_folder = folder.join(SEGMENTS_FOLDER);
        Self::remove_unfinished_segments(&segments_folder, &ids)?;

        let segments = {
            let mut map = HashMap::with_capacity(100);

            for id in ids {
                map.insert(
                    id,
                    Arc::new(Segment {
                        id,
                        path: segments_folder.join(id.to_string()),
                        stats: Stats::default(),
                    }),
                );
            }

            map
        };

        Ok(Self(Arc::new(SegmentManifestInner {
            path,
            segments: RwLock::new(segments),
        })))
    }

    pub(crate) fn create_new<P: AsRef<Path>>(folder: P) -> crate::Result<Self> {
        let path = folder.as_ref().join(MANIFEST_FILE);

        let m = Self(Arc::new(SegmentManifestInner {
            path,
            segments: RwLock::new(HashMap::default()),
        }));
        Self::write_to_disk(&m.path, &[])?;

        Ok(m)
    }

    pub fn drop_segments(&self, ids: &[u64]) -> crate::Result<()> {
        // TODO: atomic swap

        let mut lock = self.segments.write().expect("lock is poisoned");
        lock.retain(|x, _| !ids.contains(x));
        Self::write_to_disk(&self.path, &lock.keys().copied().collect::<Vec<_>>())
    }

    pub fn register(&self, writer: MultiWriter) -> crate::Result<()> {
        // TODO: atomic swap

        let mut lock = self.segments.write().expect("lock is poisoned");
        let writers = writer.finish()?;

        for writer in writers {
            let segment_id = writer.segment_id;
            let segment_folder = writer.folder.clone();

            lock.insert(
                segment_id,
                Arc::new(Segment {
                    id: segment_id,
                    path: segment_folder,
                    stats: Stats {
                        item_count: writer.item_count,
                        total_bytes: writer.written_blob_bytes,
                        total_uncompressed_bytes: writer.uncompressed_bytes,
                        stale_items: AtomicU64::default(),
                        stale_bytes: AtomicU64::default(),
                    },
                }),
            );

            log::debug!(
                "Created segment #{segment_id:?} ({} items, {} userdata bytes)",
                writer.item_count,
                writer.uncompressed_bytes,
            );
        }

        Self::write_to_disk(&self.path, &lock.keys().copied().collect::<Vec<_>>())
    }

    fn write_to_disk<P: AsRef<Path>>(path: P, segment_ids: &[SegmentId]) -> crate::Result<()> {
        let path = path.as_ref();
        log::trace!("Writing segment manifest to {}", path.display());

        let mut bytes = Vec::new();

        let cnt = segment_ids.len() as u64;
        bytes.write_u64::<BigEndian>(cnt)?;

        for id in segment_ids {
            bytes.write_u64::<BigEndian>(*id)?;
        }

        rewrite_atomic(path, &bytes)?;

        Ok(())
    }

    /// Gets a segment
    #[must_use]
    pub fn get_segment(&self, id: SegmentId) -> Option<Arc<Segment>> {
        self.segments
            .read()
            .expect("lock is poisoned")
            .get(&id)
            .cloned()
    }

    /// Lists all segment IDs
    #[must_use]
    pub fn list_segment_ids(&self) -> Vec<SegmentId> {
        self.segments
            .read()
            .expect("lock is poisoned")
            .keys()
            .copied()
            .collect()
    }

    /// Counts segments
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.read().expect("lock is poisoned").len()
    }

    /// Lists all segments
    #[must_use]
    pub fn list_segments(&self) -> Vec<Arc<Segment>> {
        self.segments
            .read()
            .expect("lock is poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Returns the amount of bytes on disk that are occupied by blobs.
    #[must_use]
    pub fn disk_space_used(&self) -> u64 {
        self.segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.total_bytes)
            .sum::<u64>()
    }

    /*  /// Returns the amount of stale items
    #[must_use]
    pub fn stale_items_count(&self) -> u64 {
        self.segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.stale_items())
            .sum::<u64>()
    } */

    /// Returns the percent of dead bytes (uncompressed) in the value log
    #[must_use]
    pub fn stale_ratio(&self) -> f32 {
        let used_bytes = self
            .segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.total_uncompressed_bytes)
            .sum::<u64>();
        if used_bytes == 0 {
            return 0.0;
        }

        let stale_bytes = self
            .segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.stale_bytes())
            .sum::<u64>();

        if stale_bytes == 0 {
            return 0.0;
        }

        stale_bytes as f32 / used_bytes as f32
    }

    /// Returns the approximate space amplification
    ///
    /// Returns 0.0 if there are no items.
    #[must_use]
    pub fn space_amp(&self) -> f32 {
        let used_bytes = self
            .segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.total_uncompressed_bytes)
            .sum::<u64>();

        if used_bytes == 0 {
            return 0.0;
        }

        let stale_bytes = self
            .segments
            .read()
            .expect("lock is poisoned")
            .values()
            .map(|x| x.stats.stale_bytes())
            .sum::<u64>();

        let alive_bytes = used_bytes - stale_bytes;
        if alive_bytes == 0 {
            return 0.0;
        }

        used_bytes as f32 / alive_bytes as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use test_log::test;

    #[test]
    fn test_atomic_rewrite() -> crate::Result<()> {
        let dir = tempfile::tempdir()?;

        let path = dir.path().join("test.txt");
        {
            let mut file = File::create(&path)?;
            write!(file, "asdasdasdasdasd")?;
        }

        rewrite_atomic(&path, b"newcontent")?;

        let content = std::fs::read_to_string(&path)?;
        assert_eq!("newcontent", content);

        Ok(())
    }
}
