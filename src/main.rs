use clap::Parser;
use flate2::write::GzEncoder;
use flate2::Compression;
use png::{Encoder, Filter};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tiff::decoder::ifd::Value;
use tiff::encoder::colortype;
use tiff::encoder::{Compression as TiffCompression, Predictor, TiffEncoder, TiffKindStandard};
use tiff::tags::Tag;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Magic bytes for ZIP files (PK\x03\x04)
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];
/// Magic bytes for PNG files (\x89PNG\r\n\x1a\n)
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// Magic bytes for GZ files (\x1f\x8b)
const GZ_MAGIC: &[u8] = &[0x1F, 0x8B];
/// Magic bytes for TIFF little-endian (II*\x00)
const TIFF_LE_MAGIC: &[u8] = &[0x49, 0x49, 0x2A, 0x00];
/// Magic bytes for TIFF big-endian (MM\x00*)
const TIFF_BE_MAGIC: &[u8] = &[0x4D, 0x4D, 0x00, 0x2A];

/// File types detected by magic bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileType {
    Zip,
    Gz,
    Png,
    Tiff,
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

    // Check for TIFF (4 bytes) - both little-endian and big-endian
    if bytes_read >= 4 && (buffer[..4] == *TIFF_LE_MAGIC || buffer[..4] == *TIFF_BE_MAGIC) {
        return Some(FileType::Tiff);
    }

    None
}

/// Decompress files for better git storage
///
/// Handles ZIP-based files (docx, xlsx, ipynb, etc.), GZ files, PNG images, and TIFF/GeoTIFF files.
/// For PNG, applies Paeth filter with zero compression.
/// For TIFF, recompresses with no compression and predictor filter.
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

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = Args::parse();
    let mut success = true;

    for path in &args.paths {
        if path.is_dir() {
            let entries: Vec<_> = WalkDir::new(path)
                .max_depth(if args.recursive { usize::MAX } else { 1 })
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .collect();

            for entry in entries {
                if let Err(e) = process_file(entry.path(), args.output.as_ref(), args.verbose) {
                    eprintln!("Error processing {}: {}", entry.path().display(), e);
                    success = false;
                }
            }
        } else if let Err(e) = process_file(path, args.output.as_ref(), args.verbose) {
            eprintln!("Error processing {}: {}", path.display(), e);
            success = false;
        }
    }

    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn process_file(
    path: &Path,
    output_dir: Option<&PathBuf>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Detect file type by magic bytes
    let file_type = match detect_file_type(path) {
        Some(ft) => ft,
        None => {
            if verbose {
                println!("Skipping unsupported file type: {}", path.display());
            }
            return Ok(());
        }
    };

    let output_path = determine_output_path(path, output_dir)?;

    let result = match file_type {
        FileType::Png => process_png(path, &output_path, verbose),
        FileType::Gz => process_gz(path, &output_path, verbose),
        FileType::Zip => process_zip_based(path, &output_path, verbose),
        FileType::Tiff => process_tiff(path, &output_path, verbose),
    };

    if let Err(e) = result {
        // Clean up partial output file if it exists
        if output_path.exists() {
            let _ = fs::remove_file(&output_path);
        }
        return Err(e);
    }

    // If processing in-place (no output dir), rename temp file to original
    if output_dir.is_none() && output_path != path {
        if let Err(e) = fs::rename(&output_path, path) {
            // Cleanup temp file on rename failure
            if output_path.exists() {
                let _ = fs::remove_file(&output_path);
            }
            return Err(e.into());
        }
        if verbose {
            println!("Processed: {}", path.display());
        }
    } else if verbose {
        println!("Processed: {} -> {}", path.display(), output_path.display());
    }

    Ok(())
}

/// Process ZIP-based files (docx, xlsx, ipynb, etc.)
/// Recompress with STORED method (no compression)
/// Uses streaming to avoid loading entire file into memory
fn process_zip_based(
    path: &Path,
    output_path: &Path,
    _verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(input_file)?;

    let output_file = File::create(output_path)?;
    let mut zip_writer = ZipWriter::new(output_file);

    // Use STORED method (no compression) for all entries
    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = entry.name().to_string();

        // Skip directories
        if entry.name().ends_with('/') {
            continue;
        }

        zip_writer.start_file(&outpath, options)?;

        // Stream the entry data directly to the writer (no buffering in memory)
        std::io::copy(&mut entry, &mut zip_writer)?;
    }

    zip_writer.finish()?;

    Ok(())
}

