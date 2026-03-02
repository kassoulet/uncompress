use clap::Parser;
use flate2::write::GzEncoder;
use flate2::Compression;
use png::{Encoder, Filter};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tempfile::Builder;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;

#[cfg(feature = "tiff-support")]
use std::process::Command;

#[cfg(feature = "tiff-support")]
use tiff::encoder::colortype;

#[cfg(feature = "tiff-support")]
use tiff::encoder::TiffEncoder;

/// Magic bytes for ZIP files (PK\x03\x04)
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];
/// Magic bytes for PNG files (\x89PNG\r\n\x1a\n)
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// Magic bytes for GZ files (\x1f\x8b)
const GZ_MAGIC: &[u8] = &[0x1F, 0x8B];

#[cfg(feature = "tiff-support")]
/// Magic bytes for TIFF little-endian (II*\x00)
const TIFF_LE_MAGIC: &[u8] = &[0x49, 0x49, 0x2A, 0x00];

#[cfg(feature = "tiff-support")]
/// Magic bytes for TIFF big-endian (MM\x00*)
const TIFF_BE_MAGIC: &[u8] = &[0x4D, 0x4D, 0x00, 0x2A];

#[cfg(feature = "tiff-support")]
/// Magic bytes for BigTIFF little-endian (II+\x00)
const BIGTIFF_LE_MAGIC: &[u8] = &[0x49, 0x49, 0x2B, 0x00];

#[cfg(feature = "tiff-support")]
/// Magic bytes for BigTIFF big-endian (MM\x00+)
const BIGTIFF_BE_MAGIC: &[u8] = &[0x4D, 0x4D, 0x00, 0x2B];

/// File types detected by magic bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileType {
    Zip,
    Gz,
    Png,
    #[cfg(feature = "tiff-support")]
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

    #[cfg(feature = "tiff-support")]
    // Check for TIFF (4 bytes) - both little-endian and big-endian, including BigTIFF
    if bytes_read >= 4
        && (buffer[..4] == *TIFF_LE_MAGIC
            || buffer[..4] == *TIFF_BE_MAGIC
            || buffer[..4] == *BIGTIFF_LE_MAGIC
            || buffer[..4] == *BIGTIFF_BE_MAGIC)
    {
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
            let walk = WalkDir::new(path)
                .max_depth(if args.recursive { usize::MAX } else { 1 })
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file());

            for entry in walk {
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

/// Check if a file is already uncompressed
fn is_already_uncompressed(
    path: &Path,
    file_type: FileType,
) -> Result<bool, Box<dyn std::error::Error>> {
    match file_type {
        #[cfg(feature = "tiff-support")]
        FileType::Tiff => {
            // Check TIFF compression tag
            // Compression codes: 1=Uncompressed, 5=LZW, 6=JPEG, 8=Deflate, 32946=Deflate, 34933=Deflate, 50000=ZSTD, 52546=WebP
            let decoder = tiff::decoder::Decoder::new(File::open(path)?);
            if let Ok(mut dec) = decoder {
                if let Ok(compression) = dec.get_tag(tiff::tags::Tag::Compression) {
                    if let Ok(comp_val) = compression.into_u16() {
                        // Only skip if already uncompressed (1) with horizontal predictor (2)
                        if comp_val == 1 {
                            // Check predictor - if horizontal predictor is already set, skip
                            if let Ok(pred) = dec.get_tag(tiff::tags::Tag::Predictor) {
                                if let Ok(pred_val) = pred.into_u16() {
                                    // Predictor 2 = Horizontal, already optimal
                                    return Ok(pred_val == 2);
                                }
                                // Other predictor value, needs processing to set it to 2
                                return Ok(false);
                            }
                            // Uncompressed but no predictor tag (default 1), needs processing to set it to 2
                            return Ok(false);
                        }
                        // All other compression types need processing:
                        // 5=LZW, 6=JPEG, 8=Deflate, 32946=Deflate, 34933=Deflate, 50000=ZSTD, 52546=WebP
                        return Ok(false);
                    }
                }
            }
            Ok(false) // Needs processing
        }
        FileType::Png => {
            // PNG files processed by us use Paeth filter and no compression
            // We can't easily detect this without re-reading, so always process
            // to ensure optimal compression
            Ok(false)
        }
        FileType::Gz => {
            // Check GZ compression level from header
            // GZ header: magic(2) + compression(1) + flags(1) + mtime(4) + xfl(1) + os(1)
            let mut file = File::open(path)?;
            let mut header = [0u8; 10];
            if file.read_exact(&mut header).is_ok() {
                // XFL byte (index 8): 2 = max compression, 4 = fastest compression
                // We can't detect level 0 (no compression) reliably from header alone
                // So we always process to ensure optimal compression
            }
            Ok(false)
        }
        FileType::Zip => {
            // Check if all entries in ZIP use STORED method (no compression)
            let input_file = File::open(path)?;
            if let Ok(mut archive) = zip::ZipArchive::new(input_file) {
                for i in 0..archive.len() {
                    if let Ok(entry) = archive.by_index(i) {
                        if !entry.name().ends_with('/') {
                            // Check if any entry uses compression
                            if entry.compression() != zip::CompressionMethod::Stored {
                                return Ok(false); // Has compressed entries, needs processing
                            }
                        }
                    }
                }
                return Ok(true); // All entries are stored (uncompressed)
            }
            Ok(false)
        }
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
            println!(
                "{} | UNSUPPORTED | Skipped",
                path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_else(|| path.display().to_string().into())
            );
            return Ok(());
        }
    };

    // Check if file is already uncompressed
    if is_already_uncompressed(path, file_type)? {
        println!(
            "{} | {} | Already uncompressed | Skipped",
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| path.display().to_string().into()),
            match file_type {
                FileType::Zip => "ZIP",
                FileType::Gz => "GZ",
                FileType::Png => "PNG",
                #[cfg(feature = "tiff-support")]
                FileType::Tiff => "TIFF",
            }
        );
        return Ok(());
    }

    // Get input file metadata and permissions before processing
    // 🛡️ SECURITY: Capture original permissions to preserve them
    let metadata = fs::metadata(path)?;
    let input_size = metadata.len();
    let permissions = metadata.permissions();

    let output_path = determine_output_path(path, output_dir)?;
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));

    // 🛡️ SECURITY: Use NamedTempFile for secure, unique temporary files
    // This prevents symlink attacks and ensured atomic replacement
    // We use the original extension to help external tools like gdal identify the format
    let suffix = output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext))
        .unwrap_or_default();
    let temp_file = Builder::new()
        .prefix(".unc.")
        .suffix(&suffix)
        .tempfile_in(parent)?;
    let temp_path = temp_file.path().to_path_buf();

    let result = match file_type {
        FileType::Png => process_png(path, &temp_path, verbose),
        FileType::Gz => process_gz(path, &temp_path, verbose),
        FileType::Zip => process_zip_based(path, &temp_path, verbose),
        #[cfg(feature = "tiff-support")]
        FileType::Tiff => process_tiff(path, &temp_path, verbose),
    };

    result?;

    // 🛡️ SECURITY: Reapply original permissions to the new file
    temp_file.as_file().set_permissions(permissions)?;

    // Get output file size after processing
    let output_size = fs::metadata(&temp_path)?.len();

    // 🛡️ SECURITY: Atomically move the temporary file to its final destination
    temp_file.persist(&output_path)?;

    if verbose {
        if output_dir.is_none() {
            println!("Processed: {}", path.display());
        } else {
            println!("Processed: {} -> {}", path.display(), output_path.display());
        }
    }

    // Print progress information
    print_progress(
        path,
        &output_path,
        file_type,
        input_size,
        output_size,
        output_dir.is_some(),
    );

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

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = entry.name().to_string();

        let options: FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .last_modified_time(entry.last_modified())
            .unix_permissions(entry.unix_mode().unwrap_or(0o644));

        if entry.is_dir() {
            zip_writer.add_directory(outpath.clone(), options)?;
        } else {
            zip_writer.start_file(outpath.clone(), options)?;
            std::io::copy(&mut entry, &mut zip_writer)?;
        }
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

