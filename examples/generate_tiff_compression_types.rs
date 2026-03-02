// Script to generate test TIFF files with all compression types using gdal
// Run with: cargo run --example generate_tiff_compression_types
// Requires: gdal_translate command-line tool

use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let fixtures_dir = Path::new("tests/fixtures/tiff_compression");
    fs::create_dir_all(fixtures_dir).expect("Failed to create fixtures directory");

    // First create an uncompressed base TIFF
    let base_tiff = fixtures_dir.join("base_uncompressed.tif");
    create_base_tiff(&base_tiff);

    // Generate TIFF files with different compression types
    let compression_types = vec![
        ("uncompressed", "NONE", ""),
        ("lzw", "LZW", ""),
        ("deflate", "DEFLATE", ""),
        ("packbits", "PACKBITS", ""),
        ("jpeg", "JPEG", ""),
        ("zstd", "ZSTD", ""),
        ("webp", "WEBP", ""),
        ("lzma", "LZMA", ""),
        ("lerc", "LERC", ""),
        ("lerc_deflate", "LERC_DEFLATE", ""),
        ("lerc_zstd", "LERC_ZSTD", ""),
    ];

    for (name, compression, extra_opts) in compression_types {
        let output_path = fixtures_dir.join(format!("test_{}.tif", name));

        let mut cmd = Command::new("gdal_translate");
        cmd.arg("-co")
           .arg(format!("COMPRESS={}", compression))
           .arg("-co")
           .arg("PREDICTOR=2")  // Horizontal predictor for better compression
           .arg(&base_tiff)
           .arg(&output_path);

        if !extra_opts.is_empty() {
            cmd.arg(extra_opts);
        }

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    println!("Created: {:?} ({} compression)", output_path, compression);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("Failed to create {}: {}", name, stderr);
                }
            }
            Err(e) => {
                println!("Error running gdal_translate for {}: {}", name, e);
            }
        }
    }

    // Also create some special bit depth TIFFs
    create_16bit_tiff(fixtures_dir);
    create_multiband_tiff(fixtures_dir);

    println!("\nTest TIFF fixtures generated in {:?}", fixtures_dir);
    println!("Run 'gdalinfo tests/fixtures/tiff_compression/test_*.tif' to inspect");
}

fn create_base_tiff(path: &Path) {
    // Create a simple 64x64 RGB byte TIFF using gdal
    let output = Command::new("gdal_translate")
        .arg("-of")
        .arg("GTiff")
        .arg("-ot")
        .arg("Byte")
        .arg("-co")
        .arg("COMPRESS=NONE")
        .arg("-co")
        .arg("PHOTOMETRIC=RGB")
        .arg("-a_srs")
        .arg("EPSG:4326")
        .arg("-a_ullr")
        .arg("0.0")
        .arg("1.0")
        .arg("1.0")
        .arg("0.0")
        .arg("-of")
        .arg("GTiff")
        .arg("/vsimem/temp.vrt")
        .arg(path)
        .output();

    // If VRT approach doesn't work, create with Python or use a simpler method
    if output.is_err() || !output.as_ref().unwrap().status.success() {
        // Alternative: create using gdaldem or simpler approach
        create_base_tiff_simple(path);
    }
}

fn create_base_tiff_simple(path: &Path) {
    // Create using gdal_translate with -of MEM and then copy
    // Or use a pre-made approach with gdal's built-in test data generation

    // Use gdal's create method via Python bindings alternative
    // For simplicity, use gdal_translate with a constant value raster
    let output = Command::new("gdal_translate")
        .arg("-of")
        .arg("GTiff")
        .arg("-outsize")
        .arg("64")
        .arg("64")
        .arg("-co")
        .arg("COMPRESS=NONE")
        .arg("data/byte.tif")  // Use GDAL's test data if available
        .arg(path)
        .output();

    if output.is_ok() && output.as_ref().unwrap().status.success() {
        return;
    }

    // Fallback: create with gdal_rasterize or gdal_calc
    create_with_gdal_rasterize(path);
}

