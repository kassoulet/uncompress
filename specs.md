# Uncompress - Specifications

## Overview

`uncompress` is a command-line tool that optimizes files for better Git storage by recompressing them with no or minimal compression. This is particularly useful for mutable files that change frequently, as uncompressed or minimally compressed files produce better deltas in Git.

## Purpose

- **Reduce Git repository size** by optimizing file compression for better delta storage
- **Support multiple file formats** including ZIP-based files, PNG, GZ, and TIFF/GeoTIFF
- **Magic byte detection** - File type detection based on content, not file extensions

## Supported File Types

### ZIP-Based Files
**Magic Bytes:** `PK\x03\x04` (50 4B 03 04)

**Supported Extensions:**
- `.zip`, `.docx`, `.xlsx`, `.pptx`
- `.xlsm`, `.pptm`, `.dotx`, `.dotm`
- `.xltm`, `.potx`, `.potm`, `.ipynb`

**Processing:**
- Decompresses and recompresses with STORED method (no compression)
- Preserves all file entries and directory structure

### PNG Images
**Magic Bytes:** `\x89PNG\r\n\x1a\n` (89 50 4E 47 0D 0A 1A 0A)

**Processing:**
- Applies Paeth filter (optimal for most images)
- Sets compression level to zero (no compression)
- Preserves original dimensions, color type, and bit depth

### GZ Files
**Magic Bytes:** `\x1f\x8b` (1F 8B)

**Processing:**
- Decompresses and recompresses with compression level 0
- Uses raw deflate for optimal Git delta storage

### TIFF/GeoTIFF Images
**Magic Bytes:**
- Little-endian: `II*\x00` (49 49 2A 00)
- Big-endian: `MM\x00*` (4D 4D 00 2A)

**Supported Formats:**
- 8-bit and 16-bit images
- Grayscale (1 sample per pixel)
- RGB (3 samples per pixel)
- RGBA (4 samples per pixel)

**Processing:**
- Recompresses with no compression (Uncompressed)
- Applies horizontal predictor filter
- Preserves original bit depth and color type
- **Full metadata preservation** - All TIFF tags are preserved including:
  - Standard TIFF tags (resolution, color space, etc.)
  - GeoTIFF geospatial tags:
    - ModelPixelScaleTag (33550)
    - ModelTiepointTag (33922)
    - GeoKeyDirectoryTag (34735)
    - GeoDoubleParamsTag (34736)
    - GeoAsciiParamsTag (34737)
  - EXIF tags
  - Custom/private tags

## File Type Detection

The tool uses **magic byte detection** instead of file extension filtering. This means:

- Files are identified by their actual content
- Files with incorrect extensions are still processed correctly
- Example: A ZIP file named `data.dat` will be detected and processed as ZIP

### Magic Byte Signatures

| File Type | Magic Bytes (Hex) | Magic Bytes (ASCII) |
|-----------|-------------------|---------------------|
| ZIP | `50 4B 03 04` | `PK\x03\x04` |
| PNG | `89 50 4E 47 0D 0A 1A 0A` | `\x89PNG\r\n\x1a\n` |
| GZ | `1F 8B` | - |
| TIFF (LE) | `49 49 2A 00` | `II*\x00` |
| TIFF (BE) | `4D 4D 00 2A` | `MM\x00*` |

## Command-Line Interface

### Usage

```bash
uncompress [OPTIONS] <PATHS>...
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<PATHS>...` | Files or directories to process (required, multiple allowed) |

### Options

| Option | Short | Long | Default | Description |
|--------|-------|------|---------|-------------|
| Output directory | `-o` | `--output <DIR>` | - | Output directory (default: overwrite in place) |
| Recursive | `-r` | `--recursive` | `true` | Process directories recursively |
| Verbose | `-v` | `--verbose` | `false` | Enable verbose output |
| Help | `-h` | `--help` | - | Print help information |
| Version | `-V` | `--version` | - | Print version information |

### Examples

```bash
# Process a single file
uncompress document.docx

# Process multiple files
uncompress image.png archive.zip data.tif

# Process a directory recursively
uncompress ./project/

# Process with verbose output
uncompress -v ./files/

# Process to a specific output directory
uncompress -o ./output/ ./input/

# Process without recursion (single level)
uncompress --recursive=false ./directory/
```

## Technical Details

### Output Behavior

- **Default (no `-o`)**: Overwrites files in place using temporary files
- **With `-o`**: Writes processed files to the specified output directory

### Compression Strategy

All file types are processed to minimize compression for optimal Git delta storage:

| File Type | Compression | Filter/Predictor |
|-----------|-------------|------------------|
| ZIP-based | STORED (none) | - |
| PNG | No compression | Paeth |
| GZ | Level 0 (none) | - |
| TIFF | Uncompressed | Horizontal |

### Error Handling

- Unsupported file types are silently skipped (with verbose message if `-v`)
- Processing errors are reported to stderr
- Non-existent files are skipped without error

## Testing

### Test Fixtures

Test files are located in `tests/fixtures/`:

- `test.zip`, `test.dat` - ZIP files (same content, different extensions)
- `test.png`, `test.bin` - PNG files (same content, different extensions)
- `test.txt.gz`, `test.data` - GZ files (same content, different extensions)
- `test.tiff`, `test.tif_data` - TIFF RGB files (same content, different extensions)
- `test_gray.tiff` - TIFF grayscale file

### Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test cli_tests

# Generate test fixtures
cargo run --example generate_fixtures
```

### Test Coverage

- **Unit tests**: File type detection, path determination, processing functions
- **Integration tests**: CLI behavior, file processing, magic byte detection
- **Test scenarios**: Correct extensions, wrong extensions, directories, unsupported types

## Build Requirements

### Rust Version

Minimum Rust version: **1.70**

### Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
flate2 = "1.0"
png = "0.18"
zip = "8.1.0"
walkdir = "2"
tiff = "0.10"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

## Performance Considerations

- **Memory**: Files are processed in memory; very large files may require significant RAM
- **Speed**: Decompression/recompression is CPU-intensive but typically fast for most files
- **Git benefits**: Optimized files produce smaller Git repositories over time

## Use Cases

### Ideal Scenarios

1. **Office documents** (`.docx`, `.xlsx`, `.pptx`) in version control
2. **Jupyter notebooks** (`.ipynb`) with large outputs
3. **PNG images** that change frequently
4. **TIFF/GeoTIFF** files in GIS projects
5. **Compressed archives** that need version tracking

### Not Recommended For

- Files that are already optimally compressed
- Very large binary files (consider Git LFS instead)
- Files that rarely change

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.
