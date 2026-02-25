use clap::Parser;
use flate2::write::GzEncoder;
use flate2::Compression;
use png::{Encoder, Filter};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Magic bytes for ZIP files (PK\x03\x04)
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];
/// Magic bytes for PNG files (\x89PNG\r\n\x1a\n)
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// Magic bytes for GZ files (\x1f\x8b)
const GZ_MAGIC: &[u8] = &[0x1F, 0x8B];

/// File types detected by magic bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileType {
    Zip,
    Gz,
    Png,
}

/// Detect file type by reading magic bytes from the beginning of the file
fn detect_file_type(path: &Path) -> Option<FileType> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0u8; 8];
    
    // Read enough bytes to check all magic signatures
    let bytes_read = file.read(&mut buffer).ok()?;
    
    // Check for PNG (8 bytes)
    if bytes_read >= PNG_MAGIC.len() && buffer[..PNG_MAGIC.len()] == *PNG_MAGIC {
        return Some(FileType::Png);
    }
    
    // Check for ZIP (4 bytes)
    if bytes_read >= ZIP_MAGIC.len() && buffer[..ZIP_MAGIC.len()] == *ZIP_MAGIC {
        return Some(FileType::Zip);
    }
    
    // Check for GZ (2 bytes)
    if bytes_read >= GZ_MAGIC.len() && buffer[..GZ_MAGIC.len()] == *GZ_MAGIC {
        return Some(FileType::Gz);
    }
    
    None
}

/// Decompress files for better git storage
///
/// Handles ZIP-based files (docx, xlsx, ipynb, etc.), GZ files, and PNG images.
/// For PNG, applies Paeth filter with zero compression.
/// File type detection is based on magic bytes, not file extensions.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Files or directories to process
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Output directory (default: overwrite in place)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Process files recursively in directories
    #[arg(short, long, default_value = "true")]
    recursive: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    for path in &args.paths {
        if path.is_dir() {
            let entries: Vec<_> = WalkDir::new(path)
                .max_depth(if args.recursive { usize::MAX } else { 1 })
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .collect();

            for entry in entries {
                process_file(entry.path(), args.output.as_ref(), args.verbose);
            }
        } else {
            process_file(path, args.output.as_ref(), args.verbose);
        }
    }
}

fn process_file(path: &Path, output_dir: Option<&PathBuf>, verbose: bool) {
    // Detect file type by magic bytes
    let file_type = detect_file_type(path);

    let result = match file_type {
        Some(FileType::Png) => process_png(path, output_dir, verbose),
        Some(FileType::Gz) => process_gz(path, output_dir, verbose),
        Some(FileType::Zip) => process_zip_based(path, output_dir, verbose),
        None => {
            if verbose {
                println!("Skipping unsupported file type: {}", path.display());
            }
            return;
        }
    };

    match result {
        Ok(new_path) => {
            if verbose {
                println!("Processed: {} -> {}", path.display(), new_path.display());
            }
        }
        Err(e) => {
            eprintln!("Error processing {}: {}", path.display(), e);
        }
    }
}

/// Process ZIP-based files (docx, xlsx, ipynb, etc.)
/// Recompress with STORED method (no compression)
fn process_zip_based(
    path: &Path,
    output_dir: Option<&PathBuf>,
    _verbose: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = determine_output_path(path, output_dir)?;

    let input_file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(input_file)?;

    let output_file = File::create(&output_path)?;
    let mut zip_writer = ZipWriter::new(output_file);

    // Use STORED method (no compression) for all entries
    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = entry.name().to_string();

        // Skip directories
        if entry.name().ends_with('/') {
            continue;
        }

        zip_writer.start_file(&outpath, options)?;

        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)?;
        zip_writer.write_all(&buffer)?;
    }

    zip_writer.finish()?;

    Ok(output_path)
}

