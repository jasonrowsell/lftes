use crate::buffer::Buffer;
use crate::slot::SlotState;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct Consumer<T> {
    buffer: Arc<Buffer<T>>,
    cursor: u64,
}

impl<T> Consumer<T>
where
    T: Copy + Send + 'static,
{
    pub(crate) fn new(buffer: Arc<Buffer<T>>) -> Self {
        Self { buffer, cursor: 0 }
    }

    pub fn try_next(&mut self) -> Option<Event<T>> {
        // Calculate slot index from cursor
        let slot_idx = (self.cursor as usize) & self.buffer.mask;
        let slot = &self.buffer.slots[slot_idx];

        // Check if slot is sequenced
        let state = slot.state.load(Ordering::Acquire);
        if state != SlotState::Sequenced as u8 {
            return None;
        }

        // Verify sequence number matches (defensive check)
        let seq = slot.sequence.load(Ordering::Acquire);
        if seq != self.cursor {
            return None; // Slot was recycled - we're too slow
        }

        // Read payload and metadata
        // SAFETY: State is Sequenced, so payload is initialized
        let payload = unsafe { (*slot.payload.get()).assume_init_read() };
        let timestamp = unsafe { *slot.timestamp.get() };
        let producer_id = unsafe { *slot.producer_id.get() };

        let event = Event {
            sequence: seq,
            timestamp,
            producer_id,
            payload,
        };

        self.cursor += 1;
        Some(event)
    }

    pub fn iter(&mut self) -> ConsumerIter<'_, T> {
        ConsumerIter { consumer: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Event<T> {
    pub sequence: u64,
    pub timestamp: u64,
    pub producer_id: u8,
    pub payload: T,
}

pub struct ConsumerIter<'a, T> {
    consumer: &'a mut Consumer<T>,
}

impl<'a, T> Iterator for ConsumerIter<'a, T>
where
    T: Copy + Send + 'static,
{
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.try_next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn consumer_reads_sequenced_slots() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();

        // Manually sequence a slot for testing
        let slot = &buffer.slots[0];
        unsafe {
            (*slot.payload.get()).write(42);
            *slot.timestamp.get() = 1000;
            *slot.producer_id.get() = 0;
        }
        slot.sequence.store(0, Ordering::Release);
        slot.state
            .store(SlotState::Sequenced as u8, Ordering::Release);

        let mut consumer = Consumer::new(buffer);
        let event = consumer.try_next();

        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.sequence, 0);
        assert_eq!(event.payload, 42);
        assert_eq!(event.timestamp, 1000);
    }

    #[test]
    fn consumer_returns_none_for_unsequenced() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();
        let mut consumer = Consumer::new(buffer);

        // No slots are sequenced yet
        let event = consumer.try_next();
        assert!(event.is_none());
    }

    #[test]
    fn consumer_advances_cursor() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();

        // Sequence two slots
        for i in 0..2 {
            let slot = &buffer.slots[i];
            unsafe {
                (*slot.payload.get()).write(100 + i as u64);
                *slot.timestamp.get() = 1000 + i as u64;
                *slot.producer_id.get() = 0;
            }
            slot.sequence.store(i as u64, Ordering::Release);
            slot.state
                .store(SlotState::Sequenced as u8, Ordering::Release);
        }

        let mut consumer = Consumer::new(buffer);

        // Read first event
        let event1 = consumer.try_next().unwrap();
        assert_eq!(event1.sequence, 0);
        assert_eq!(event1.payload, 100);

        // Read second event
        let event2 = consumer.try_next().unwrap();
        assert_eq!(event2.sequence, 1);
        assert_eq!(event2.payload, 101);

        // No more events
        assert!(consumer.try_next().is_none());
    }
}
