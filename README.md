# uncompress

<!-- [![Crates.io](https://img.shields.io/crates/v/uncompress.svg)](https://crates.io/crates/uncompress) -->
<!-- [![Documentation](https://docs.rs/uncompress/badge.svg)](https://docs.rs/uncompress) -->
<!-- [![License](https://img.shields.io/crates/l/uncompress)](LICENSE) -->
[![Build Status](https://github.com/kassoulet/uncompress/workflows/CI/badge.svg)](https://github.com/kassoulet/uncompress/actions)
[![Rust Version](https://img.shields.io/badge/rustc-1.70+-blue.svg)](https://rust-lang.org)

A command-line utility to decompress files for better Git storage. Reduces file size in Git repositories by recompressing files with zero or minimal compression.

## Overview

When storing binary files in Git repositories, compression will actually increase the repository size due to Git's delta compression working better with uncompressed data. `uncompress` helps by:

- **ZIP-based files** (`.docx`, `.xlsx`, `.pptx`, `.ipynb`, etc.): Recompresses with STORED method (no compression)
- **GZIP files** (`.gz`): Recompresses with zero compression level
- **PNG images**: Applies Paeth filter with no compression
- **TIFF/GeoTIFF images**: Uncompressed with horizontal predictor, full metadata preservation

> Use `uncompress` if your compressed files may change over time (screenshots, tests data, etc.) and you want to keep your Git repository size small.

> If the uncompressed files are part of a release, consider using an optimizer during your build process to minimize their sizes.

## Features

- **Progress output**: Shows filename, type, input/output sizes, and compression ratio
- **Skip uncompressed**: Automatically detects and skips already optimized files
- **Magic byte detection**: Identifies file types by content, not extension
- **BigTIFF support**: Handles both standard TIFF and BigTIFF formats
- **Full TIFF format support**: U8, U16, F32, F64, signed/unsigned integers
- **GeoTIFF metadata preservation**: Maintains all geospatial tags and metadata
- **Optional TIFF support**: Build without TIFF/GeoTIFF support for smaller binary

## Installation

### From Source

```bash
git clone https://github.com/kassoulet/uncompress.git
cd uncompress
cargo build --release
```

The binary will be available at `target/release/uncompress`.

### From Crates.io (when published)

```bash
cargo install uncompress
```

### Build Without TIFF Support

To build a smaller binary without TIFF/GeoTIFF support (requires no external dependencies):

```bash
cargo build --release --no-default-features
```

### GDAL Dependency

For TIFF/GeoTIFF files with advanced compression (ZSTD, WebP, JPEG, LZW, Deflate) or float formats (F32, F64), `uncompress` uses `gdal_translate` as a fallback. Install GDAL tools:

```bash
# Ubuntu/Debian
sudo apt-get install gdal-bin

# macOS
brew install gdal

# Windows
# Download from https://gdal.org/download.html
```

## Usage

```bash
# Process single files
uncompress file.docx file.ipynb image.png

# Process files to a specific output directory
uncompress -o output_dir/ file1.docx file2.xlsx

# Process all files in a directory recursively
uncompress -r my_documents/

# Verbose output
uncompress -v file.gz

# Combine options
uncompress -v -r -o processed/ documents/
```

### Command Line Options

```
Arguments:
  <PATHS>...  Files or directories to process

Options:
  -o, --output <OUTPUT>  Output directory (default: overwrite in place)
  -r, --recursive        Process files recursively in directories
  -v, --verbose          Verbose output
  -h, --help             Print help
  -V, --version          Print version
```

### Output Format

```
filename | TYPE | input_size → output_size | change (ratio%)
```

Examples:
```
document.docx | ZIP | 2.45 MB → 1.23 MB | -1.22 MB (50.2%)
image.png | PNG | 8.24 MB → 8.24 MB | +0 B (100.0%)
data.tif | TIFF | 1.47 GB → 4.69 GB | +3.21 GB (318.0%)
already_optimized.zip | ZIP | Already uncompressed | Skipped
unknown.dat | UNSUPPORTED | Skipped
```

## Supported File Types

| Extension | Description | Processing |
|-----------|-------------|------------|
| `.docx` | Microsoft Word | STORED (no compression) |
| `.xlsx` | Microsoft Excel | STORED (no compression) |
| `.pptx` | Microsoft PowerPoint | STORED (no compression) |
| `.ipynb` | Jupyter Notebook | STORED (no compression) |
| `.xlsm`, `.pptm`, `.dotx`, `.dotm`, `.xltm`, `.potx`, `.potm` | Office variants | STORED (no compression) |
| `.zip` | ZIP archive | STORED (no compression) |
| `.gz` | GZIP compressed | Zero compression level |
| `.png` | PNG image | Paeth filter, no compression |
| `.tiff`, `.tif` | TIFF/GeoTIFF image | Uncompressed + horizontal predictor |

### TIFF/GeoTIFF Format Support

**Bit Depths:**
- **Native** (tiff crate): U8, U16
- **Via GDAL**: F32, F64, F16, U32, U64, I8, I16, I32, I64

**Compression Types:**
- **Native**: Uncompressed
- **Via GDAL**: ZSTD, WebP, JPEG, LZW, Deflate

**Features:**
- BigTIFF support (files > 4GB)
- Full metadata preservation (GeoTIFF tags, georeferencing)
- Horizontal predictor for better compression

## Use Cases

### Git Repository Optimization

Files processed by `uncompress` often result in smaller Git repository sizes because:

1. Git's delta compression works better with uncompressed data
2. Binary diffs are more efficient when the base format isn't compressed
3. Changes between versions are more detectable

### Example Workflow

```bash
# Before committing large Office documents or Jupyter notebooks
uncompress -r ./documents/

# Git will now track the decompressed versions
git add documents/
git commit -m "Add documents (uncompressed for better git storage)"
```

## Building from Source

### Requirements

- Rust 1.70 or later
- Cargo
- GDAL tools (optional, for advanced TIFF/GeoTIFF support)

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Build without TIFF support (smaller binary)
cargo build --release --no-default-features

# Run tests
cargo test

# Run clippy linter
cargo clippy -- -W clippy::all

# Format code
cargo fmt
```

### Features

| Feature | Default | Description |
|---------|---------|-------------|
| `tiff-support` | Yes | Enable TIFF/GeoTIFF support (requires gdal crate) |

## License

This project is licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.
