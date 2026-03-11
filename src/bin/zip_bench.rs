use std::fs::File;
use std::io::Write;
use std::time::Instant;
use zip::write::SimpleFileOptions;

fn main() {
    let zip_path = "test_large.zip";

    // Create a zip with 10000 dummy files
    if !std::path::Path::new(zip_path).exists() {
        let file = File::create(zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        for i in 0..10000 {
            zip.start_file(format!("dummy_{}.txt", i), options.clone())
                .unwrap();
            zip.write_all(b"Hello").unwrap();
        }
        zip.start_file("game.md", options).unwrap();
        zip.write_all(b"FAKE ROM DATA").unwrap();
        zip.finish().unwrap();
    }

    let start = Instant::now();
    let file = std::fs::File::open(zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];

    let mut found = false;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        let name = entry.name().to_lowercase();
        if rom_extensions.iter().any(|ext| name.ends_with(ext)) {
            let size = entry.size();
            if size > 32 * 1024 * 1024 {
                println!("Error: ROM size {} exceeds limit of 32MB", size);
                break;
            }
            // read
            found = true;
            break;
        }
    }

    println!("Found: {}", found);
    println!("Elapsed: {:?}", start.elapsed());
}
