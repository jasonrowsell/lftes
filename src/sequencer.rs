use crate::buffer::Buffer;
use crate::slot::SlotState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct SequencerHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SequencerHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn join(mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| "Thread join failed")?;
        }
        Ok(())
    }
}

impl Drop for SequencerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start_sequencer<T>(buffer: Arc<Buffer<T>>) -> SequencerHandle
where
    T: Copy + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    let thread = thread::spawn(move || {
        sequencer_loop(&buffer, &stop_clone);
    });

    SequencerHandle {
        stop,
        thread: Some(thread),
    }
}

fn sequencer_loop<T>(buffer: &Buffer<T>, stop: &AtomicBool) {
    let mut next_seq: u64 = 0;
    let mut scan_pos: usize = 0;

    while !stop.load(Ordering::Relaxed) {
        let slot_idx = scan_pos & buffer.mask;
        let slot = &buffer.slots[slot_idx];

        let state = slot.state.load(Ordering::Acquire);

        match state {
            s if s == SlotState::Published as u8 => {
                // Assign sequence number
                slot.sequence.store(next_seq, Ordering::Release);
                next_seq += 1;

                // Transition to Sequenced
                slot.state
                    .store(SlotState::Sequenced as u8, Ordering::Release);

                scan_pos += 1;
            }
            s if s == SlotState::Claimed as u8 => {
                // Producer still writing - spin on this slot
                std::hint::spin_loop();
            }
            _ => {
                // Free or Sequenced - nothing to do
                std::hint::spin_loop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn sequencer_assigns_monotonic_sequence() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();

        // Manually publish some slots
        for i in 0..3 {
            let slot = &buffer.slots[i];
            unsafe {
                (*slot.payload.get()).write(100 + i as u64);
                *slot.timestamp.get() = 1000 + i as u64;
                *slot.producer_id.get() = 0;
            }
            slot.state
                .store(SlotState::Published as u8, Ordering::Release);
        }

        // Start sequencer
        let handle = start_sequencer(buffer.clone());

        // Wait for sequencer to process
        thread::sleep(Duration::from_millis(50));

        // Check that sequences are assigned monotonically
        for i in 0..3 {
            let slot = &buffer.slots[i];
            let seq = slot.sequence.load(Ordering::Acquire);
            let state = slot.state.load(Ordering::Acquire);

            assert_eq!(seq, i as u64, "Sequence {} should be {}", seq, i);
            assert_eq!(
                state,
                SlotState::Sequenced as u8,
                "Slot {} should be sequenced",
                i
            );
        }

        // Cleanup
        handle.stop();
        handle.join().unwrap();
    }

    #[test]
    fn sequencer_processes_in_slot_order() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();

        // Publish slots in reverse order (3, 2, 1, 0)
        // But sequencer should process in slot order (0, 1, 2, 3)
        for i in (0..4).rev() {
            let slot = &buffer.slots[i];
            unsafe {
                (*slot.payload.get()).write(100 + i as u64);
                *slot.timestamp.get() = 1000 + (3 - i) as u64; // Reverse timestamp too
                *slot.producer_id.get() = 0;
            }
            slot.state
                .store(SlotState::Published as u8, Ordering::Release);
        }

        // Start sequencer
        let handle = start_sequencer(buffer.clone());

        // Wait for sequencer to process
        thread::sleep(Duration::from_millis(50));

        // Verify sequence is based on slot position, not publish time
        for i in 0..4 {
            let slot = &buffer.slots[i];
            let seq = slot.sequence.load(Ordering::Acquire);
            assert_eq!(
                seq, i as u64,
                "Slot {} should have sequence {}, got {}",
                i, i, seq
            );
        }

        handle.stop();
        handle.join().unwrap();
    }

    #[test]
    fn sequencer_stops_on_signal() {
        let buffer = Buffer::<u64>::builder().capacity(16).build().unwrap();

        let handle = start_sequencer(buffer);

        // Signal stop
        handle.stop();

        // Wait for thread to finish
        let result = handle.join();
        assert!(result.is_ok(), "Sequencer should stop cleanly");
    }
}
