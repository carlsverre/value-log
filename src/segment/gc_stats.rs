use std::sync::atomic::AtomicU64;

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct GcStats {
    pub(crate) stale_items: AtomicU64,
    pub(crate) stale_bytes: AtomicU64,
}

impl GcStats {
    pub fn set_stale_items(&self, x: u64) {
        self.stale_items
            .store(x, std::sync::atomic::Ordering::Release);
    }

    pub fn set_stale_bytes(&self, x: u64) {
        self.stale_bytes
            .store(x, std::sync::atomic::Ordering::Release);
    }

    /// Returns the amount of dead items in the segment
    pub fn stale_items(&self) -> u64 {
        self.stale_items.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Returns the amount of dead bytes in the segment
    pub fn stale_bytes(&self) -> u64 {
        self.stale_bytes.load(std::sync::atomic::Ordering::Acquire)
    }
}