/// Process GZ files
/// Decompress and recompress with no compression (stored as raw deflate with level 0)
/// Uses streaming to avoid loading entire file into memory
fn process_gz(
    path: &Path,
    output_path: &Path,
    _verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Stream decompression directly to recompression (no intermediate buffer)
    let input_file = File::open(path)?;
    let decoder = flate2::read::GzDecoder::new(input_file);

    let output_file = File::create(output_path)?;
    let mut encoder = GzEncoder::new(output_file, Compression::none());

    std::io::copy(&mut decoder.take(u64::MAX), &mut encoder)?;
    encoder.finish()?;

    Ok(())
}

/// Process PNG files
/// Apply Paeth filter with zero compression
/// Note: PNG processing requires holding one frame in memory for filter application
fn process_png(
    path: &Path,
    output_path: &Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the PNG file using streaming decoder
    let file = File::open(path)?;
    let decoder = png::Decoder::new(BufReader::new(file));

    let mut reader = decoder.read_info()?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .expect("Failed to get buffer size")
    ];
    let info = reader.next_frame(&mut buf)?;

    // Use actual decoded data size from info
    let actual_data_size = info.height as usize * info.line_size;
    let data = &buf[..actual_data_size.min(buf.len())];

    // Create output PNG with Paeth filter and no compression
    let output_file = File::create(output_path)?;
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

    Ok(())
}

