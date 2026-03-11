use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

// We will test creating and populating the points Vec like the GUI does
fn bench_vec_allocation(c: &mut Criterion) {
    let channel_waveform = [0i16; 128]; // dummy data
    let mut group = c.benchmark_group("gui_points_allocation");

    group.bench_function("current_implementation", |b| {
        b.iter(|| {
            let mut points = Vec::new();
            for i in 0..128 {
                let val = channel_waveform[i];
                let x = i as f32 * 2.0;
                let y = 100.0 - (val as f32 / 16384.0 * 20.0);
                // We use a dummy tuple to represent egui::pos2
                points.push((x, y));
            }
            black_box(points);
        });
    });

    group.bench_function("pre_allocated", |b| {
        b.iter(|| {
            let mut points = Vec::with_capacity(128);
            for i in 0..128 {
                let val = channel_waveform[i];
                let x = i as f32 * 2.0;
                let y = 100.0 - (val as f32 / 16384.0 * 20.0);
                points.push((x, y));
            }
            black_box(points);
        });
    });

    group.bench_function("map_collect", |b| {
        b.iter(|| {
            let points: Vec<_> = (0..128)
                .map(|i| {
                    let val = channel_waveform[i];
                    let x = i as f32 * 2.0;
                    let y = 100.0 - (val as f32 / 16384.0 * 20.0);
                    (x, y)
                })
                .collect();
            black_box(points);
        });
    });

    // Reuse a buffer
    let mut reusable_buffer = Vec::with_capacity(128);
    group.bench_function("reuse_buffer", |b| {
        b.iter(|| {
            reusable_buffer.clear();
            for i in 0..128 {
                let val = channel_waveform[i];
                let x = i as f32 * 2.0;
                let y = 100.0 - (val as f32 / 16384.0 * 20.0);
                reusable_buffer.push((x, y));
            }
            black_box(&reusable_buffer);
        });
    });
}

criterion_group!(benches, bench_vec_allocation);
criterion_main!(benches);