/// Process GZ files
/// Decompress and recompress with no compression (stored as raw deflate with level 0)
fn process_gz(
    path: &Path,
    output_dir: Option<&PathBuf>,
    _verbose: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = determine_output_path(path, output_dir)?;

    // Read and decompress the input
    let input_file = File::open(path)?;
    let mut decoder = flate2::read::GzDecoder::new(input_file);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;

    // Recompress with zero compression level
    let output_file = File::create(&output_path)?;
    let mut encoder = GzEncoder::new(output_file, Compression::none());
    encoder.write_all(&decompressed_data)?;
    encoder.finish()?;

    Ok(output_path)
}

/// Process PNG files
/// Apply Paeth filter with zero compression
fn process_png(
    path: &Path,
    output_dir: Option<&PathBuf>,
    verbose: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = determine_output_path(path, output_dir)?;

    // Read the PNG file
    let file = File::open(path)?;
    let decoder = png::Decoder::new(BufReader::new(file));

    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().expect("Failed to get buffer size")];
    let info = reader.next_frame(&mut buf)?;

    // Calculate actual data size (height * (row_bytes + 1 for filter byte))
    let bytes_per_pixel = info.color_type.samples() as usize;
    let row_bytes = info.width as usize * bytes_per_pixel;
    let actual_data_size = info.height as usize * (row_bytes + 1);
    let data = &buf[..actual_data_size.min(buf.len())];

    // Create output PNG with Paeth filter and no compression
    let output_file = File::create(&output_path)?;
    let mut encoder = Encoder::new(output_file, info.width, info.height);
    encoder.set_color(info.color_type);
    encoder.set_depth(info.bit_depth);
    encoder.set_filter(Filter::Paeth);
    encoder.set_compression(png::Compression::NoCompression);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(data)?;
    writer.finish()?;

    if verbose {
        println!(
            "PNG: {}x{}, {:?}, {:?}, Paeth filter, no compression",
            info.width, info.height, info.color_type, info.bit_depth
        );
    }

    Ok(output_path)
}

