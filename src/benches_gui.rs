use std::time::Instant;
use egui::ColorImage;
use egui::Color32;

fn main() {
    let plane_w = 64;
    let plane_h = 32;
    let iterations = 1000;

    let vram = vec![0u8; 65536];
    let cram = vec![0u16; 64];

    // Simulate render_plane baseline
    let start_baseline = Instant::now();
    for _ in 0..iterations {
        let mut image = ColorImage::new([plane_w * 8, plane_h * 8], Color32::TRANSPARENT);
        for ty in 0..plane_h {
            for tx in 0..plane_w {
                for py in 0..8 {
                    for px in 0..8 {
                        let pixel_idx = (ty * 8 + py) * plane_w * 8 + (tx * 8 + px);
                        image.pixels[pixel_idx] = Color32::from_rgb(255, 0, 0);
                    }
                }
            }
        }
    }
    println!("Baseline: {:?}", start_baseline.elapsed());

    // Simulate reused buffer
    let mut reused_image = ColorImage::new([plane_w * 8, plane_h * 8], Color32::TRANSPARENT);
    let start_optimized = Instant::now();
    for _ in 0..iterations {
        for ty in 0..plane_h {
            for tx in 0..plane_w {
                for py in 0..8 {
                    for px in 0..8 {
                        let pixel_idx = (ty * 8 + py) * plane_w * 8 + (tx * 8 + px);
                        reused_image.pixels[pixel_idx] = Color32::from_rgb(255, 0, 0);
                    }
                }
            }
        }
    }
    println!("Optimized (reused image): {:?}", start_optimized.elapsed());

    // Wait, the prompt specifically says:
    // `vec![0u8; plane_w * 8 * plane_h * 8 * 4]`
    // but the code actually says:
    // `let mut image = egui::ColorImage::new([plane_w * 8, plane_h * 8], egui::Color32::TRANSPARENT);`
    // Let me check my previous output...
}
