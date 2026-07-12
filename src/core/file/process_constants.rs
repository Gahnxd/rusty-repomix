//! Constants used by binary filtering and charset normalization.

/// Common binary-file extensions that can be rejected before opening a file.
pub(super) const BINARY_EXTENSIONS: &[&str] = &[
    "7z", "a", "apk", "appimage", "avi", "bin", "bmp", "bz2", "cab", "class", "dll", "dmg", "doc",
    "docx", "eot", "exe", "flac", "gif", "gz", "ico", "iso", "jar", "jpeg", "jpg", "lib", "mp3",
    "mp4", "msi", "o", "odt", "ogg", "otf", "pdf", "png", "psd", "rar", "so", "tar", "tiff", "ttf",
    "wasm", "wav", "webm", "webp", "woff", "woff2", "xls", "xlsx", "zip",
];
