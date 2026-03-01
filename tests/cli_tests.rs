//! Integration tests for uncompress CLI using real fixture files

use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

/// Get the path to the fixtures directory
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Test that the help command works
#[test]
fn test_help() {
    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

/// Test that version command works
#[test]
fn test_version() {
    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("uncompress"));
}

/// Test error on missing file
#[test]
fn test_missing_file() {
    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    // The program should handle missing files gracefully (skip them)
    // We just verify it doesn't crash
    cmd.arg("/nonexistent/file.zip").assert().success();
}

/// Test processing a ZIP file with correct extension
#[test]
fn test_process_zip_with_correct_extension() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = fixtures_dir().join("test.zip");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
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

/// Test processing a ZIP file with WRONG extension (tests magic byte detection)
#[test]
fn test_process_zip_with_wrong_extension() {
    let temp_dir = TempDir::new().unwrap();
    // test.dat has ZIP magic bytes but wrong extension
    let input_file = fixtures_dir().join("test.dat");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists (should be processed despite wrong extension)
    let output_file = output_dir.join("test.dat");
    assert!(output_file.exists());
}

/// Test processing a PNG file with correct extension
#[test]
fn test_process_png_with_correct_extension() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = fixtures_dir().join("test.png");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists
    let output_file = output_dir.join("test.png");
    assert!(output_file.exists());
}

/// Test processing a PNG file with WRONG extension (tests magic byte detection)
#[test]
fn test_process_png_with_wrong_extension() {
    let temp_dir = TempDir::new().unwrap();
    // test.bin has PNG magic bytes but wrong extension
    let input_file = fixtures_dir().join("test.bin");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists (should be processed despite wrong extension)
    let output_file = output_dir.join("test.bin");
    assert!(output_file.exists());
}

/// Test processing a GZ file with correct extension
#[test]
fn test_process_gz_with_correct_extension() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = fixtures_dir().join("test.txt.gz");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
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

/// Test processing a GZ file with WRONG extension (tests magic byte detection)
#[test]
fn test_process_gz_with_wrong_extension() {
    let temp_dir = TempDir::new().unwrap();
    // test.data has GZ magic bytes but wrong extension
    let input_file = fixtures_dir().join("test.data");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists (should be processed despite wrong extension)
    let output_file = output_dir.join("test.data");
    assert!(output_file.exists());
}

/// Test processing a directory with mixed files
#[test]
#[cfg(feature = "tiff-support")]
fn test_process_directory_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let fixtures = fixtures_dir();
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&fixtures)
        .assert()
        .success();

    // Verify output files exist for supported types
    assert!(output_dir.join("test.zip").exists());
    assert!(output_dir.join("test.dat").exists()); // ZIP with wrong ext
    assert!(output_dir.join("test.png").exists());
    assert!(output_dir.join("test.bin").exists()); // PNG with wrong ext
    assert!(output_dir.join("test.txt.gz").exists());
    assert!(output_dir.join("test.data").exists()); // GZ with wrong ext
    assert!(output_dir.join("test.tiff").exists());
    assert!(output_dir.join("test.tif_data").exists()); // TIFF with wrong ext
    assert!(output_dir.join("test_gray.tiff").exists());
}

/// Test processing a TIFF file with correct extension
#[test]
#[cfg(feature = "tiff-support")]
fn test_process_tiff_with_correct_extension() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = fixtures_dir().join("test.tiff");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists
    let output_file = output_dir.join("test.tiff");
    assert!(output_file.exists());
}

/// Test processing a TIFF file with WRONG extension (tests magic byte detection)
#[test]
#[cfg(feature = "tiff-support")]
fn test_process_tiff_with_wrong_extension() {
    let temp_dir = TempDir::new().unwrap();
    // test.tif_data has TIFF magic bytes but wrong extension
    let input_file = fixtures_dir().join("test.tif_data");
    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&input_file)
        .assert()
        .success();

    // Verify output file exists (should be processed despite wrong extension)
    let output_file = output_dir.join("test.tif_data");
    assert!(output_file.exists());
}

/// Test that unsupported file types are skipped
#[test]
fn test_unsupported_file_types_skipped() {
    let temp_dir = TempDir::new().unwrap();

    // Create a text file (unsupported)
    let txt_file = temp_dir.path().join("unsupported.txt");
    std::fs::write(&txt_file, "This is not a supported format").unwrap();

    let output_dir = temp_dir.path().join("output");

    let mut cmd = assert_cmd::Command::cargo_bin("uncompress").unwrap();
    cmd.arg("-o")
        .arg(&output_dir)
        .arg("-v")
        .arg(&txt_file)
        .assert()
        .success();

    // Output file should NOT exist for unsupported types
    let output_file = output_dir.join("unsupported.txt");
    assert!(!output_file.exists());
}
