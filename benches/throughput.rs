use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lftes::Buffer;
use std::thread;
use std::time::Duration;

// Note: These benchmarks are limited by lack of slot recycling
// They measure setup/teardown overhead more than raw throughput

fn bench_buffer_lifecycle(c: &mut Criterion) {
    c.bench_function("buffer_lifecycle", |b| {
        b.iter(|| {
            let buffer = Buffer::<u64>::builder().capacity(1024).build().unwrap();
            let handle = buffer.start();

            let producer = buffer.producer();
            for i in 0..100 {
                producer.push(black_box(i)).unwrap();
            }

            thread::sleep(Duration::from_millis(10));

            let mut consumer = buffer.consumer();
            let mut count = 0;
            while consumer.try_next().is_some() && count < 100 {
                count += 1;
            }

            handle.stop();
            let _ = handle.join();
        });
    });
}

fn bench_multi_producer(c: &mut Criterion) {
    c.bench_function("multi_producer_4x50", |b| {
        b.iter(|| {
            let buffer = Buffer::<u64>::builder().capacity(4096).build().unwrap();
            let handle = buffer.start();

            let mut threads = vec![];
            for _ in 0..4 {
                let buffer_clone = buffer.clone();
                let thread = thread::spawn(move || {
                    let producer = buffer_clone.producer();
                    for i in 0..50 {
                        producer.push(black_box(i)).unwrap();
                    }
                });
                threads.push(thread);
            }

            for thread in threads {
                thread.join().unwrap();
            }

            handle.stop();
            let _ = handle.join();
        });
    });
}

fn bench_vs_crossbeam(c: &mut Criterion) {
    use crossbeam_channel::bounded;

    let mut group = c.benchmark_group("comparison");

    group.bench_function("lftes", |b| {
        b.iter(|| {
            let buffer = Buffer::<u64>::builder().capacity(1024).build().unwrap();
            let handle = buffer.start();
            let producer = buffer.producer();

            for i in 0..100 {
                producer.push(black_box(i)).unwrap();
            }

            handle.stop();
            let _ = handle.join();
        });
    });

    group.bench_function("crossbeam", |b| {
        b.iter(|| {
            let (tx, rx) = bounded(1024);

            let tx_thread = thread::spawn(move || {
                for i in 0..100 {
                    tx.send(black_box(i)).unwrap();
                }
            });

            let rx_thread = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = rx.recv().unwrap();
                }
            });

            tx_thread.join().unwrap();
            rx_thread.join().unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_lifecycle,
    bench_multi_producer,
    bench_vs_crossbeam,
);
criterion_main!(benches);
