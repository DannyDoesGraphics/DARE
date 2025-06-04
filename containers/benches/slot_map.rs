//! Comprehensive benchmarks for slot map implementations
//!
//! This benchmark suite compares the performance of different slot map variants:
//! - `SlotMap`: Basic slot map with O(1) insert/remove/get operations
//! - `UniqueSlotMap`: Slot map that prevents duplicate values using hash-based checking
//! - `InsertionSortSlotMap`: Slot map that maintains insertion order with additional sorting capabilities
//!
//! Benchmarks include:
//! - Insert operations: Bulk insertion of sequential values
//! - Get operations: Random access to stored values
//! - Remove operations: Removal of all stored values
//! - UniqueSlotMap-specific operations: Contains and get-slot-for-value lookups
//!
//! Each benchmark tests multiple data sizes (100, 1000, 10000) to observe scaling behavior.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dare_containers::prelude::{InsertionSortSlotMap, SlotMap, UniqueSlotMap};
use std::hint::black_box;

fn benchmark_slot_map_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_map_insert");
    group.measurement_time(std::time::Duration::from_secs(100));
    group.sample_size(100);
    // Test different sizes
    for size in [100, 1000, 10000, 100_000, 1_000_000].iter() {
        group.bench_with_input(BenchmarkId::new("SlotMap", size), size, |b, &size| {
            b.iter(|| {
                let mut slot_map: SlotMap<u64> = SlotMap::default();
                for i in 0..size {
                    black_box(slot_map.insert(black_box(i)));
                }
                black_box(slot_map)
            });
        });

        group.bench_with_input(BenchmarkId::new("UniqueSlotMap", size), size, |b, &size| {
            b.iter(|| {
                let mut slot_map: UniqueSlotMap<u64> = UniqueSlotMap::default();
                for i in 0..size {
                    black_box(slot_map.insert(black_box(i)).unwrap());
                }
                black_box(slot_map)
            });
        });
    }

    group.finish();
}

fn benchmark_slot_map_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_map_get");
    group.measurement_time(std::time::Duration::from_secs(100));
    group.sample_size(100);
    for size in [100, 1000, 10000, 100_000, 1_000_000].iter() {
        // Setup data for get benchmarks
        let mut slot_map: SlotMap<u64> = SlotMap::default();
        let mut slots = Vec::new();
        for i in 0..*size {
            slots.push(slot_map.insert(i));
        }
        group.bench_with_input(BenchmarkId::new("SlotMap", size), size, |b, _| {
            b.iter(|| {
                for slot in &slots {
                    black_box(slot_map.get(black_box(slot.clone())));
                }
            });
        });

        // UniqueSlotMap get benchmark
        let mut unique_slot_map: UniqueSlotMap<u64> = UniqueSlotMap::default();
        let mut unique_slots = Vec::new();
        for i in 0..*size {
            unique_slots.push(unique_slot_map.insert(i).unwrap());
        }
        group.bench_with_input(BenchmarkId::new("UniqueSlotMap", size), size, |b, _| {
            b.iter(|| {
                for slot in &unique_slots {
                    black_box(unique_slot_map.get(black_box(slot.clone())));
                }
            });
        });
    }

    group.finish();
}

fn benchmark_slot_map_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_map_remove");
    group.measurement_time(std::time::Duration::from_secs(100));
    group.sample_size(100);
    for size in [100, 1_000, 10_000, 100_000, 1_000_000].iter() {
        group.bench_with_input(BenchmarkId::new("SlotMap", size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut slot_map: SlotMap<u64> = SlotMap::default();
                    let mut slots = Vec::new();
                    for i in 0..size {
                        slots.push(slot_map.insert(i));
                    }
                    (slot_map, slots)
                },
                |(mut slot_map, slots)| {
                    for slot in slots {
                        black_box(slot_map.remove(black_box(slot)).unwrap());
                    }
                    black_box(slot_map)
                },
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("UniqueSlotMap", size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut slot_map: UniqueSlotMap<u64> = UniqueSlotMap::default();
                    let mut slots = Vec::new();
                    for i in 0..size {
                        slots.push(slot_map.insert(i).unwrap());
                    }
                    (slot_map, slots)
                },
                |(mut slot_map, slots)| {
                    for slot in slots {
                        black_box(slot_map.remove(black_box(slot)).unwrap());
                    }
                    black_box(slot_map)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_unique_slot_map_contains(c: &mut Criterion) {
    let mut group = c.benchmark_group("unique_slot_map_contains");
    group.measurement_time(std::time::Duration::from_secs(100));
    group.sample_size(100);
    for size in [100, 1_000, 10_000, 100_000, 1_000_000].iter() {
        let mut slot_map: UniqueSlotMap<u64> = UniqueSlotMap::default();
        for i in 0..*size {
            slot_map.insert(i).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("contains_value", size),
            size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        black_box(slot_map.contains_value(black_box(&i)));
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_slot_for_value", size),
            size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        black_box(slot_map.get_slot_for_value(black_box(&i)));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_slot_map_insert,
    benchmark_slot_map_get,
    benchmark_slot_map_remove,
    benchmark_unique_slot_map_contains
);
criterion_main!(benches);
