## 2026-03-02 - Secure Atomic File Replacement
**Vulnerability:** Predictable temporary filenames and loss of file permissions during in-place updates.
**Learning:** The application used a fixed prefix (`.unc.`) for temporary files and did not preserve original file permissions. This exposed the application to symlink attacks (overwriting sensitive files via pre-created symlinks) and potentially made restricted files world-readable after processing.
**Prevention:** Always use secure temporary file creation libraries (like `tempfile` in Rust) that use random suffixes and ensure atomic replacement (`persist` / `rename`). Explicitly capture and reapply file permissions when replacing user-owned files.