fn create_with_gdal_rasterize(path: &Path) {
    // Create a simple raster using gdal_rasterize
    // First create a vector, then rasterize

    // Simpler: use gdal's MEM driver and translate
    let mem_tiff = "/vsimem/temp_base.tif";

    let output = Command::new("gdal_translate")
        .arg("-of")
        .arg("MEM")
        .arg("-outsize")
        .arg("64")
        .arg("64")
        .arg("-ot")
        .arg("Byte")
        .arg("-b")
        .arg("1")
        .arg("-b")
        .arg("2")
        .arg("-b")
        .arg("3")
        .arg("/vsimem/constant.tif")
        .arg(mem_tiff)
        .output();

    // If all else fails, create a minimal TIFF manually
    if output.is_err() || !output.as_ref().unwrap().status.success() {
        create_minimal_tiff_manual(path);
    }
}

fn create_minimal_tiff_manual(path: &Path) {
    // Create a minimal valid TIFF file manually for testing
    // This is a simple 2x2 RGB TIFF

    // TIFF header (little-endian): II + 42 + IFD offset
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00]); // Little-endian TIFF
    data.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // IFD offset (8)

    // IFD (12 entries for basic TIFF)
    data.extend_from_slice(&[0x0C, 0x00]); // Number of entries

    // We'll use a simpler approach: create with Rust tiff crate
    use std::fs::File;
    use tiff::encoder::colortype::RGB8;
    use tiff::encoder::TiffEncoder;

    let file = File::create(path).expect("Failed to create TIFF");
    let mut encoder = TiffEncoder::new(file).expect("Failed to create encoder");
    let image = encoder
        .new_image::<RGB8>(64, 64)
        .expect("Failed to create image");

    // Create gradient pattern
    let mut img_data = Vec::with_capacity(64 * 64 * 3);
    for y in 0..64 {
        for x in 0..64 {
            img_data.push((x * 4) as u8); // R gradient
            img_data.push((y * 4) as u8); // G gradient
            img_data.push(128); // B constant
        }
    }

    image.write_data(&img_data).expect("Failed to write data");
    println!("Created base TIFF: {:?}", path);
}

fn create_16bit_tiff(fixtures_dir: &Path) {
    let path = fixtures_dir.join("test_16bit_lzw.tif");

    use std::fs::File;
    use tiff::encoder::colortype::Gray16;
    use tiff::encoder::TiffEncoder;

    let file = File::create(&path).expect("Failed to create 16-bit TIFF");
    let mut encoder = TiffEncoder::new(file).expect("Failed to create encoder");
    let image = encoder
        .new_image::<Gray16>(32, 32)
        .expect("Failed to create image");

    // Create 16-bit gradient
    let mut img_data = Vec::with_capacity(32 * 32);
    for i in 0..(32 * 32) {
        img_data.push((i * 2) as u16);
    }

    image.write_data(&img_data).expect("Failed to write data");
    println!("Created 16-bit TIFF: {:?}", path);
}

fn create_multiband_tiff(fixtures_dir: &Path) {
    // Create a 4-band (RGBA) TIFF
    let path = fixtures_dir.join("test_rgba_lzw.tif");

    use std::fs::File;
    use tiff::encoder::colortype::RGBA8;
    use tiff::encoder::TiffEncoder;

    let file = File::create(&path).expect("Failed to create RGBA TIFF");
    let mut encoder = TiffEncoder::new(file).expect("Failed to create encoder");
    let image = encoder
        .new_image::<RGBA8>(32, 32)
        .expect("Failed to create image");

    // Create RGBA data
    let mut img_data = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            img_data.push((x * 8) as u8); // R
            img_data.push((y * 8) as u8); // G
            img_data.push(128); // B
            img_data.push(255); // A (opaque)
        }
    }

    image.write_data(&img_data).expect("Failed to write data");
    println!("Created RGBA TIFF: {:?}", path);
}
