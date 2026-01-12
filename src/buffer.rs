use crate::consumer::Consumer;
use crate::error::BuildError;
use crate::producer::Producer;
use crate::sequencer::{start_sequencer, SequencerHandle};
use crate::slot::Slot;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::Arc;

#[cfg(test)]
use crate::slot::SlotState;
#[cfg(test)]
use std::sync::atomic::Ordering;

const MAX_CAPACITY: usize = 1 << 30; // 1 billion slots max

#[derive(Debug)]
pub struct Buffer<T> {
    pub(crate) slots: Box<[Slot<T>]>,
    pub(crate) capacity: usize,
    pub(crate) mask: usize,
    pub(crate) head: AtomicUsize,
    pub(crate) tail: AtomicU64,
}

impl<T> Buffer<T>
where
    T: Copy + Send + 'static,
{
    pub fn builder() -> BufferBuilder<T> {
        BufferBuilder::new()
    }

    fn new(capacity: usize) -> Result<Self, BuildError> {
        if !capacity.is_power_of_two() {
            return Err(BuildError::InvalidCapacity);
        }

        if capacity > MAX_CAPACITY {
            return Err(BuildError::TooLarge);
        }

        let slots: Vec<Slot<T>> = (0..capacity).map(|_| Slot::new()).collect();

        Ok(Self {
            slots: slots.into_boxed_slice(),
            capacity,
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicU64::new(0),
        })
    }

    /// Start the sequencer thread
    pub fn start(self: &Arc<Self>) -> SequencerHandle {
        start_sequencer(self.clone())
    }

    /// Create a new producer handle
    pub fn producer(self: &Arc<Self>) -> Producer<T> {
        // TODO: Track producer IDs
        Producer::new(self.clone(), 0)
    }

    /// Create a new consumer handle
    pub fn consumer(self: &Arc<Self>) -> Consumer<T> {
        Consumer::new(self.clone())
    }

    #[cfg(test)]
    fn slots_are_free(&self) -> bool {
        self.slots.iter().all(|slot| {
            let state = slot.state.load(Ordering::Relaxed);
            state == SlotState::Free as u8
        })
    }
}

pub struct BufferBuilder<T> {
    capacity: Option<usize>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> BufferBuilder<T>
where
    T: Copy + Send + 'static,
{
    pub fn new() -> Self {
        Self {
            capacity: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = Some(capacity);
        self
    }

    pub fn build(self) -> Result<Arc<Buffer<T>>, BuildError> {
        let capacity = self.capacity.unwrap_or(1024);
        let buffer = Buffer::new(capacity)?;
        Ok(Arc::new(buffer))
    }
}

impl<T> Default for BufferBuilder<T>
where
    T: Copy + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_must_be_power_of_two() {
        // Valid powers of two
        assert!(Buffer::<u64>::new(2).is_ok());
        assert!(Buffer::<u64>::new(1024).is_ok());
        assert!(Buffer::<u64>::new(8192).is_ok());

        // Invalid - not powers of two
        assert_eq!(Buffer::<u64>::new(3).unwrap_err(), BuildError::InvalidCapacity);
        assert_eq!(
            Buffer::<u64>::new(1000).unwrap_err(),
            BuildError::InvalidCapacity
        );
        assert_eq!(
            Buffer::<u64>::new(7).unwrap_err(),
            BuildError::InvalidCapacity
        );
    }

    #[test]
    fn buffer_creates_with_valid_capacity() {
        let buffer = Buffer::<u64>::new(1024).unwrap();
        assert_eq!(buffer.capacity, 1024);
        assert_eq!(buffer.mask, 1023);
    }

    #[test]
    fn slots_initialized_to_free() {
        let buffer = Buffer::<u64>::new(256).unwrap();
        assert!(buffer.slots_are_free());
    }

    #[test]
    fn buffer_builder_uses_default_capacity() {
        let buffer = Buffer::<u64>::builder().build().unwrap();
        assert_eq!(buffer.capacity, 1024);
    }

    #[test]
    fn buffer_builder_uses_specified_capacity() {
        let buffer = Buffer::<u64>::builder().capacity(512).build().unwrap();
        assert_eq!(buffer.capacity, 512);
    }
}
