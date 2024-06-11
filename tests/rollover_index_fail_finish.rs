use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};
use value_log::{Config, IndexReader, IndexWriter, ValueHandle, ValueLog};

type DebugIndexInner = RwLock<BTreeMap<Arc<[u8]>, (ValueHandle, u32)>>;

#[derive(Clone, Default)]
pub struct DebugIndex(Arc<DebugIndexInner>);

impl std::ops::Deref for DebugIndex {
    type Target = DebugIndexInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DebugIndex {
    fn insert_indirection(&self, key: &[u8], value: ValueHandle, size: u32) -> std::io::Result<()> {
        self.write()
            .expect("lock is poisoned")
            .insert(key.into(), (value, size));

        Ok(())
    }
}

impl IndexReader for DebugIndex {
    fn get(&self, key: &[u8]) -> std::io::Result<Option<ValueHandle>> {
        Ok(self
            .read()
            .expect("lock is poisoned")
            .get(key)
            .map(|(handle, _)| handle)
            .cloned())
    }
}

/// Used for tests only
#[allow(clippy::module_name_repetitions)]
pub struct DebugIndexWriter(DebugIndex);

impl IndexWriter for DebugIndexWriter {
    fn insert_indirection(
        &mut self,
        key: &[u8],
        value: ValueHandle,
        size: u32,
    ) -> std::io::Result<()> {
        self.0.insert_indirection(key, value, size)
    }

    fn finish(self) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Oh no"))
    }
}

#[test]
fn rollover_index_fail_finish() -> value_log::Result<()> {
    let folder = tempfile::tempdir()?;
    let vl_path = folder.path();

    let index = DebugIndex(Arc::new(RwLock::new(BTreeMap::default())));

    let value_log = ValueLog::open(vl_path, Config::default())?;

    let items = ["a", "b", "c", "d", "e"];

    {
        let mut writer = value_log.get_writer()?;

        let segment_id = writer.segment_id();

        for key in &items {
            let value = key.repeat(1_000);
            let value = value.as_bytes();

            let offset = writer.offset(key.as_bytes());

            index.insert_indirection(
                key.as_bytes(),
                ValueHandle { offset, segment_id },
                value.len() as u32,
            )?;

            writer.write(key.as_bytes(), value)?;
        }

        value_log.register(writer)?;
    }

    assert_eq!(value_log.manifest.list_segment_ids(), [0]);

    value_log.scan_for_stats(index.read().unwrap().values().cloned().map(Ok))?;

    assert_eq!(
        value_log.manifest.get_segment(0).unwrap().stale_ratio(),
        0.0
    );

    let result = value_log.rollover(&[0], &index, DebugIndexWriter(index.clone()));
    assert!(result.is_err());

    assert_eq!(
        {
            let mut ids = value_log.manifest.list_segment_ids();
            ids.sort();
            ids
        },
        [0, 1]
    );

    value_log.scan_for_stats(index.read().unwrap().values().cloned().map(Ok))?;

    assert_eq!(
        value_log.manifest.get_segment(0).unwrap().stale_ratio(),
        1.0
    );

    value_log.drop_stale_segments()?;

    assert_eq!(value_log.manifest.list_segment_ids(), [1]);

    Ok(())
}
