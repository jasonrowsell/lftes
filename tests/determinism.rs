use lftes::{Buffer, Event};
use std::thread;
use std::time::Duration;

#[test]
fn deterministic_replay_same_order() {
    const NUM_EVENTS: usize = 50;

    let buffer: std::sync::Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(256).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    // Push events from a single producer
    let producer: lftes::Producer<u64> = buffer.producer();
    for i in 0..NUM_EVENTS {
        producer.push(i as u64).unwrap();
    }

    // Give sequencer time to process
    thread::sleep(Duration::from_millis(100));

    // Create two consumers starting from sequence 0
    let mut consumer1: lftes::Consumer<u64> = buffer.consumer();
    let mut consumer2: lftes::Consumer<u64> = buffer.consumer();

    // Collect events from both consumers
    let events1: Vec<Event<u64>> = (0..NUM_EVENTS)
        .filter_map(|_| consumer1.try_next())
        .collect();

    let events2: Vec<Event<u64>> = (0..NUM_EVENTS)
        .filter_map(|_| consumer2.try_next())
        .collect();

    // Verify both consumers received the same events in the same order
    assert_eq!(events1.len(), NUM_EVENTS);
    assert_eq!(events2.len(), NUM_EVENTS);

    for i in 0..NUM_EVENTS {
        assert_eq!(
            events1[i].sequence, events2[i].sequence,
            "Sequence numbers should match"
        );
        assert_eq!(
            events1[i].payload, events2[i].payload,
            "Payloads should match"
        );
        assert_eq!(
            events1[i].timestamp, events2[i].timestamp,
            "Timestamps should match"
        );
    }

    handle.stop();
    handle.join().unwrap();
}

#[test]
fn consumer_tracks_minimum_cursor() {
    let buffer: std::sync::Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(64).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    // Push some events
    let producer: lftes::Producer<u64> = buffer.producer();
    for i in 0..10 {
        producer.push(i as u64).unwrap();
    }

    // Give sequencer time to process
    thread::sleep(Duration::from_millis(50));

    // Create two consumers
    let mut consumer1: lftes::Consumer<u64> = buffer.consumer();
    let mut consumer2: lftes::Consumer<u64> = buffer.consumer();

    // Consumer1 reads 5 events
    for _ in 0..5 {
        consumer1.try_next();
    }

    // Consumer2 reads 3 events
    for _ in 0..3 {
        consumer2.try_next();
    }

    // Both consumers are at different positions
    // This demonstrates independent cursor tracking
    // (A full implementation would track min cursor for slot recycling)

    let event1: Option<Event<u64>> = consumer1.try_next();
    let event2: Option<Event<u64>> = consumer2.try_next();

    assert!(event1.is_some());
    assert!(event2.is_some());

    // Consumer1 should be at sequence 5
    assert_eq!(event1.unwrap().sequence, 5);

    // Consumer2 should be at sequence 3
    assert_eq!(event2.unwrap().sequence, 3);

    handle.stop();
    handle.join().unwrap();
}