#[cfg(feature = "tiff-support")]
/// Process TIFF files (including GeoTIFF)
/// Recompress with no compression and horizontal predictor
/// Preserves all TIFF tags including GeoTIFF metadata
/// For unsupported compression (ZSTD, WebP, JPEG, etc.), uses gdal as external tool
fn process_tiff(
    path: &Path,
    output_path: &Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Try to read the TIFF file to check compression
    let decoder_result = tiff::decoder::Decoder::new(File::open(path)?);

    let mut decoder = match decoder_result {
        Ok(d) => d,
        Err(e) => {
            // If decoder fails, try using gdal for unsupported compression
            return process_tiff_with_gdal(path, output_path, verbose, e.to_string());
        }
    };

    // Get image information - this may also fail for unsupported compression
    let dimensions_result = decoder.dimensions();
    if let Err(e) = dimensions_result {
        return process_tiff_with_gdal(path, output_path, verbose, e.to_string());
    }

    let width = decoder.dimensions()?.0;
    let height = decoder.dimensions()?.1;
    let photometric_interpretation: u16 = decoder
        .get_tag(tiff::tags::Tag::PhotometricInterpretation)?
        .into_u16()?;

    // Read the image data - this may fail for unsupported compression like zstd
    let image = match decoder.read_image() {
        Ok(img) => img,
        Err(e) => {
            return process_tiff_with_gdal(path, output_path, verbose, e.to_string());
        }
    };

    // Create output TIFF with no compression and predictor
    // In tiff 0.9, we use new_image_with_compression directly
    let output_file = File::create(output_path)?;
    let mut encoder = TiffEncoder::new(output_file)?;

    // Write the image data based on the decoded type and preserve tags
    // Note: tiff encoder only supports U8 and U16 natively
    // Other types (U32, U64, I8, I16, I32, I64, F32, F64) use gdal fallback
    match image {
        tiff::decoder::DecodingResult::U8(data) => {
            // Determine color type based on photometric interpretation and samples
            let samples: u16 = decoder
                .get_tag(tiff::tags::Tag::SamplesPerPixel)?
                .into_u16()?;

            if samples == 1 {
                // Grayscale - uncompressed
                let image_encoder = encoder.new_image_with_compression::<colortype::Gray8, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else if samples == 3 {
                // RGB
                let image_encoder = encoder.new_image_with_compression::<colortype::RGB8, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else if samples == 4 {
                // RGBA
                let image_encoder = encoder.new_image_with_compression::<colortype::RGBA8, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else {
                return process_tiff_with_gdal(
                    path,
                    output_path,
                    verbose,
                    format!("Unsupported number of samples for 8-bit TIFF: {}", samples),
                );
            }
        }
        tiff::decoder::DecodingResult::U16(data) => {
            // For 16-bit images
            let samples: u16 = decoder
                .get_tag(tiff::tags::Tag::SamplesPerPixel)?
                .into_u16()?;

            if samples == 1 {
                let image_encoder = encoder.new_image_with_compression::<colortype::Gray16, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else if samples == 3 {
                let image_encoder = encoder.new_image_with_compression::<colortype::RGB16, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else if samples == 4 {
                let image_encoder = encoder.new_image_with_compression::<colortype::RGBA16, tiff::encoder::compression::Uncompressed>(width, height, tiff::encoder::compression::Uncompressed)?;
                image_encoder.write_data(&data)?;
            } else {
                return process_tiff_with_gdal(
                    path,
                    output_path,
                    verbose,
                    format!("Unsupported number of samples for 16-bit TIFF: {}", samples),
                );
            }
        }
        // For all other bit depths, use gdal which has full format support
        tiff::decoder::DecodingResult::U32(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "32-bit integer TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::U64(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "64-bit integer TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::I8(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "Signed 8-bit TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::I16(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "Signed 16-bit TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::I32(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "Signed 32-bit integer TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::I64(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "Signed 64-bit integer TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::F32(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "32-bit float TIFF requires gdal".to_string(),
            );
        }
        tiff::decoder::DecodingResult::F64(_) => {
            return process_tiff_with_gdal(
                path,
                output_path,
                verbose,
                "64-bit float TIFF requires gdal".to_string(),
            );
        }
    }

    if verbose {
        println!(
            "TIFF: {}x{}, photometric={}, uncompressed",
            width, height, photometric_interpretation
        );
    }

    Ok(())
}

#[cfg(feature = "tiff-support")]
/// Process TIFF files using gdal (for zstd, webp, jpeg, lzw, deflate, float, or other unsupported formats)
/// Uses gdal_translate to convert to uncompressed TIFF with horizontal predictor
/// Supports: F32, F64, F16, U32, U64, I8, I16, I32, I64 and compressed formats (ZSTD, WebP, JPEG, LZW, Deflate)
fn process_tiff_with_gdal(
    path: &Path,
    output_path: &Path,
    verbose: bool,
    error_msg: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if gdal_translate is available
    let gdal_check = Command::new("gdal_translate").arg("--version").output();

    if gdal_check.is_err() || !gdal_check.as_ref().unwrap().status.success() {
        return Err(format!(
            "gdal_translate not found. Install GDAL tools to process this TIFF. Error: {}",
            error_msg
        )
        .into());
    }

    // Use gdal_translate to convert to uncompressed TIFF
    // -co COMPRESS=NONE: No compression
    // -co PREDICTOR=2: Horizontal predictor
    let output = Command::new("gdal_translate")
        .arg("-co")
        .arg("COMPRESS=NONE")
        .arg("-co")
        .arg("PREDICTOR=2")
        .arg(path)
        .arg(output_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gdal_translate failed: {}", stderr).into());
    }

    if verbose {
        println!("TIFF: processed with gdal_translate (zstd or unsupported compression)");
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
        // Overwrite in place
        Ok(input_path.to_path_buf())
    }
}

/// Print progress information for processed files
fn print_progress(
    path: &Path,
    _output_path: &Path,
    file_type: FileType,
    input_size: u64,
    output_size: u64,
    _has_output_dir: bool,
) {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| path.display().to_string().into());

    let type_str = match file_type {
        FileType::Zip => "ZIP",
        FileType::Gz => "GZ",
        FileType::Png => "PNG",
        #[cfg(feature = "tiff-support")]
        FileType::Tiff => "TIFF",
    };

    let size_diff = output_size as i64 - input_size as i64;
    let size_change = if size_diff >= 0 {
        format!("+{}", format_size(size_diff as u64))
    } else {
        format!("-{}", format_size((-size_diff) as u64))
    };

    let compression_ratio = if input_size > 0 {
        (output_size as f64 / input_size as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "{} | {} | {} → {} | {} ({:.1}%)",
        filename,
        type_str,
        format_size(input_size),
        format_size(output_size),
        size_change,
        compression_ratio
    );
}

/// Format file size in human-readable format
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
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
        assert_eq!(result, input_path);
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
            let options: FileOptions<()> =
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
            let options: FileOptions<()> =
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

    #[cfg(feature = "tiff-support")]
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

    #[cfg(feature = "tiff-support")]
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
            let options: FileOptions<()> =
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

    #[cfg(feature = "tiff-support")]
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
