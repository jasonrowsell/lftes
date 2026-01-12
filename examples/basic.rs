use lftes::Buffer;
use std::thread;
use std::time::Duration;

fn main() {
    println!("lftes - Lock-Free Temporal Event Store Demo\n");

    // Create a buffer with 256 slots
    let buffer = Buffer::<u64>::builder().capacity(256).build().unwrap();

    println!("Created buffer with 256 slots");

    // Start the sequencer
    let handle = buffer.start();
    println!("Started sequencer thread\n");

    // Create a producer
    let producer = buffer.producer();

    // Push some events
    println!("Pushing 10 events...");
    for i in 0..10 {
        producer.push(i * 100).unwrap();
    }

    // Give sequencer time to process
    thread::sleep(Duration::from_millis(50));

    // Create a consumer and read events
    println!("\nConsuming events:");
    let mut consumer = buffer.consumer();

    while let Some(event) = consumer.try_next() {
        println!(
            "  seq={} payload={} ts={} producer={}",
            event.sequence, event.payload, event.timestamp, event.producer_id
        );
    }

    // Demonstrate deterministic replay
    println!("\nDeterministic replay (second consumer):");
    let mut consumer2 = buffer.consumer();

    while let Some(event) = consumer2.try_next() {
        println!("  seq={} payload={}", event.sequence, event.payload);
    }

    // Cleanup
    handle.stop();
    handle.join().unwrap();

    println!("\nSequencer stopped. Demo complete!");
}
