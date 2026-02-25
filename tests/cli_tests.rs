//! Integration tests for uncompress CLI

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Test that the help command works
#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("uncompress").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

/// Test that version command works
#[test]
fn test_version() {
    let mut cmd = Command::cargo_bin("uncompress").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("uncompress"));
}

/// Test error on missing file
#[test]
fn test_missing_file() {
    let mut cmd = Command::cargo_bin("uncompress").unwrap();
    cmd.arg("/nonexistent/file.zip")
        .assert()
        .stderr(predicate::str::contains("No such file"));
}

/// Test processing a ZIP file
#[test]
fn test_process_zip() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.zip");
    let output_dir = temp_dir.path().join("output");

    // Create a simple ZIP file
    create_test_zip(&input_file);

    let mut cmd = Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists
    let output_file = output_dir.join("test.zip");
    assert!(output_file.exists());
}

/// Test processing a GZ file
#[test]
fn test_process_gz() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.txt.gz");
    let output_dir = temp_dir.path().join("output");

    // Create a simple GZ file
    create_test_gz(&input_file);

    let mut cmd = Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists
    let output_file = output_dir.join("test.txt.gz");
    assert!(output_file.exists());
}

/// Helper function to create a test ZIP file
fn create_test_zip(path: &std::path::Path) {
    use std::fs::File;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    let file = File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("test.txt", options).unwrap();
    zip.write_all(b"Hello, World!").unwrap();

    zip.finish().unwrap();
}

/// Helper function to create a test GZ file
fn create_test_gz(path: &std::path::Path) {
    use std::fs::File;
    use std::io::Write;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let file = File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(b"Hello, GZIP!").unwrap();
    encoder.finish().unwrap();
}
