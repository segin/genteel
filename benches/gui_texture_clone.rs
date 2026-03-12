use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_texture_clone(c: &mut Criterion) {
    let plane_w = 64;
    let plane_h = 64;

    let mut group = c.benchmark_group("texture_update");

    group.bench_function("from_rgba_unmultiplied", |b| {
        b.iter(|| {
            let mut pixels = vec![0u8; plane_w * 8 * plane_h * 8 * 4];
            for ty in 0..plane_h {
                for tx in 0..plane_w {
                    for py in 0..8 {
                        for px in 0..8 {
                            let pixel_idx = ((ty * 8 + py) * plane_w * 8 + (tx * 8 + px)) * 4;
                            pixels[pixel_idx] = 255;
                            pixels[pixel_idx + 1] = 0;
                            pixels[pixel_idx + 2] = 0;
                            pixels[pixel_idx + 3] = 255;
                        }
                    }
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([plane_w * 8, plane_h * 8], &pixels);
            black_box(image);
        });
    });

    group.bench_function("direct_colorimage", |b| {
        b.iter(|| {
            let mut image = egui::ColorImage::new(
                [plane_w * 8, plane_h * 8],
                egui::Color32::TRANSPARENT,
            );
            for ty in 0..plane_h {
                for tx in 0..plane_w {
                    for py in 0..8 {
                        for px in 0..8 {
                            let pixel_idx = (ty * 8 + py) * plane_w * 8 + (tx * 8 + px);
                            image.pixels[pixel_idx] = egui::Color32::from_rgb(255, 0, 0);
                        }
                    }
                }
            }
            black_box(image);
        });
    });
}

criterion_group!(benches, bench_texture_clone);
criterion_main!(benches);
