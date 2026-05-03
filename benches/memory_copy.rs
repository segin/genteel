use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_memory_copy(c: &mut Criterion) {
    let wram = vec![0u8; 0x10000];
    let z80_ram = vec![0u8; 0x2000];
    let vram = vec![0u8; 0x10000];

    let mut target_wram = [0u8; 0x10000];
    let mut target_z80_ram = [0u8; 0x2000];
    let mut target_vram = [0u8; 0x10000];

    c.bench_function("copy_72kb", |b| {
        b.iter(|| {
            target_wram.copy_from_slice(black_box(&wram));
            target_z80_ram.copy_from_slice(black_box(&z80_ram));
        })
    });

    c.bench_function("copy_136kb", |b| {
        b.iter(|| {
            target_wram.copy_from_slice(black_box(&wram));
            target_z80_ram.copy_from_slice(black_box(&z80_ram));
            target_vram.copy_from_slice(black_box(&vram));
        })
    });
}

criterion_group!(benches, bench_memory_copy);
criterion_main!(benches);
