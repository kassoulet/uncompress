// Script to generate test fixture files
// Run with: cargo run --example generate_fixtures

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let fixtures_dir = Path::new("tests/fixtures");
    fs::create_dir_all(fixtures_dir).expect("Failed to create fixtures directory");

    // Generate test ZIP file
    generate_test_zip(fixtures_dir.join("test.zip"));
    
    // Generate test ZIP file with wrong extension
    generate_test_zip(fixtures_dir.join("test.dat"));
    
    // Generate test PNG file
    generate_test_png(fixtures_dir.join("test.png"));
    
    // Generate test PNG file with wrong extension
    generate_test_png(fixtures_dir.join("test.bin"));
    
    // Generate test GZ file
    generate_test_gz(fixtures_dir.join("test.txt.gz"));
    
    // Generate test GZ file with wrong extension
    generate_test_gz(fixtures_dir.join("test.data"));
    
    // Generate test TIFF file (RGB)
    generate_test_tiff_rgb(fixtures_dir.join("test.tiff"));
    
    // Generate test TIFF file with wrong extension
    generate_test_tiff_rgb(fixtures_dir.join("test.tif_data"));
    
    // Generate test TIFF file (grayscale)
    generate_test_tiff_gray(fixtures_dir.join("test_gray.tiff"));

    println!("Test fixtures generated successfully in {:?}", fixtures_dir);
}

fn generate_test_zip(path: PathBuf) {
    use zip::write::FileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    let file = File::create(&path).expect("Failed to create ZIP file");
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(CompressionMethod::Deflated);

    // Add multiple files to the ZIP
    zip.start_file("content.txt", options).expect("Failed to add file to ZIP");
    zip.write_all(b"Hello from ZIP! This is test content.").expect("Failed to write to ZIP");

    zip.start_file("data.json", options).expect("Failed to add file to ZIP");
    zip.write_all(b"{\"key\": \"value\", \"number\": 42}").expect("Failed to write to ZIP");

    zip.start_file("subdir/nested.txt", options).expect("Failed to add nested file");
    zip.write_all(b"Nested file content").expect("Failed to write nested content");

    zip.finish().expect("Failed to finish ZIP");
    
    println!("Created: {:?}", path);
}

fn generate_test_png(path: PathBuf) {
    use png::{BitDepth, ColorType, Encoder};

    // Create a simple 4x4 RGB image with different colors
    let width = 4;
    let height = 4;
    
    // Create image data: each pixel has RGB values (no filter bytes - encoder adds them)
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            data.push((x * 64) as u8); // R
            data.push((y * 64) as u8); // G
            data.push(128); // B
        }
    }

    let file = File::create(&path).expect("Failed to create PNG file");
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(BitDepth::Eight);
    
    let mut writer = encoder.write_header().expect("Failed to write PNG header");
    writer.write_image_data(&data).expect("Failed to write PNG data");
    writer.finish().expect("Failed to finish PNG");

    println!("Created: {:?}", path);
}

fn generate_test_gz(path: PathBuf) {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let file = File::create(&path).expect("Failed to create GZ file");
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(b"This is compressed test data for GZ fixtures.").expect("Failed to write GZ");
    encoder.finish().expect("Failed to finish GZ");

    println!("Created: {:?}", path);
}

fn generate_test_tiff_rgb(path: PathBuf) {
    use tiff::encoder::colortype::RGB8;
    use tiff::encoder::TiffEncoder;

    let file = File::create(&path).expect("Failed to create TIFF file");
    let mut encoder = TiffEncoder::new(file).expect("Failed to create TIFF encoder");
    let mut image = encoder.new_image::<RGB8>(4, 4).expect("Failed to create TIFF image");
    
    // Create RGB image data (4x4 pixels)
    let mut data = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            data.push((x * 64) as u8); // R
            data.push((y * 64) as u8); // G
            data.push(128); // B
        }
    }
    
    image.write_data(&data).expect("Failed to write TIFF data");
    println!("Created: {:?}", path);
}

fn generate_test_tiff_gray(path: PathBuf) {
    use tiff::encoder::colortype::Gray8;
    use tiff::encoder::TiffEncoder;

    let file = File::create(&path).expect("Failed to create TIFF file");
    let mut encoder = TiffEncoder::new(file).expect("Failed to create TIFF encoder");
    let mut image = encoder.new_image::<Gray8>(4, 4).expect("Failed to create TIFF image");
    
    // Create grayscale image data (4x4 pixels)
    let data: Vec<u8> = (0..16).map(|i| (i * 16) as u8).collect();
    
    image.write_data(&data).expect("Failed to write TIFF data");
    println!("Created: {:?}", path);
}