/// Process TIFF files (including GeoTIFF)
/// Recompress with no compression and horizontal predictor
/// Preserves all TIFF tags including GeoTIFF metadata
fn process_tiff(
    path: &Path,
    output_path: &Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the TIFF file
    let mut decoder = tiff::decoder::Decoder::new(File::open(path)?)?;

    // Get image information
    let width = decoder.dimensions()?.0;
    let height = decoder.dimensions()?.1;
    let photometric_interpretation: u16 = decoder
        .get_tag(tiff::tags::Tag::PhotometricInterpretation)?
        .into_u16()?;

    // Read all tags from the input file for preservation (including GeoTIFF tags)
    let mut preserved_tags: Vec<(Tag, Value)> = Vec::new();
    
    // Use tag_iter to read all tags from the current IFD
    for result in decoder.tag_iter() {
        if let Ok((tag, value)) = result {
            // Skip tags that will be rewritten by the encoder
            if matches!(
                tag,
                Tag::StripOffsets
                    | Tag::StripByteCounts
                    | Tag::TileOffsets
                    | Tag::TileByteCounts
                    | Tag::JPEGTables
                    | Tag::Compression
                    | Tag::Predictor
                    | Tag::ImageWidth
                    | Tag::ImageLength
                    | Tag::BitsPerSample
                    | Tag::PhotometricInterpretation
                    | Tag::SamplesPerPixel
                    | Tag::RowsPerStrip
                    | Tag::PlanarConfiguration
            ) {
                continue;
            }
            preserved_tags.push((tag, value));
        }
    }

    // Read the image data
    let image = decoder.read_image()?;

    // Create output TIFF with no compression and predictor
    let mut encoder = TiffEncoder::new(File::create(output_path)?)?
        .with_compression(TiffCompression::Uncompressed)
        .with_predictor(Predictor::Horizontal);

    // Write the image data based on the decoded type and preserve tags
    match image {
        tiff::decoder::DecodingResult::U8(data) => {
            // Determine color type based on photometric interpretation and samples
            let samples: u16 = decoder
                .get_tag(tiff::tags::Tag::SamplesPerPixel)?
                .into_u16()?;

            if samples == 1 {
                // Grayscale
                let mut image_encoder = encoder.new_image::<colortype::Gray8>(width, height)?;
                write_preserved_tags_8(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else if samples == 3 {
                // RGB
                let mut image_encoder = encoder.new_image::<colortype::RGB8>(width, height)?;
                write_preserved_tags_8(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else if samples == 4 {
                // RGBA
                let mut image_encoder = encoder.new_image::<colortype::RGBA8>(width, height)?;
                write_preserved_tags_8(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else {
                return Err(
                    format!("Unsupported number of samples for 8-bit TIFF: {}", samples).into(),
                );
            }
        }
        tiff::decoder::DecodingResult::U16(data) => {
            // For 16-bit images
            let samples: u16 = decoder
                .get_tag(tiff::tags::Tag::SamplesPerPixel)?
                .into_u16()?;

            if samples == 1 {
                let mut image_encoder = encoder.new_image::<colortype::Gray16>(width, height)?;
                write_preserved_tags_16(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else if samples == 3 {
                let mut image_encoder = encoder.new_image::<colortype::RGB16>(width, height)?;
                write_preserved_tags_16(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else if samples == 4 {
                let mut image_encoder = encoder.new_image::<colortype::RGBA16>(width, height)?;
                write_preserved_tags_16(&mut image_encoder, &preserved_tags)?;
                image_encoder.write_data(&data)?;
            } else {
                return Err(
                    format!("Unsupported number of samples for 16-bit TIFF: {}", samples).into(),
                );
            }
        }
        _ => {
            return Err("Unsupported TIFF bit depth".into());
        }
    }

    if verbose {
        println!(
            "TIFF: {}x{}, photometric={}, {} tags preserved, uncompressed with horizontal predictor",
            width, height, photometric_interpretation, preserved_tags.len()
        );
    }

    Ok(())
}

/// Write preserved tags to the 8-bit image encoder
fn write_preserved_tags_8<C: colortype::ColorType<Inner = u8>>(
    image_encoder: &mut tiff::encoder::ImageEncoder<'_, File, C, TiffKindStandard>,
    preserved_tags: &[(Tag, Value)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut dir = image_encoder.encoder();
    for (tag, value) in preserved_tags {
        write_tag_value(&mut dir, *tag, value)?;
    }
    Ok(())
}

/// Write preserved tags to the 16-bit image encoder
fn write_preserved_tags_16<C: colortype::ColorType<Inner = u16>>(
    image_encoder: &mut tiff::encoder::ImageEncoder<'_, File, C, TiffKindStandard>,
    preserved_tags: &[(Tag, Value)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut dir = image_encoder.encoder();
    for (tag, value) in preserved_tags {
        write_tag_value(&mut dir, *tag, value)?;
    }
    Ok(())
}

/// Write a single tag value to the directory encoder
fn write_tag_value(
    dir: &mut tiff::encoder::DirectoryEncoder<'_, File, TiffKindStandard>,
    tag: Tag,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    #[allow(deprecated)]
    match value {
        Value::Byte(v) => { dir.write_tag(tag, *v)?; }
        Value::Short(v) => { dir.write_tag(tag, *v)?; }
        Value::SignedByte(v) => { dir.write_tag(tag, *v)?; }
        Value::SignedShort(v) => { dir.write_tag(tag, *v)?; }
        Value::Signed(v) => { dir.write_tag(tag, *v)?; }
        Value::SignedBig(v) => { dir.write_tag(tag, *v)?; }
        Value::Unsigned(v) => { dir.write_tag(tag, *v)?; }
        Value::UnsignedBig(v) => { dir.write_tag(tag, *v)?; }
        Value::Float(v) => { dir.write_tag(tag, *v)?; }
        Value::Double(v) => { dir.write_tag(tag, *v)?; }
        Value::Ifd(v) => { dir.write_tag(tag, *v)?; }
        Value::IfdBig(v) => { dir.write_tag(tag, *v)?; }
        Value::Ascii(v) => { dir.write_tag(tag, v.as_str())?; }
        Value::Rational(n, d) => { 
            dir.write_tag(tag, [*n, *d].as_slice())?; 
        }
        Value::SRational(n, d) => { 
            dir.write_tag(tag, [*n, *d].as_slice())?; 
        }
        Value::RationalBig(n, d) => { 
            dir.write_tag(tag, [*n, *d].as_slice())?; 
        }
        Value::SRationalBig(n, d) => { 
            dir.write_tag(tag, [*n, *d].as_slice())?; 
        }
        Value::List(values) => {
            if let Some(first) = values.first() {
                match first {
                    Value::Byte(_) => {
                        let bytes: Vec<u8> = values.iter().filter_map(|v| match v {
                            Value::Byte(b) => Some(*b),
                            _ => None,
                        }).collect();
                        dir.write_tag(tag, bytes.as_slice())?;
                    }
                    Value::Short(_) => {
                        let shorts: Vec<u16> = values.iter().filter_map(|v| match v {
                            Value::Short(s) => Some(*s),
                            _ => None,
                        }).collect();
                        dir.write_tag(tag, shorts.as_slice())?;
                    }
                    Value::Unsigned(_) => {
                        let longs: Vec<u32> = values.iter().filter_map(|v| match v {
                            Value::Unsigned(l) => Some(*l),
                            _ => None,
                        }).collect();
                        dir.write_tag(tag, longs.as_slice())?;
                    }
                    Value::Float(_) => {
                        let floats: Vec<f32> = values.iter().filter_map(|v| match v {
                            Value::Float(f) => Some(*f),
                            _ => None,
                        }).collect();
                        dir.write_tag(tag, floats.as_slice())?;
                    }
                    Value::Double(_) => {
                        let doubles: Vec<f64> = values.iter().filter_map(|v| match v {
                            Value::Double(d) => Some(*d),
                            _ => None,
                        }).collect();
                        dir.write_tag(tag, doubles.as_slice())?;
                    }
                    _ => {
                        eprintln!("Warning: Unsupported list value type for tag {:?}", tag);
                    }
                }
            }
        }
        _ => {
            eprintln!("Warning: Unsupported tag value type for tag {:?}", tag);
        }
    }
    Ok(())
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
        // Overwrite in place - create temp file with .unc. prefix
        // e.g., file.zip -> .unc.file.zip
        let file_name = input_path.file_name().ok_or("Invalid filename")?;
        let parent = input_path.parent().unwrap_or(Path::new(""));
        let temp_name = format!(".unc.{}", file_name.to_string_lossy());
        Ok(parent.join(temp_name))
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
        assert_eq!(result, PathBuf::from("/some/path/.unc.test.zip"));
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
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
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
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
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
    fn test_detect_file_type_tiff() {
        let temp_dir = TempDir::new().unwrap();
        let tiff_path = temp_dir.path().join("test.tiff");

        // Create a TIFF file
        create_minimal_tiff(&tiff_path);

        // Detect file type
        let detected = detect_file_type(&tiff_path);
        assert_eq!(detected, Some(FileType::Tiff));
    }

    #[test]
    fn test_detect_file_type_tiff_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        let tiff_path = temp_dir.path().join("test.dat"); // Wrong extension

        // Create a TIFF file with wrong extension
        create_minimal_tiff(&tiff_path);

        // Should detect TIFF despite wrong extension
        let detected = detect_file_type(&tiff_path);
        assert_eq!(detected, Some(FileType::Tiff));
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
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            zip.start_file("content.txt", options).unwrap();
            zip.write_all(b"Test content").unwrap();
            zip.finish().unwrap();
        }

        // Process the ZIP file
        let output_path = output_dir.join("test.zip");
        fs::create_dir_all(&output_dir).unwrap();
        let result = process_zip_based(&input_path, &output_path, false);
        assert!(result.is_ok());

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
            0u8, 0, 0, // Pixel 1: RGB
            255, 255, 255, // Pixel 2: RGB
            128, 128, 128, // Pixel 3: RGB
            64, 64, 64, // Pixel 4: RGB
        ];

        let file = File::create(path).unwrap();
        let mut encoder = Encoder::new(file, width, height);
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);

        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
    }

    /// Helper function to create a minimal valid TIFF file
    fn create_minimal_tiff(path: &Path) {
        use tiff::encoder::colortype::RGB8;
        use tiff::encoder::TiffEncoder;

        let file = File::create(path).unwrap();
        let mut encoder = TiffEncoder::new(file).unwrap();
        let image = encoder.new_image::<RGB8>(2, 2).unwrap();

        // 2x2 RGB image = 2 * 2 * 3 = 12 bytes
        let data = vec![
            0u8, 0, 0, // Pixel 1: RGB
            255, 255, 255, // Pixel 2: RGB
            128, 128, 128, // Pixel 3: RGB
            64, 64, 64, // Pixel 4: RGB
        ];

        image.write_data(&data).unwrap();
    }
}
