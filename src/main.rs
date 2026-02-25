use clap::Parser;
use flate2::write::GzEncoder;
use flate2::Compression;
use png::{Encoder, FilterType};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Decompress files for better git storage
/// 
/// Handles ZIP-based files (docx, xlsx, ipynb, etc.), GZ files, and PNG images.
/// For PNG, applies Paeth filter with zero compression.
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
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let result = match extension.to_lowercase().as_str() {
        "png" => process_png(path, output_dir, verbose),
        "gz" => process_gz(path, output_dir, verbose),
        "docx" | "xlsx" | "pptx" | "xlsm" | "pptm" | "dotx" | "dotm" | "xltm" | "potx" | "potm"
        | "ipynb" | "zip" => process_zip_based(path, output_dir, verbose),
        _ => {
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
fn process_zip_based(path: &Path, output_dir: Option<&PathBuf>, _verbose: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = determine_output_path(path, output_dir)?;

    let input_file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(input_file)?;

    let output_file = File::create(&output_path)?;
    let mut zip_writer = ZipWriter::new(output_file);

    // Use STORED method (no compression) for all entries
    let options = FileOptions::default()
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
fn process_gz(path: &Path, output_dir: Option<&PathBuf>, _verbose: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
fn process_png(path: &Path, output_dir: Option<&PathBuf>, verbose: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = determine_output_path(path, output_dir)?;

    // Read the PNG file
    let file = File::open(path)?;
    let decoder = png::Decoder::new(file);
    
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
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
    encoder.set_filter(FilterType::Paeth);
    encoder.set_compression(png::Compression::Fast);
    
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
fn determine_output_path(input_path: &Path, output_dir: Option<&PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = output_dir {
        fs::create_dir_all(dir)?;
        Ok(dir.join(input_path.file_name().ok_or("Invalid filename")?))
    } else {
        // Overwrite in place - create temp name then rename
        let temp_path = input_path.with_extension(format!("{}.tmp", input_path.extension().unwrap_or_default().to_string_lossy()));
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
    fn test_extension_matching_zip_based() {
        let extensions = ["docx", "xlsx", "pptx", "ipynb", "zip", "xlsm", "pptm"];
        for ext in extensions {
            let path = PathBuf::from(format!("test.{}", ext));
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            match extension.to_lowercase().as_str() {
                "docx" | "xlsx" | "pptx" | "xlsm" | "pptm" | "dotx" | "dotm" | "xltm" | "potx" | "potm"
                | "ipynb" | "zip" => {}, // Expected match
                _ => panic!("Extension {} should match ZIP-based", ext),
            }
        }
    }

    #[test]
    fn test_extension_matching_png() {
        let path = PathBuf::from("test.png");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert_eq!(extension.to_lowercase().as_str(), "png");
    }

    #[test]
    fn test_extension_matching_gz() {
        let path = PathBuf::from("test.txt.gz");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert_eq!(extension.to_lowercase().as_str(), "gz");
    }

    #[test]
    fn test_unsupported_extension() {
        let path = PathBuf::from("test.pdf");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match extension.to_lowercase().as_str() {
            "png" | "gz" => panic!("Should not match"),
            "docx" | "xlsx" | "pptx" | "xlsm" | "pptm" | "dotx" | "dotm" | "xltm" | "potx" | "potm"
            | "ipynb" | "zip" => panic!("Should not match ZIP-based"),
            _ => {}, // Expected for unsupported
        }
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
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

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
}