/// Determine the output path for a processed file
fn determine_output_path(
    input_path: &Path,
    output_dir: Option<&PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = output_dir {
        fs::create_dir_all(dir)?;
        Ok(dir.join(input_path.file_name().ok_or("Invalid filename")?))
    } else {
        // Overwrite in place - create temp name then rename
        let temp_path = input_path.with_extension(format!(
            "{}.tmp",
            input_path.extension().unwrap_or_default().to_string_lossy()
        ));
        Ok(temp_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_determine_output_path_with_output_dir() {
        let temp_dir = TempDir::new().unwrap();
        let output_dir = Some(temp_dir.path().to_path_buf());
        let input_path = PathBuf::from("/some/path/test.zip");

        let result = determine_output_path(&input_path, output_dir.as_ref()).unwrap();
        assert_eq!(result, temp_dir.path().join("test.zip"));
    }

    #[test]
    fn test_determine_output_path_in_place() {
        let input_path = PathBuf::from("/some/path/test.zip");
        let output_dir: Option<&PathBuf> = None;

        let result = determine_output_path(&input_path, output_dir).unwrap();
        assert_eq!(result, PathBuf::from("/some/path/test.zip.tmp"));
    }

    #[test]
    fn test_detect_file_type_zip() {
        let temp_dir = TempDir::new().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        
        // Create a ZIP file
        {
            use zip::write::FileOptions;
            use zip::ZipWriter;

            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<'_, ()> = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("test.txt", options).unwrap();
            zip.write_all(b"Test").unwrap();
            zip.finish().unwrap();
        }

        // Detect file type - should work regardless of extension
        let detected = detect_file_type(&zip_path);
        assert_eq!(detected, Some(FileType::Zip));
    }

    #[test]
    fn test_detect_file_type_zip_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        let zip_path = temp_dir.path().join("test.dat"); // Wrong extension
        
        // Create a ZIP file with wrong extension
        {
            use zip::write::FileOptions;
            use zip::ZipWriter;

            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<'_, ()> = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("test.txt", options).unwrap();
            zip.write_all(b"Test").unwrap();
            zip.finish().unwrap();
        }

        // Should detect ZIP despite wrong extension
        let detected = detect_file_type(&zip_path);
        assert_eq!(detected, Some(FileType::Zip));
    }

    #[test]
    fn test_detect_file_type_png() {
        let temp_dir = TempDir::new().unwrap();
        let png_path = temp_dir.path().join("test.png");
        
        // Create a minimal PNG file
        create_minimal_png(&png_path);

        // Detect file type
        let detected = detect_file_type(&png_path);
        assert_eq!(detected, Some(FileType::Png));
    }

    #[test]
    fn test_detect_file_type_png_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        let png_path = temp_dir.path().join("test.bin"); // Wrong extension
        
        // Create a PNG file with wrong extension
        create_minimal_png(&png_path);

        // Should detect PNG despite wrong extension
        let detected = detect_file_type(&png_path);
        assert_eq!(detected, Some(FileType::Png));
    }

    #[test]
    fn test_detect_file_type_gz() {
        let temp_dir = TempDir::new().unwrap();
        let gz_path = temp_dir.path().join("test.gz");
        
        // Create a GZ file
        {
            use flate2::write::GzEncoder;
            use flate2::Compression;

            let file = File::create(&gz_path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(b"Test data").unwrap();
            encoder.finish().unwrap();
        }

        // Detect file type
        let detected = detect_file_type(&gz_path);
        assert_eq!(detected, Some(FileType::Gz));
    }

    #[test]
    fn test_detect_file_type_gz_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        let gz_path = temp_dir.path().join("test.data"); // Wrong extension
        
        // Create a GZ file with wrong extension
        {
            use flate2::write::GzEncoder;
            use flate2::Compression;

            let file = File::create(&gz_path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(b"Test data").unwrap();
            encoder.finish().unwrap();
        }

        // Should detect GZ despite wrong extension
        let detected = detect_file_type(&gz_path);
        assert_eq!(detected, Some(FileType::Gz));
    }

    #[test]
    fn test_detect_file_type_unsupported() {
        let temp_dir = TempDir::new().unwrap();
        let txt_path = temp_dir.path().join("test.txt");
        
        // Create a text file (unsupported)
        std::fs::write(&txt_path, "Hello, World!").unwrap();

        // Should not detect any supported type
        let detected = detect_file_type(&txt_path);
        assert_eq!(detected, None);
    }

    #[test]
    fn test_create_and_process_zip() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.zip");
        let output_dir = temp_dir.path().join("output");

        // Create a test ZIP file
        {
            use zip::write::FileOptions;
            use zip::ZipWriter;

            let file = File::create(&input_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<'_, ()> = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip.start_file("content.txt", options).unwrap();
            zip.write_all(b"Test content").unwrap();
            zip.finish().unwrap();
        }

        // Process the ZIP file
        let result = process_zip_based(&input_path, Some(&output_dir), false);
        assert!(result.is_ok());

        let output_path = result.unwrap();
        assert!(output_path.exists());

        // Verify the output ZIP can be read
        let output_file = File::open(&output_path).unwrap();
        let mut archive = zip::ZipArchive::new(output_file).unwrap();
        assert_eq!(archive.len(), 1);

        let entry = archive.by_index(0).unwrap();
        assert_eq!(entry.name(), "content.txt");
    }

    /// Helper function to create a minimal valid PNG file
    fn create_minimal_png(path: &Path) {
        use png::{BitDepth, ColorType, Encoder};
        
        let width = 2;
        let height = 2;
        // 2x2 RGB image = 2 * 2 * 3 = 12 bytes
        let data = vec![
            0u8, 0, 0,       // Pixel 1: RGB
            255, 255, 255,   // Pixel 2: RGB
            128, 128, 128,   // Pixel 3: RGB
            64, 64, 64,      // Pixel 4: RGB
        ];
        
        let file = File::create(path).unwrap();
        let mut encoder = Encoder::new(file, width, height);
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);
        
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
    }
}
