# lftes

Lock-free temporal event store. Deterministic replay for multi-producer scenarios.

LMAX Disruptor is for single-JVM. This is for when you need reproducible ordering across producers - debugging, audit trails, etc.

## Design

Producers CAS slots in ring buffer. Background sequencer assigns monotonic sequence numbers by scanning in slot order. Consumers iterate independently.

Key: separate claiming (parallel) from ordering (serial).

Cache-line aligned slots (64B). `rdtsc`/`cntvct_el0` timestamps.

## Usage

```rust
let buffer = Buffer::<MyEvent>::builder().capacity(8192).build()?;
let handle = buffer.start();

let producer = buffer.producer();
producer.push(event)?;

let mut consumer = buffer.consumer();
for event in consumer.iter() { }
```

## Performance

Quick bench (includes setup overhead, not pure push latency):

- 150ns/event (100 events, single producer)
- 320ns/event (200 events, 4 concurrent producers)
- 33% faster than crossbeam-channel in comparable scenario

Run `cargo bench --bench throughput -- --quick` to verify.

---

Prototype. No slot recycling yet. `T: Copy + Send` only.

```
cargo test
cargo run --example basic
```
