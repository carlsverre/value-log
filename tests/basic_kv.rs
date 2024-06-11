use std::sync::Arc;
use test_log::test;
use value_log::{Config, KeyRange, MockIndex, MockIndexWriter, ValueLog};

#[test]
fn basic_kv() -> value_log::Result<()> {
    let folder = tempfile::tempdir()?;
    let vl_path = folder.path();

    let index = MockIndex::default();

    let value_log = ValueLog::open(vl_path, Config::default())?;

    let items = ["a", "b", "c", "d", "e"];

    {
        let index_writer = MockIndexWriter(index.clone());
        let mut writer = value_log.get_writer(index_writer)?;

        for key in &items {
            let value = key.repeat(1_000);
            let value = value.as_bytes();

            writer.write(key.as_bytes(), value)?;
        }

        value_log.register_writer(writer)?;
    }

    {
        assert_eq!(1, value_log.segment_count());

        let segments = value_log.manifest.list_segments();

        let segment = segments.first().unwrap();

        assert_eq!(items.len() as u64, segment.len());
        assert_eq!(0, segment.gc_stats.stale_items());

        assert_eq!(
            segment.meta.key_range,
            KeyRange::new((Arc::new(*b"a"), Arc::new(*b"e")))
        );

        assert_eq!(
            segment.len(),
            segment.scan().into_iter().flatten().count() as u64
        );
    }

    for (key, (handle, _)) in index.read().unwrap().iter() {
        let item = value_log.get(handle)?.unwrap();
        assert_eq!(item, key.repeat(1_000).into());
    }

    Ok(())
}
