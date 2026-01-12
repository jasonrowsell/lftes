use lftes::Buffer;
use std::collections::HashSet;
use std::thread;
use std::time::Duration;

#[test]
fn multiple_producers_no_lost_events() {
    const NUM_PRODUCERS: usize = 4;
    const EVENTS_PER_PRODUCER: usize = 50;
    const TOTAL_EVENTS: usize = NUM_PRODUCERS * EVENTS_PER_PRODUCER;

    let buffer: std::sync::Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(512).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    // Spawn multiple producers
    let mut producer_threads: Vec<thread::JoinHandle<()>> = vec![];
    for producer_id in 0..NUM_PRODUCERS {
        let buffer_clone: std::sync::Arc<Buffer<u64>> = buffer.clone();
        let thread: thread::JoinHandle<()> = thread::spawn(move || {
            let producer: lftes::Producer<u64> = buffer_clone.producer();
            for i in 0..EVENTS_PER_PRODUCER {
                let value = (producer_id * 1000 + i) as u64;
                producer.push(value).unwrap();
            }
        });
        producer_threads.push(thread);
    }

    // Wait for all producers to finish
    for thread in producer_threads {
        thread.join().unwrap();
    }

    // Give sequencer time to process
    thread::sleep(Duration::from_millis(100));

    // Consume all events
    let mut consumer: lftes::Consumer<u64> = buffer.consumer();
    let mut events: Vec<lftes::Event<u64>> = vec![];
    for _ in 0..TOTAL_EVENTS {
        if let Some(event) = consumer.try_next() {
            events.push(event);
        } else {
            // Wait a bit and retry
            thread::sleep(Duration::from_millis(10));
            if let Some(event) = consumer.try_next() {
                events.push(event);
            }
        }
    }

    // Verify no events lost
    assert_eq!(
        events.len(),
        TOTAL_EVENTS,
        "Should receive all {} events",
        TOTAL_EVENTS
    );

    // Verify all payloads are unique
    let payloads: HashSet<u64> = events.iter().map(|e: &lftes::Event<u64>| e.payload).collect();
    assert_eq!(
        payloads.len(),
        TOTAL_EVENTS,
        "All events should have unique payloads"
    );

    // Verify sequence numbers are contiguous
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.sequence, i as u64,
            "Sequence number should be contiguous"
        );
    }

    handle.stop();
    handle.join().unwrap();
}

#[test]
fn sequential_push_and_consume() {
    const BUFFER_SIZE: usize = 64;
    const NUM_EVENTS: usize = 32; // Well under capacity

    let buffer: std::sync::Arc<Buffer<u64>> = Buffer::<u64>::builder()
        .capacity(BUFFER_SIZE)
        .build()
        .unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    // Push events
    let producer: lftes::Producer<u64> = buffer.producer();
    for i in 0..NUM_EVENTS {
        producer.push(i as u64).unwrap();
    }

    // Give sequencer time to process
    thread::sleep(Duration::from_millis(100));

    // Consume events
    let mut consumer: lftes::Consumer<u64> = buffer.consumer();
    let mut consumed = 0;
    while let Some(_event) = consumer.try_next() {
        consumed += 1;
        if consumed >= NUM_EVENTS {
            break;
        }
    }

    assert_eq!(consumed, NUM_EVENTS, "All events should be consumed");

    handle.stop();
    handle.join().unwrap();
}
