// Example: Process a single file
//
// This example demonstrates how to use uncompress to process a single file.
//
// Usage: cargo run --example process_file <path/to/file>

use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/file>", args[0]);
        eprintln!();
        eprintln!("This example processes a single file using uncompress.");
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);

    if !file_path.exists() {
        eprintln!("Error: File does not exist: {}", file_path.display());
        std::process::exit(1);
    }

    println!("Processing file: {}", file_path.display());

    // In a real application, you would call the processing functions here
    // For this example, we just show the file info
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");

    println!("File extension: {}", extension);
    println!(
        "File size: {} bytes",
        std::fs::metadata(&file_path).unwrap().len()
    );

    match extension.to_lowercase().as_str() {
        "png" => println!("Would process as PNG with Paeth filter"),
        "gz" => println!("Would process as GZIP with zero compression"),
        "docx" | "xlsx" | "pptx" | "ipynb" | "zip" => {
            println!("Would process as ZIP-based file with STORED method");
        }
        _ => println!("Unsupported file type for uncompress"),
    }
}
