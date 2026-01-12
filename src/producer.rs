use crate::buffer::Buffer;
use crate::error::PushError;
use crate::slot::SlotState;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct Producer<T> {
    buffer: Arc<Buffer<T>>,
    id: u8,
}

impl<T> Producer<T>
where
    T: Copy + Send + 'static,
{
    pub(crate) fn new(buffer: Arc<Buffer<T>>, id: u8) -> Self {
        Self { buffer, id }
    }

    pub fn push(&self, event: T) -> Result<(), PushError> {
        // Claim a slot
        let slot_ref = self.claim()?;

        // Write payload, timestamp, and producer_id
        // SAFETY: We own exclusive access via Claimed state
        unsafe {
            (*slot_ref.slot.payload.get()).write(event);
            *slot_ref.slot.timestamp.get() = timestamp();
            *slot_ref.slot.producer_id.get() = self.id;
        }

        // Publish (transition Claimed → Published)
        slot_ref
            .slot
            .state
            .store(SlotState::Published as u8, Ordering::Release);

        Ok(())
    }

    fn claim(&self) -> Result<SlotRef<'_, T>, PushError> {
        let mut attempts = 0;
        const MAX_SPIN: usize = 10000;

        loop {
            let pos = self.buffer.head.load(Ordering::Acquire);
            let slot_idx = pos & self.buffer.mask;
            let slot = &self.buffer.slots[slot_idx];

            let state = slot.state.load(Ordering::Acquire);

            if state == SlotState::Free as u8 {
                // Try to claim
                match slot.state.compare_exchange_weak(
                    SlotState::Free as u8,
                    SlotState::Claimed as u8,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully claimed - advance head
                        self.buffer.head.fetch_add(1, Ordering::Release);
                        return Ok(SlotRef { slot });
                    }
                    Err(_) => {
                        // Lost race, retry
                        std::hint::spin_loop();
                    }
                }
            } else {
                // Slot not free - backpressure
                attempts += 1;
                if attempts > MAX_SPIN {
                    std::thread::yield_now();
                    attempts = 0;
                }
                std::hint::spin_loop();
            }
        }
    }
}

struct SlotRef<'a, T> {
    slot: &'a crate::slot::Slot<T>,
}

/// Capture a timestamp using the fastest available method
#[inline(always)]
fn timestamp() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe { core::arch::x86_64::_rdtsc() }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // On ARM, use system counter
        let mut val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
        }
        val
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // Fallback for other architectures
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn single_producer_can_push() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();
        let producer = Producer::new(buffer, 0);

        let result = producer.push(42);
        assert!(result.is_ok());
    }

    #[test]
    fn push_transitions_slot_to_published() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();
        let producer = Producer::new(buffer.clone(), 0);

        producer.push(42).unwrap();

        // Check that slot 0 is now in Published state
        let slot = &buffer.slots[0];
        let state = slot.state.load(Ordering::Acquire);
        assert_eq!(state, SlotState::Published as u8);
    }

    #[test]
    fn timestamp_captured_on_publish() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();
        let producer = Producer::new(buffer.clone(), 0);

        producer.push(42).unwrap();

        // Check that timestamp is non-zero
        let slot = &buffer.slots[0];
        let ts = unsafe { *slot.timestamp.get() };
        assert!(ts > 0, "Timestamp should be captured");
    }
}
