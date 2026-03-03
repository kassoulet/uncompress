## 2025-01-24 - Initial Assessment
**Learning:** The application processes files sequentially and opens the same file multiple times (detection, skip check, processing). ZIP processing lacks output buffering.
**Action:** Focus on reducing redundant IO and adding buffering to writers.
