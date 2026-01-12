use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lftes::Buffer;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn bench_push_latency(c: &mut Criterion) {
    let buffer: Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(1024).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();
    let producer: lftes::Producer<u64> = buffer.producer();

    c.bench_function("push_latency", |b: &mut criterion::Bencher<'_>| {
        b.iter(|| {
            producer.push(black_box(42)).unwrap();
        });
    });

    handle.stop();
    handle.join().unwrap();
}

fn bench_end_to_end_latency(c: &mut Criterion) {
    let buffer: Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(1024).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    c.bench_function("end_to_end_latency", |b: &mut criterion::Bencher<'_>| {
        b.iter(|| {
            let producer: lftes::Producer<u64> = buffer.producer();
            let mut consumer: lftes::Consumer<u64> = buffer.consumer();

            // Push event
            let start: std::time::Instant = std::time::Instant::now();
            producer.push(black_box(42)).unwrap();

            // Wait for sequencing
            while consumer.try_next().is_none() {
                std::hint::spin_loop();
            }
            let _duration: Duration = start.elapsed();
        });
    });

    handle.stop();
    handle.join().unwrap();
}

fn bench_producer_throughput(c: &mut Criterion) {
    let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("producer_throughput");

    for num_producers in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*num_producers as u64 * 1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_producers),
            num_producers,
            |b: &mut criterion::Bencher<'_>, &num_producers| {
                b.iter(|| {
                    let buffer: Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(2048).build().unwrap();
                    let handle: lftes::SequencerHandle = buffer.start();

                    let mut threads: Vec<thread::JoinHandle<()>> = vec![];
                    for _ in 0..num_producers {
                        let buffer_clone: Arc<Buffer<u64>> = buffer.clone();
                        let thread: thread::JoinHandle<()> = thread::spawn(move || {
                            let producer: lftes::Producer<u64> = buffer_clone.producer();
                            for i in 0..1000 {
                                producer.push(black_box(i)).unwrap();
                            }
                        });
                        threads.push(thread);
                    }

                    for thread in threads {
                        thread.join().unwrap();
                    }

                    handle.stop();
                    handle.join().unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_consume_throughput(c: &mut Criterion) {
    let buffer: Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(2048).build().unwrap();
    let handle: lftes::SequencerHandle = buffer.start();

    // Pre-fill buffer with events
    let producer: lftes::Producer<u64> = buffer.producer();
    for i in 0..1000 {
        producer.push(i).unwrap();
    }

    // Wait for sequencing
    thread::sleep(Duration::from_millis(100));

    c.bench_function("consume_throughput", |b: &mut criterion::Bencher<'_>| {
        b.iter(|| {
            let mut consumer: lftes::Consumer<u64> = buffer.consumer();
            let mut count = 0;
            while consumer.try_next().is_some() && count < 1000 {
                count += 1;
            }
            black_box(count);
        });
    });

    handle.stop();
    handle.join().unwrap();
}

fn bench_vs_crossbeam_channel(c: &mut Criterion) {
    use crossbeam_channel::{bounded, Sender};

    let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("comparison");

    // lftes
    group.bench_function("lftes", |b| {
        b.iter(|| {
            let buffer: Arc<Buffer<u64>> = Buffer::<u64>::builder().capacity(1024).build().unwrap();
            let handle: lftes::SequencerHandle = buffer.start();
            let producer: lftes::Producer<u64> = buffer.producer();

            for i in 0..100 {
                producer.push(black_box(i)).unwrap();
            }

            handle.stop();
            handle.join().unwrap();
        });
    });

    // crossbeam
    group.bench_function("crossbeam_channel", |b: &mut criterion::Bencher<'_>| {
        b.iter(|| {
            let (tx, rx): (Sender<u64>, _) = bounded(1024);

            let tx_thread = thread::spawn(move || {
                for i in 0..100 {
                    tx.send(black_box(i)).unwrap();
                }
            });

            let rx_thread: thread::JoinHandle<()> = thread::spawn(move || {
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
    bench_push_latency,
    bench_end_to_end_latency,
    bench_producer_throughput,
    bench_consume_throughput,
    bench_vs_crossbeam_channel,
);
criterion_main!(benches);
