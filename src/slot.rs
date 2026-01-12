use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SlotState {
    Free = 0,
    Claimed = 1,
    Published = 2,
    Sequenced = 3,
}

#[repr(C, align(64))]
pub struct Slot<T> {
    pub(crate) state: AtomicU8,
    pub(crate) producer_id: std::cell::UnsafeCell<u8>,
    _pad1: [u8; 6],
    pub(crate) sequence: AtomicU64,
    pub(crate) timestamp: std::cell::UnsafeCell<u64>,
    pub(crate) payload: std::cell::UnsafeCell<MaybeUninit<T>>,
}

impl<T> Slot<T> {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(SlotState::Free as u8),
            producer_id: std::cell::UnsafeCell::new(0),
            _pad1: [0; 6],
            sequence: AtomicU64::new(0),
            timestamp: std::cell::UnsafeCell::new(0),
            payload: std::cell::UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Slot")
            .field("state", &self.state.load(Ordering::Relaxed))
            .field(
                "producer_id",
                unsafe { &*self.producer_id.get() },
            )
            .field("sequence", &self.sequence.load(Ordering::Relaxed))
            .field("timestamp", unsafe { &*self.timestamp.get() })
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_state_values_are_correct() {
        assert_eq!(SlotState::Free as u8, 0);
        assert_eq!(SlotState::Claimed as u8, 1);
        assert_eq!(SlotState::Published as u8, 2);
        assert_eq!(SlotState::Sequenced as u8, 3);
    }

    #[test]
    fn slot_is_cache_line_aligned() {
        assert_eq!(std::mem::align_of::<Slot<u64>>(), 64);
    }

    #[test]
    fn slot_size_with_payload() {
        // Slot should be cache-line aligned (64 bytes minimum)
        // With small payload like u64, it should still be 64 bytes
        let size = std::mem::size_of::<Slot<u64>>();
        assert!(size >= 64, "Slot size {} should be at least 64 bytes", size);
        assert_eq!(size % 64, 0, "Slot size {} should be multiple of 64 bytes", size);
    }
}
