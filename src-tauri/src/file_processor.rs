// SPDX-License-Identifier: Apache-2.0
//! Universal File Processor -- format detection, preview, conversion, and routing.
//!
//! ImpForge recognizes every common file format, displays it, can edit it, and
//! converts between formats.  Drag-and-drop any file and ImpForge opens it in
//! the right module.
//!
//! ## Capabilities
//!
//! - **Detect**: magic-byte + extension recognition for 100+ formats
//! - **Preview**: text extraction for text-based files; metadata for binary
//! - **Convert**: .docx -> .md/.txt/.html, .xlsx -> .csv/.json, .md -> .html, etc.
//! - **Route**: auto-navigate to ForgeWriter, ForgeSheets, ForgePDF, CodeForge IDE
//! - **AI Digest**: Ollama-powered document summarization
//!
//! .docx and .pptx are ZIP archives -- parsed with the `zip` crate (MIT).

use std::collections::HashMap;
use std::io::Read as IoRead;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum bytes to read for text preview.
const TEXT_PREVIEW_LIMIT: usize = 5000;

/// Maximum recent files to track.
const RECENT_FILES_LIMIT: usize = 50;

/// Ollama timeout for AI digest (seconds).
const AI_DIGEST_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// File categories
// ---------------------------------------------------------------------------

/// Top-level file category (determines routing and conversion options).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCategory {
    Document,
    Spreadsheet,
    Presentation,
    Pdf,
    Image,
    Video,
    Audio,
    Archive,
    Code,
    Data,
    Email,
    Font,
    ThreeD,
    Unknown,
}

impl std::fmt::Display for FileCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Document => "Document",
            Self::Spreadsheet => "Spreadsheet",
            Self::Presentation => "Presentation",
            Self::Pdf => "PDF",
            Self::Image => "Image",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Archive => "Archive",
            Self::Code => "Code",
            Self::Data => "Data",
            Self::Email => "Email",
            Self::Font => "Font",
            Self::ThreeD => "3D Model",
            Self::Unknown => "Unknown",
        };
        write!(f, "{name}")
    }
}

// ---------------------------------------------------------------------------
// File info (returned to frontend)
// ---------------------------------------------------------------------------

/// Full metadata about a detected file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub category: FileCategory,
    pub size_bytes: u64,
    pub mime_type: String,
    pub can_preview: bool,
    pub can_edit: bool,
    pub can_convert_to: Vec<String>,
    pub recommended_module: String,
    pub metadata: serde_json::Value,
}

/// Result of a file conversion operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub success: bool,
    pub output_path: String,
    pub source_format: String,
    pub target_format: String,
    pub message: String,
}

/// Description of a supported format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub extension: String,
    pub category: FileCategory,
    pub mime_type: String,
    pub can_preview: bool,
    pub can_edit: bool,
    pub can_convert_to: Vec<String>,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Extension → category / MIME / capabilities lookup
// ---------------------------------------------------------------------------

struct ExtMeta {
    category: FileCategory,
    mime: &'static str,
    can_preview: bool,
    can_edit: bool,
    convert_targets: &'static [&'static str],
    module: &'static str,
    description: &'static str,
}

fn extension_db() -> HashMap<&'static str, ExtMeta> {
    let mut m = HashMap::new();

    // ---- Documents ----
    for (ext, mime, desc, targets) in [
        ("docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "Microsoft Word (OOXML)", &["md", "txt", "html"] as &[&str]),
        ("doc",  "application/msword", "Microsoft Word (legacy)", &[]),
        ("odt",  "application/vnd.oasis.opendocument.text", "OpenDocument Text", &[]),
        ("rtf",  "application/rtf", "Rich Text Format", &[]),
        ("txt",  "text/plain", "Plain Text", &["md", "html"]),
        ("md",   "text/markdown", "Markdown", &["html", "txt"]),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Document,
            mime,
            can_preview: true,
            can_edit: matches!(ext, "txt" | "md"),
            convert_targets: targets,
            module: "writer",
            description: desc,
        });
    }

    // ---- Spreadsheets ----
    for (ext, mime, desc, targets) in [
        ("xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "Microsoft Excel (OOXML)", &["csv", "json", "tsv"] as &[&str]),
        ("xls",  "application/vnd.ms-excel", "Microsoft Excel (legacy)", &["csv", "json", "tsv"]),
        ("csv",  "text/csv", "Comma-Separated Values", &["xlsx", "json", "tsv"]),
        ("tsv",  "text/tab-separated-values", "Tab-Separated Values", &["csv", "json", "xlsx"]),
        ("ods",  "application/vnd.oasis.opendocument.spreadsheet", "OpenDocument Spreadsheet", &["csv", "json"]),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Spreadsheet,
            mime,
            can_preview: true,
            can_edit: matches!(ext, "csv" | "tsv"),
            convert_targets: targets,
            module: "sheets",
            description: desc,
        });
    }

    // ---- Presentations ----
    for (ext, mime, desc, targets) in [
        ("pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation", "Microsoft PowerPoint (OOXML)", &["txt"] as &[&str]),
        ("ppt",  "application/vnd.ms-powerpoint", "Microsoft PowerPoint (legacy)", &[]),
        ("odp",  "application/vnd.oasis.opendocument.presentation", "OpenDocument Presentation", &[]),
        ("key",  "application/x-iwork-keynote-sftkey", "Apple Keynote", &[]),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Presentation,
            mime,
            can_preview: matches!(ext, "pptx"),
            can_edit: false,
            convert_targets: targets,
            module: "files",
            description: desc,
        });
    }

    // ---- PDF ----
    m.insert("pdf", ExtMeta {
        category: FileCategory::Pdf,
        mime: "application/pdf",
        can_preview: true,
        can_edit: false,
        convert_targets: &["txt", "md"],
        module: "pdf",
        description: "Portable Document Format",
    });

    // ---- Images ----
    for (ext, mime, desc) in [
        ("png",  "image/png", "PNG Image"),
        ("jpg",  "image/jpeg", "JPEG Image"),
        ("jpeg", "image/jpeg", "JPEG Image"),
        ("gif",  "image/gif", "GIF Image"),
        ("svg",  "image/svg+xml", "SVG Vector"),
        ("webp", "image/webp", "WebP Image"),
        ("bmp",  "image/bmp", "Bitmap Image"),
        ("tiff", "image/tiff", "TIFF Image"),
        ("tif",  "image/tiff", "TIFF Image"),
        ("ico",  "image/x-icon", "Icon"),
        ("psd",  "image/vnd.adobe.photoshop", "Photoshop Document"),
        ("ai",   "application/postscript", "Adobe Illustrator"),
        ("eps",  "application/postscript", "Encapsulated PostScript"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Image,
            mime,
            can_preview: matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "ico"),
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- Video ----
    for (ext, mime, desc) in [
        ("mp4",  "video/mp4", "MP4 Video"),
        ("avi",  "video/x-msvideo", "AVI Video"),
        ("mkv",  "video/x-matroska", "Matroska Video"),
        ("mov",  "video/quicktime", "QuickTime Video"),
        ("webm", "video/webm", "WebM Video"),
        ("flv",  "video/x-flv", "Flash Video"),
        ("wmv",  "video/x-ms-wmv", "Windows Media Video"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Video,
            mime,
            can_preview: false,
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- Audio ----
    for (ext, mime, desc) in [
        ("mp3",  "audio/mpeg", "MP3 Audio"),
        ("wav",  "audio/wav", "WAV Audio"),
        ("flac", "audio/flac", "FLAC Audio"),
        ("ogg",  "audio/ogg", "Ogg Vorbis"),
        ("aac",  "audio/aac", "AAC Audio"),
        ("wma",  "audio/x-ms-wma", "Windows Media Audio"),
        ("m4a",  "audio/mp4", "M4A Audio"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Audio,
            mime,
            can_preview: false,
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- Archives ----
    for (ext, mime, desc) in [
        ("zip",  "application/zip", "ZIP Archive"),
        ("tar",  "application/x-tar", "TAR Archive"),
        ("gz",   "application/gzip", "GZip Archive"),
        ("bz2",  "application/x-bzip2", "BZip2 Archive"),
        ("xz",   "application/x-xz", "XZ Archive"),
        ("7z",   "application/x-7z-compressed", "7-Zip Archive"),
        ("rar",  "application/vnd.rar", "RAR Archive"),
        ("zst",  "application/zstd", "Zstandard Archive"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Archive,
            mime,
            can_preview: false,
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- Code ----
    for (ext, mime, desc) in [
        ("rs",    "text/x-rust", "Rust Source"),
        ("py",    "text/x-python", "Python Source"),
        ("ts",    "text/typescript", "TypeScript Source"),
        ("tsx",   "text/typescript", "TypeScript JSX"),
        ("js",    "text/javascript", "JavaScript Source"),
        ("jsx",   "text/javascript", "JavaScript JSX"),
        ("html",  "text/html", "HTML Document"),
        ("css",   "text/css", "CSS Stylesheet"),
        ("scss",  "text/x-scss", "SCSS Stylesheet"),
        ("less",  "text/x-less", "LESS Stylesheet"),
        ("java",  "text/x-java", "Java Source"),
        ("c",     "text/x-c", "C Source"),
        ("cpp",   "text/x-c++", "C++ Source"),
        ("h",     "text/x-c", "C/C++ Header"),
        ("hpp",   "text/x-c++", "C++ Header"),
        ("cs",    "text/x-csharp", "C# Source"),
        ("go",    "text/x-go", "Go Source"),
        ("rb",    "text/x-ruby", "Ruby Source"),
        ("php",   "text/x-php", "PHP Source"),
        ("swift", "text/x-swift", "Swift Source"),
        ("kt",    "text/x-kotlin", "Kotlin Source"),
        ("scala", "text/x-scala", "Scala Source"),
        ("lua",   "text/x-lua", "Lua Source"),
        ("r",     "text/x-r", "R Source"),
        ("jl",    "text/x-julia", "Julia Source"),
        ("ex",    "text/x-elixir", "Elixir Source"),
        ("exs",   "text/x-elixir", "Elixir Script"),
        ("sh",    "text/x-shellscript", "Shell Script"),
        ("bash",  "text/x-shellscript", "Bash Script"),
        ("zsh",   "text/x-shellscript", "Zsh Script"),
        ("fish",  "text/x-shellscript", "Fish Script"),
        ("ps1",   "text/x-powershell", "PowerShell Script"),
        ("sql",   "text/x-sql", "SQL"),
        ("svelte","text/x-svelte", "Svelte Component"),
        ("vue",   "text/x-vue", "Vue Component"),
        ("dart",  "text/x-dart", "Dart Source"),
        ("zig",   "text/x-zig", "Zig Source"),
        ("v",     "text/x-v", "V Source"),
        ("nim",   "text/x-nim", "Nim Source"),
        ("ml",    "text/x-ocaml", "OCaml Source"),
        ("hs",    "text/x-haskell", "Haskell Source"),
        ("erl",   "text/x-erlang", "Erlang Source"),
        ("clj",   "text/x-clojure", "Clojure Source"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Code,
            mime,
            can_preview: true,
            can_edit: true,
            convert_targets: &[],
            module: "ide",
            description: desc,
        });
    }

    // ---- Data ----
    for (ext, mime, desc, targets) in [
        ("json", "application/json", "JSON Data", &["yaml", "csv"] as &[&str]),
        ("yaml", "application/x-yaml", "YAML Data", &["json"]),
        ("yml",  "application/x-yaml", "YAML Data", &["json"]),
        ("toml", "application/toml", "TOML Config", &["json"]),
        ("xml",  "application/xml", "XML Document", &[]),
        ("ini",  "text/plain", "INI Config", &[]),
        ("env",  "text/plain", "Environment Variables", &[]),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Data,
            mime,
            can_preview: true,
            can_edit: true,
            convert_targets: targets,
            module: "ide",
            description: desc,
        });
    }

    // ---- Email ----
    for (ext, mime, desc) in [
        ("eml", "message/rfc822", "Email Message"),
        ("msg", "application/vnd.ms-outlook", "Outlook Message"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Email,
            mime,
            can_preview: matches!(ext, "eml"),
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- Fonts ----
    for (ext, mime, desc) in [
        ("ttf",   "font/ttf", "TrueType Font"),
        ("otf",   "font/otf", "OpenType Font"),
        ("woff",  "font/woff", "Web Open Font Format"),
        ("woff2", "font/woff2", "WOFF2 Font"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::Font,
            mime,
            can_preview: false,
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    // ---- 3D Models ----
    for (ext, mime, desc) in [
        ("obj",   "model/obj", "Wavefront OBJ"),
        ("fbx",   "application/octet-stream", "Autodesk FBX"),
        ("gltf",  "model/gltf+json", "glTF (JSON)"),
        ("glb",   "model/gltf-binary", "glTF Binary"),
        ("stl",   "model/stl", "Stereolithography"),
        ("blend", "application/x-blender", "Blender File"),
        ("dae",   "model/vnd.collada+xml", "COLLADA"),
        ("3ds",   "application/x-3ds", "3D Studio"),
        ("usdz",  "model/vnd.usdz+zip", "USD Zipped"),
    ] {
        m.insert(ext, ExtMeta {
            category: FileCategory::ThreeD,
            mime,
            can_preview: false,
            can_edit: false,
            convert_targets: &[],
            module: "files",
            description: desc,
        });
    }

    m
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Normalize extension: lowercase, no leading dot.
fn normalize_ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Read magic bytes to refine detection when extension is missing/ambiguous.
fn detect_by_magic(path: &Path) -> Option<&'static str> {
    let mut buf = [0u8; 8];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut buf).ok()?;
    if n < 4 {
        return None;
    }

    // PDF
    if buf.starts_with(b"%PDF") {
        return Some("pdf");
    }
    // PNG
    if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("png");
    }
    // JPEG
    if buf.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    // GIF
    if buf.starts_with(b"GIF8") {
        return Some("gif");
    }
    // ZIP (also .docx, .pptx, .xlsx, .odt, .ods, .odp)
    if buf.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        // Peek inside the ZIP to distinguish office formats
        return detect_office_zip(path);
    }
    // GZIP
    if buf.starts_with(&[0x1F, 0x8B]) {
        return Some("gz");
    }
    // 7-Zip
    if n >= 6 && buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return Some("7z");
    }
    // RIFF (WAV, AVI, WebP)
    if buf.starts_with(b"RIFF") && n >= 8 {
        let mut sub = [0u8; 4];
        // Read bytes 8-12 to distinguish
        let mut f2 = std::fs::File::open(path).ok()?;
        let mut skip = [0u8; 12];
        f2.read(&mut skip).ok()?;
        sub.copy_from_slice(&skip[8..12]);
        if &sub == b"WAVE" {
            return Some("wav");
        }
        if &sub == b"AVI " {
            return Some("avi");
        }
        if &sub == b"WEBP" {
            return Some("webp");
        }
    }
    // OGG
    if buf.starts_with(b"OggS") {
        return Some("ogg");
    }
    // FLAC
    if buf.starts_with(b"fLaC") {
        return Some("flac");
    }
    // MP3 (ID3 tag or sync word)
    if buf.starts_with(b"ID3") || (buf[0] == 0xFF && (buf[1] & 0xE0) == 0xE0) {
        return Some("mp3");
    }
    // BMP
    if buf.starts_with(b"BM") {
        return Some("bmp");
    }

    None
}

/// For ZIP-based files, peek inside to determine if it is docx/xlsx/pptx/odt/ods/odp.
fn detect_office_zip(path: &Path) -> Option<&'static str> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    // Check for OOXML content types
    for i in 0..archive.len().min(30) {
        let entry = archive.by_index(i).ok()?;
        let name = entry.name().to_string();
        if name.starts_with("word/") {
            return Some("docx");
        }
        if name.starts_with("xl/") {
            return Some("xlsx");
        }
        if name.starts_with("ppt/") {
            return Some("pptx");
        }
    }

    // Check for ODF mimetype entry
    if let Ok(mut mt) = archive.by_name("mimetype") {
        let mut buf = String::new();
        if mt.read_to_string(&mut buf).is_ok() {
            let trimmed = buf.trim();
            if trimmed.contains("opendocument.text") {
                return Some("odt");
            }
            if trimmed.contains("opendocument.spreadsheet") {
                return Some("ods");
            }
            if trimmed.contains("opendocument.presentation") {
                return Some("odp");
            }
        }
    }

    Some("zip")
}

/// Build a `FileInfo` struct from a path.
fn build_file_info(path: &Path) -> AppResult<FileInfo> {
    let db = extension_db();
    let meta = std::fs::metadata(path).map_err(ImpForgeError::from)?;

    let mut ext = normalize_ext(path);

    // If extension is empty or unknown, try magic bytes
    if ext.is_empty() || !db.contains_key(ext.as_str()) {
        if let Some(detected) = detect_by_magic(path) {
            ext = detected.to_string();
        }
    }

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let entry = db.get(ext.as_str());

    let category = entry.map(|e| e.category.clone()).unwrap_or(FileCategory::Unknown);
    let mime_type = entry.map(|e| e.mime.to_string()).unwrap_or_else(|| "application/octet-stream".to_string());
    let can_preview = entry.map(|e| e.can_preview).unwrap_or(false);
    let can_edit = entry.map(|e| e.can_edit).unwrap_or(false);
    let can_convert_to = entry
        .map(|e| e.convert_targets.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();
    let recommended_module = entry.map(|e| e.module.to_string()).unwrap_or_else(|| "files".to_string());

    let mut metadata_map = serde_json::Map::new();
    metadata_map.insert("modified".into(), serde_json::json!(
        meta.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
        }).unwrap_or(0)
    ));
    metadata_map.insert("readonly".into(), serde_json::json!(meta.permissions().readonly()));

    Ok(FileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        extension: ext,
        category,
        size_bytes: meta.len(),
        mime_type,
        can_preview,
        can_edit,
        can_convert_to,
        recommended_module,
        metadata: serde_json::Value::Object(metadata_map),
    })
}

/// Read first N bytes of a text file as a preview string.
fn text_preview(path: &Path, limit: usize) -> AppResult<String> {
    let mut file = std::fs::File::open(path).map_err(ImpForgeError::from)?;
    let mut buf = vec![0u8; limit];
    let n = file.read(&mut buf).map_err(ImpForgeError::from)?;
    buf.truncate(n);
    // Lossy UTF-8 conversion (handles binary-ish files gracefully)
    Ok(String::from_utf8_lossy(&buf).to_string())
}

// ---------------------------------------------------------------------------
// DOCX / PPTX extraction
// ---------------------------------------------------------------------------

/// Strip XML tags from a string, keeping text content and inserting newlines
/// at paragraph boundaries.
fn strip_xml_tags(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    let mut last_was_para_end = false;

    // Track the current tag name for paragraph detection
    let mut tag_buf = String::new();

    for ch in xml.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if ch == '>' {
            in_tag = false;
            // Detect paragraph/line break tags
            let tag_lower = tag_buf.to_ascii_lowercase();
            if tag_lower.starts_with("/w:p")
                || tag_lower.starts_with("/a:p")
                || tag_lower == "w:br"
                || tag_lower == "w:br/"
            {
                if !last_was_para_end {
                    result.push('\n');
                    last_was_para_end = true;
                }
            }
            tag_buf.clear();
            continue;
        }
        if in_tag {
            tag_buf.push(ch);
        } else {
            result.push(ch);
            last_was_para_end = false;
        }
    }

    // Collapse excessive whitespace within lines
    let lines: Vec<String> = result
        .lines()
        .map(|l| {
            let trimmed = l.split_whitespace().collect::<Vec<_>>().join(" ");
            trimmed
        })
        .collect();

    // Remove consecutive empty lines
    let mut cleaned = String::new();
    let mut prev_empty = false;
    for line in &lines {
        if line.is_empty() {
            if !prev_empty {
                cleaned.push('\n');
            }
            prev_empty = true;
        } else {
            cleaned.push_str(line);
            cleaned.push('\n');
            prev_empty = false;
        }
    }

    cleaned.trim().to_string()
}

/// Extract text content from a .docx file (ZIP → word/document.xml → strip tags).
fn extract_docx_text(path: &Path) -> AppResult<String> {
    let file = std::fs::File::open(path).map_err(ImpForgeError::from)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        ImpForgeError::validation("INVALID_DOCX", format!("Not a valid DOCX file: {e}"))
    })?;

    let mut doc_xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| {
            ImpForgeError::validation(
                "MISSING_DOCUMENT_XML",
                "DOCX file does not contain word/document.xml",
            )
        })?
        .read_to_string(&mut doc_xml)
        .map_err(ImpForgeError::from)?;

    Ok(strip_xml_tags(&doc_xml))
}

/// Extract text from all slides in a .pptx file.
fn extract_pptx_text(path: &Path) -> AppResult<String> {
    let file = std::fs::File::open(path).map_err(ImpForgeError::from)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        ImpForgeError::validation("INVALID_PPTX", format!("Not a valid PPTX file: {e}"))
    })?;

    // Collect slide file names (ppt/slides/slide1.xml, slide2.xml, ...)
    let slide_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // Sort by slide number
    let mut sorted = slide_names;
    sorted.sort_by(|a, b| {
        let num_a = extract_slide_number(a);
        let num_b = extract_slide_number(b);
        num_a.cmp(&num_b)
    });

    let mut all_text = String::new();
    for (idx, slide_name) in sorted.iter().enumerate() {
        // Re-open archive for each slide (ZipArchive borrow rules)
        let file2 = std::fs::File::open(path).map_err(ImpForgeError::from)?;
        let mut archive2 = zip::ZipArchive::new(file2).map_err(|e| {
            ImpForgeError::internal("ZIP_ERROR", format!("Failed to re-open ZIP: {e}"))
        })?;

        let mut xml = String::new();
        let read_ok = {
            if let Ok(mut entry) = archive2.by_name(slide_name) {
                entry.read_to_string(&mut xml).is_ok()
            } else {
                false
            }
        };
        if read_ok {
            let text = strip_xml_tags(&xml);
            if !text.is_empty() {
                all_text.push_str(&format!("--- Slide {} ---\n{}\n\n", idx + 1, text));
            }
        }
    }

    if all_text.is_empty() {
        all_text = "(No text content found in slides)".to_string();
    }

    Ok(all_text.trim().to_string())
}

fn extract_slide_number(name: &str) -> u32 {
    // "ppt/slides/slide12.xml" → 12
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse::<u32>()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Conversion implementations
// ---------------------------------------------------------------------------

/// Simple Markdown → HTML conversion (headings, bold, italic, code, links, lists).
fn md_to_html(md: &str) -> String {
    let mut html = String::from("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"></head><body>\n");

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            html.push_str("<br>\n");
            continue;
        }

        // Headings
        if let Some(rest) = trimmed.strip_prefix("######") {
            html.push_str(&format!("<h6>{}</h6>\n", inline_md(rest.trim())));
        } else if let Some(rest) = trimmed.strip_prefix("#####") {
            html.push_str(&format!("<h5>{}</h5>\n", inline_md(rest.trim())));
        } else if let Some(rest) = trimmed.strip_prefix("####") {
            html.push_str(&format!("<h4>{}</h4>\n", inline_md(rest.trim())));
        } else if let Some(rest) = trimmed.strip_prefix("###") {
            html.push_str(&format!("<h3>{}</h3>\n", inline_md(rest.trim())));
        } else if let Some(rest) = trimmed.strip_prefix("##") {
            html.push_str(&format!("<h2>{}</h2>\n", inline_md(rest.trim())));
        } else if let Some(rest) = trimmed.strip_prefix('#') {
            html.push_str(&format!("<h1>{}</h1>\n", inline_md(rest.trim())));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            html.push_str(&format!("<li>{}</li>\n", inline_md(&trimmed[2..])));
        } else if trimmed.starts_with("> ") {
            html.push_str(&format!("<blockquote>{}</blockquote>\n", inline_md(&trimmed[2..])));
        } else if trimmed.starts_with("---") || trimmed.starts_with("***") {
            html.push_str("<hr>\n");
        } else {
            html.push_str(&format!("<p>{}</p>\n", inline_md(trimmed)));
        }
    }

    html.push_str("</body></html>");
    html
}

/// Process inline markdown: **bold**, *italic*, `code`, [links](url).
fn inline_md(text: &str) -> String {
    let mut result = text.to_string();

    // Bold: **text** or __text__
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let inner = &result[start + 2..start + 2 + end].to_string();
            result = format!(
                "{}<strong>{}</strong>{}",
                &result[..start],
                inner,
                &result[start + 2 + end + 2..]
            );
        } else {
            break;
        }
    }

    // Italic: *text*
    while let Some(start) = result.find('*') {
        if let Some(end) = result[start + 1..].find('*') {
            let inner = &result[start + 1..start + 1 + end].to_string();
            result = format!(
                "{}<em>{}</em>{}",
                &result[..start],
                inner,
                &result[start + 1 + end + 1..]
            );
        } else {
            break;
        }
    }

    // Inline code: `text`
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let inner = &result[start + 1..start + 1 + end].to_string();
            result = format!(
                "{}<code>{}</code>{}",
                &result[..start],
                inner,
                &result[start + 1 + end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

/// HTML → plain text: strip all tags.
fn html_to_text(html: &str) -> String {
    strip_xml_tags(html)
}

/// HTML → Markdown: basic conversion using fast_html2md (crate name: html2md).
fn html_to_md(html: &str) -> String {
    html2md::rewrite_html(html, false)
}

/// DOCX → HTML: extract text then wrap in minimal HTML.
fn docx_to_html(path: &Path) -> AppResult<String> {
    let text = extract_docx_text(path)?;
    let html_body: String = text
        .lines()
        .map(|l| {
            if l.is_empty() {
                "<br>".to_string()
            } else {
                format!("<p>{}</p>", l)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"></head><body>\n{}\n</body></html>",
        html_body
    ))
}

/// Convert .xlsx/.xls/.ods → CSV using calamine.
fn spreadsheet_to_csv(path: &Path) -> AppResult<String> {
    use calamine::{open_workbook_auto, Data, Reader};

    let mut workbook = open_workbook_auto(path).map_err(|e| {
        ImpForgeError::validation("INVALID_SPREADSHEET", format!("Cannot open spreadsheet: {e}"))
    })?;

    let sheet_names = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names.first().ok_or_else(|| {
        ImpForgeError::validation("EMPTY_WORKBOOK", "Workbook has no sheets")
    })?;

    let range = workbook.worksheet_range(first_sheet).map_err(|e| {
        ImpForgeError::internal("SHEET_READ_ERROR", format!("Cannot read sheet: {e}"))
    })?;

    let mut csv_out = String::new();
    for row in range.rows() {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                Data::Empty => String::new(),
                Data::String(s) => {
                    if s.contains(',') || s.contains('"') || s.contains('\n') {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    } else {
                        s.clone()
                    }
                }
                Data::Float(f) => format!("{f}"),
                Data::Int(i) => format!("{i}"),
                Data::Bool(b) => format!("{b}"),
                Data::DateTime(dt) => format!("{dt}"),
                Data::DateTimeIso(s) => s.clone(),
                Data::DurationIso(s) => s.clone(),
                Data::Error(e) => format!("#ERR:{e:?}"),
            })
            .collect();
        csv_out.push_str(&cells.join(","));
        csv_out.push('\n');
    }

    Ok(csv_out)
}

/// Convert .xlsx/.xls/.ods → JSON array-of-objects (first row = headers).
fn spreadsheet_to_json(path: &Path) -> AppResult<String> {
    use calamine::{open_workbook_auto, Data, Reader};

    let mut workbook = open_workbook_auto(path).map_err(|e| {
        ImpForgeError::validation("INVALID_SPREADSHEET", format!("Cannot open spreadsheet: {e}"))
    })?;

    let sheet_names = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names.first().ok_or_else(|| {
        ImpForgeError::validation("EMPTY_WORKBOOK", "Workbook has no sheets")
    })?;

    let range = workbook.worksheet_range(first_sheet).map_err(|e| {
        ImpForgeError::internal("SHEET_READ_ERROR", format!("Cannot read sheet: {e}"))
    })?;

    let rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
    if rows.is_empty() {
        return Ok("[]".to_string());
    }

    // First row as headers
    let headers: Vec<String> = rows[0]
        .iter()
        .enumerate()
        .map(|(i, cell)| match cell {
            Data::String(s) => s.clone(),
            _ => format!("column_{}", i + 1),
        })
        .collect();

    let mut records: Vec<serde_json::Value> = Vec::new();
    for row in rows.iter().skip(1) {
        let mut obj = serde_json::Map::new();
        for (i, cell) in row.iter().enumerate() {
            let key = headers.get(i).cloned().unwrap_or_else(|| format!("col_{i}"));
            let val = match cell {
                Data::Empty => serde_json::Value::Null,
                Data::String(s) => serde_json::Value::String(s.clone()),
                Data::Float(f) => serde_json::json!(f),
                Data::Int(i) => serde_json::json!(i),
                Data::Bool(b) => serde_json::Value::Bool(*b),
                _ => serde_json::Value::String(format!("{cell:?}")),
            };
            obj.insert(key, val);
        }
        records.push(serde_json::Value::Object(obj));
    }

    serde_json::to_string_pretty(&records).map_err(ImpForgeError::from)
}

/// Convert .xlsx/.xls/.ods → TSV.
fn spreadsheet_to_tsv(path: &Path) -> AppResult<String> {
    let csv_content = spreadsheet_to_csv(path)?;
    // Simple CSV→TSV: replace commas with tabs (handles quoted fields properly via re-parse)
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());
    let mut tsv = String::new();
    for result in rdr.records() {
        let record = result.map_err(|e| {
            ImpForgeError::internal("CSV_PARSE", format!("CSV parse error: {e}"))
        })?;
        let fields: Vec<&str> = record.iter().collect();
        tsv.push_str(&fields.join("\t"));
        tsv.push('\n');
    }
    Ok(tsv)
}

/// Convert CSV → XLSX using rust_xlsxwriter.
fn csv_to_xlsx(csv_path: &Path, output_path: &Path) -> AppResult<()> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(csv_path)
        .map_err(|e| ImpForgeError::validation("CSV_READ", format!("Cannot read CSV: {e}")))?;

    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();

    for (row_idx, result) in rdr.records().enumerate() {
        let record = result.map_err(|e| {
            ImpForgeError::internal("CSV_PARSE", format!("CSV parse error: {e}"))
        })?;
        for (col_idx, field) in record.iter().enumerate() {
            // Try parsing as number first
            if let Ok(num) = field.parse::<f64>() {
                let _ = sheet.write_number(row_idx as u32, col_idx as u16, num);
            } else {
                let _ = sheet.write_string(row_idx as u32, col_idx as u16, field);
            }
        }
    }

    workbook.save(output_path).map_err(|e| {
        ImpForgeError::filesystem("XLSX_WRITE", format!("Cannot write XLSX: {e}"))
    })?;

    Ok(())
}

/// JSON ↔ YAML conversion.
fn json_to_yaml(json_str: &str) -> AppResult<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).map_err(ImpForgeError::from)?;
    // Use serde_json → serde_yaml would require the crate; do a simple recursive format instead
    Ok(json_value_to_yaml(&value, 0))
}

fn json_value_to_yaml(val: &serde_json::Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match val {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => format!("{b}"),
        serde_json::Value::Number(n) => format!("{n}"),
        serde_json::Value::String(s) => {
            if s.contains('\n') || s.contains(':') || s.contains('#') {
                format!("\"{}\"", s.replace('"', "\\\""))
            } else {
                s.clone()
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            let mut out = String::new();
            for item in arr {
                let formatted = json_value_to_yaml(item, indent + 1);
                out.push_str(&format!("\n{pad}- {formatted}"));
            }
            out
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut out = String::new();
            for (key, val) in map {
                let formatted = json_value_to_yaml(val, indent + 1);
                if matches!(val, serde_json::Value::Object(_) | serde_json::Value::Array(_))
                    && !formatted.starts_with('[')
                    && !formatted.starts_with('{')
                {
                    out.push_str(&format!("\n{pad}{key}:{formatted}"));
                } else {
                    out.push_str(&format!("\n{pad}{key}: {formatted}"));
                }
            }
            out
        }
    }
}

/// YAML → JSON conversion.
fn yaml_to_json(yaml_str: &str) -> AppResult<String> {
    // Parse YAML manually: for a robust solution we convert line-by-line
    // Simple approach: treat YAML as JSON-compatible subset (works for most data files)
    // For production, we'd add a YAML parser crate; here we do best-effort.
    //
    // Strategy: use serde_json to parse if it looks like JSON already,
    // otherwise wrap as a string value.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(yaml_str) {
        return serde_json::to_string_pretty(&val).map_err(ImpForgeError::from);
    }

    // Simple YAML key: value parser (flat structure)
    let mut map = serde_json::Map::new();
    for line in yaml_str.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let k = key.trim().to_string();
            let v = value.trim();
            let json_val = if v == "true" || v == "True" {
                serde_json::Value::Bool(true)
            } else if v == "false" || v == "False" {
                serde_json::Value::Bool(false)
            } else if v == "null" || v == "~" || v.is_empty() {
                serde_json::Value::Null
            } else if let Ok(n) = v.parse::<f64>() {
                serde_json::json!(n)
            } else {
                serde_json::Value::String(v.trim_matches('"').trim_matches('\'').to_string())
            };
            map.insert(k, json_val);
        }
    }

    serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(ImpForgeError::from)
}

/// TOML → JSON conversion.
fn toml_to_json(toml_str: &str) -> AppResult<String> {
    // Simple flat TOML parser (key = value pairs)
    let mut map = serde_json::Map::new();
    let mut current_section = String::new();

    for line in toml_str.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Section header
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].to_string();
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let full_key = if current_section.is_empty() {
                key.trim().to_string()
            } else {
                format!("{}.{}", current_section, key.trim())
            };
            let v = value.trim();
            let json_val = if v == "true" {
                serde_json::Value::Bool(true)
            } else if v == "false" {
                serde_json::Value::Bool(false)
            } else if let Ok(n) = v.parse::<f64>() {
                serde_json::json!(n)
            } else {
                serde_json::Value::String(v.trim_matches('"').to_string())
            };
            map.insert(full_key, json_val);
        }
    }

    serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(ImpForgeError::from)
}

/// JSON → CSV conversion (array of objects → CSV with header row).
fn json_to_csv(json_str: &str) -> AppResult<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).map_err(ImpForgeError::from)?;
    let arr = value.as_array().ok_or_else(|| {
        ImpForgeError::validation("JSON_NOT_ARRAY", "JSON must be an array of objects for CSV conversion")
    })?;

    if arr.is_empty() {
        return Ok(String::new());
    }

    // Collect all keys from all objects for headers
    let mut headers: Vec<String> = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    let mut csv_out = headers.join(",");
    csv_out.push('\n');

    for item in arr {
        let obj = item.as_object();
        let row: Vec<String> = headers
            .iter()
            .map(|h| {
                obj.and_then(|o| o.get(h))
                    .map(|v| match v {
                        serde_json::Value::String(s) => {
                            if s.contains(',') || s.contains('"') || s.contains('\n') {
                                format!("\"{}\"", s.replace('"', "\"\""))
                            } else {
                                s.clone()
                            }
                        }
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default()
            })
            .collect();
        csv_out.push_str(&row.join(","));
        csv_out.push('\n');
    }

    Ok(csv_out)
}

/// Build output path: same directory, same stem, new extension.
fn output_path_for(source: &Path, target_ext: &str) -> PathBuf {
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let parent = source.parent().unwrap_or(Path::new("."));
    let mut out = parent.join(format!("{stem}.{target_ext}"));

    // If output already exists, add a timestamp suffix
    if out.exists() {
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        out = parent.join(format!("{stem}_{ts}.{target_ext}"));
    }

    out
}

/// Master conversion dispatcher.
fn convert_file(source: &Path, target_format: &str) -> AppResult<ConversionResult> {
    let ext = normalize_ext(source);
    let target = target_format.trim_start_matches('.').to_ascii_lowercase();

    // Read source content for text-based conversions
    let read_text = || -> AppResult<String> {
        std::fs::read_to_string(source).map_err(ImpForgeError::from)
    };

    let (output_content, out_path) = match (ext.as_str(), target.as_str()) {
        // ── DOCX conversions ──
        ("docx", "txt") => {
            let text = extract_docx_text(source)?;
            let p = output_path_for(source, "txt");
            (text, p)
        }
        ("docx", "md") => {
            let text = extract_docx_text(source)?;
            let p = output_path_for(source, "md");
            (text, p)
        }
        ("docx", "html") => {
            let html = docx_to_html(source)?;
            let p = output_path_for(source, "html");
            (html, p)
        }

        // ── PPTX conversions ──
        ("pptx", "txt") => {
            let text = extract_pptx_text(source)?;
            let p = output_path_for(source, "txt");
            (text, p)
        }

        // ── Markdown conversions ──
        ("md", "html") => {
            let md = read_text()?;
            let html = md_to_html(&md);
            let p = output_path_for(source, "html");
            (html, p)
        }
        ("md", "txt") => {
            let text = read_text()?;
            let p = output_path_for(source, "txt");
            (text, p)
        }

        // ── HTML conversions ──
        ("html" | "htm", "md") => {
            let html = read_text()?;
            let md = html_to_md(&html);
            let p = output_path_for(source, "md");
            (md, p)
        }
        ("html" | "htm", "txt") => {
            let html = read_text()?;
            let text = html_to_text(&html);
            let p = output_path_for(source, "txt");
            (text, p)
        }

        // ── Plain text → other text ──
        ("txt", "md") => {
            let text = read_text()?;
            let p = output_path_for(source, "md");
            (text, p)
        }
        ("txt", "html") => {
            let text = read_text()?;
            let html = md_to_html(&text);
            let p = output_path_for(source, "html");
            (html, p)
        }

        // ── Spreadsheet conversions ──
        ("xlsx" | "xls" | "ods" | "xlsb", "csv") => {
            let csv_data = spreadsheet_to_csv(source)?;
            let p = output_path_for(source, "csv");
            (csv_data, p)
        }
        ("xlsx" | "xls" | "ods" | "xlsb", "json") => {
            let json_data = spreadsheet_to_json(source)?;
            let p = output_path_for(source, "json");
            (json_data, p)
        }
        ("xlsx" | "xls" | "ods" | "xlsb", "tsv") => {
            let tsv_data = spreadsheet_to_tsv(source)?;
            let p = output_path_for(source, "tsv");
            (tsv_data, p)
        }

        // ── CSV → XLSX (binary output) ──
        ("csv", "xlsx") => {
            let p = output_path_for(source, "xlsx");
            csv_to_xlsx(source, &p)?;
            return Ok(ConversionResult {
                success: true,
                output_path: p.to_string_lossy().to_string(),
                source_format: ext,
                target_format: target,
                message: "CSV converted to XLSX".to_string(),
            });
        }
        ("csv", "json") => {
            let csv_text = read_text()?;
            // Parse CSV to JSON array
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(csv_text.as_bytes());
            let headers: Vec<String> = rdr
                .headers()
                .map_err(|e| ImpForgeError::validation("CSV_HEADERS", format!("{e}")))?
                .iter()
                .map(|s| s.to_string())
                .collect();
            let mut records = Vec::new();
            for result in rdr.records() {
                let record = result.map_err(|e| ImpForgeError::internal("CSV_PARSE", format!("{e}")))?;
                let mut obj = serde_json::Map::new();
                for (i, field) in record.iter().enumerate() {
                    let key = headers.get(i).cloned().unwrap_or_else(|| format!("col_{i}"));
                    obj.insert(key, serde_json::Value::String(field.to_string()));
                }
                records.push(serde_json::Value::Object(obj));
            }
            let json = serde_json::to_string_pretty(&records).map_err(ImpForgeError::from)?;
            let p = output_path_for(source, "json");
            (json, p)
        }
        ("csv", "tsv") => {
            let csv_text = read_text()?;
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(csv_text.as_bytes());
            let mut tsv = String::new();
            for result in rdr.records() {
                let record = result.map_err(|e| ImpForgeError::internal("CSV_PARSE", format!("{e}")))?;
                let fields: Vec<&str> = record.iter().collect();
                tsv.push_str(&fields.join("\t"));
                tsv.push('\n');
            }
            let p = output_path_for(source, "tsv");
            (tsv, p)
        }
        ("tsv", "csv") => {
            let tsv_text = read_text()?;
            let csv_data: String = tsv_text.lines().map(|l| l.replace('\t', ",")).collect::<Vec<_>>().join("\n");
            let p = output_path_for(source, "csv");
            (csv_data, p)
        }
        ("tsv", "json") => {
            // TSV → CSV → JSON
            let tsv_text = read_text()?;
            let csv_data: String = tsv_text.lines().map(|l| l.replace('\t', ",")).collect::<Vec<_>>().join("\n");
            let json = json_from_csv_string(&csv_data)?;
            let p = output_path_for(source, "json");
            (json, p)
        }

        // ── Data format conversions ──
        ("json", "yaml" | "yml") => {
            let json = read_text()?;
            let yaml = json_to_yaml(&json)?;
            let p = output_path_for(source, "yaml");
            (yaml, p)
        }
        ("json", "csv") => {
            let json = read_text()?;
            let csv_data = json_to_csv(&json)?;
            let p = output_path_for(source, "csv");
            (csv_data, p)
        }
        ("yaml" | "yml", "json") => {
            let yaml = read_text()?;
            let json = yaml_to_json(&yaml)?;
            let p = output_path_for(source, "json");
            (json, p)
        }
        ("toml", "json") => {
            let toml = read_text()?;
            let json = toml_to_json(&toml)?;
            let p = output_path_for(source, "json");
            (json, p)
        }

        _ => {
            return Err(ImpForgeError::validation(
                "UNSUPPORTED_CONVERSION",
                format!("Conversion from .{ext} to .{target} is not supported"),
            )
            .with_suggestion(format!(
                "Supported conversions for .{ext}: check file_supported_formats()"
            )));
        }
    };

    // Write the output file
    std::fs::write(&out_path, &output_content).map_err(ImpForgeError::from)?;

    Ok(ConversionResult {
        success: true,
        output_path: out_path.to_string_lossy().to_string(),
        source_format: ext,
        target_format: target,
        message: format!(
            "Converted successfully ({} bytes written)",
            output_content.len()
        ),
    })
}

/// Helper: parse CSV string content to JSON array of objects.
fn json_from_csv_string(csv_str: &str) -> AppResult<String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_str.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| ImpForgeError::validation("CSV_HEADERS", format!("{e}")))?
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut records = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| ImpForgeError::internal("CSV_PARSE", format!("{e}")))?;
        let mut obj = serde_json::Map::new();
        for (i, field) in record.iter().enumerate() {
            let key = headers.get(i).cloned().unwrap_or_else(|| format!("col_{i}"));
            obj.insert(key, serde_json::Value::String(field.to_string()));
        }
        records.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string_pretty(&records).map_err(ImpForgeError::from)
}

// ---------------------------------------------------------------------------
// Recent files management (in-memory + on-disk JSON)
// ---------------------------------------------------------------------------

fn recent_files_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
        .join("recent_files.json")
}

fn load_recent_files() -> Vec<FileInfo> {
    let path = recent_files_path();
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_recent_file(info: &FileInfo) {
    let path = recent_files_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut recents = load_recent_files();

    // Remove existing entry for same path
    recents.retain(|f| f.path != info.path);

    // Insert at front
    recents.insert(0, info.clone());

    // Trim to limit
    recents.truncate(RECENT_FILES_LIMIT);

    let _ = std::fs::write(&path, serde_json::to_string_pretty(&recents).unwrap_or_default());
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Detect file type by extension + magic bytes, return full info.
#[tauri::command]
pub async fn file_detect(path: String) -> Result<FileInfo, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
            .to_json_string());
    }

    let info = build_file_info(&p).map_err(|e| e.to_json_string())?;

    // Track in recent files
    save_recent_file(&info);

    Ok(info)
}

/// Return text preview (first 5000 chars for text files, metadata for binary).
#[tauri::command]
pub async fn file_preview(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
            .to_json_string());
    }

    let ext = normalize_ext(&p);

    let preview = match ext.as_str() {
        // DOCX: extract text from ZIP
        "docx" => extract_docx_text(&p).map_err(|e| e.to_json_string())?,
        // PPTX: extract text from slides
        "pptx" => extract_pptx_text(&p).map_err(|e| e.to_json_string())?,
        // Spreadsheets: use calamine → CSV preview
        "xlsx" | "xls" | "ods" | "xlsb" => {
            let csv = spreadsheet_to_csv(&p).map_err(|e| e.to_json_string())?;
            // Limit preview to first 5000 chars
            if csv.len() > TEXT_PREVIEW_LIMIT {
                format!("{}...\n\n(Preview truncated)", &csv[..TEXT_PREVIEW_LIMIT])
            } else {
                csv
            }
        }
        // Text-based files
        _ => {
            let info = build_file_info(&p).map_err(|e| e.to_json_string())?;
            if info.can_preview {
                text_preview(&p, TEXT_PREVIEW_LIMIT).map_err(|e| e.to_json_string())?
            } else {
                // Binary file: return metadata summary
                format!(
                    "Binary file: {}\nSize: {} bytes\nType: {}\nMIME: {}",
                    info.name, info.size_bytes, info.category, info.mime_type
                )
            }
        }
    };

    Ok(preview)
}

/// Convert between formats.
#[tauri::command]
pub async fn file_convert(path: String, target_format: String) -> Result<ConversionResult, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
            .to_json_string());
    }

    convert_file(&p, &target_format).map_err(|e| e.to_json_string())
}

/// Auto-route to the right ImpForge module.  Returns the route URL to navigate to.
#[tauri::command]
pub async fn file_open_in_module(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
            .to_json_string());
    }

    let info = build_file_info(&p).map_err(|e| e.to_json_string())?;

    // Track in recent files
    save_recent_file(&info);

    let route = match info.recommended_module.as_str() {
        "writer" => format!("/writer?open={}", urlencoded(&path)),
        "sheets" => format!("/sheets?import={}", urlencoded(&path)),
        "pdf" => format!("/pdf?import={}", urlencoded(&path)),
        "ide" => format!("/ide?open={}", urlencoded(&path)),
        _ => format!("/files?preview={}", urlencoded(&path)),
    };

    Ok(route)
}

/// Convert multiple files at once.
#[tauri::command]
pub async fn file_batch_convert(
    paths: Vec<String>,
    target_format: String,
) -> Result<Vec<ConversionResult>, String> {
    let mut results = Vec::with_capacity(paths.len());

    for path_str in &paths {
        let p = PathBuf::from(path_str);
        if !p.exists() {
            results.push(ConversionResult {
                success: false,
                output_path: String::new(),
                source_format: normalize_ext(&p),
                target_format: target_format.clone(),
                message: format!("File not found: {path_str}"),
            });
            continue;
        }

        match convert_file(&p, &target_format) {
            Ok(r) => results.push(r),
            Err(e) => {
                results.push(ConversionResult {
                    success: false,
                    output_path: String::new(),
                    source_format: normalize_ext(&p),
                    target_format: target_format.clone(),
                    message: e.message,
                });
            }
        }
    }

    Ok(results)
}

/// AI-powered document summary via Ollama.
#[tauri::command]
pub async fn file_ai_digest(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
            .to_json_string());
    }

    let ext = normalize_ext(&p);

    // Extract text content
    let text = match ext.as_str() {
        "docx" => extract_docx_text(&p).map_err(|e| e.to_json_string())?,
        "pptx" => extract_pptx_text(&p).map_err(|e| e.to_json_string())?,
        "xlsx" | "xls" | "ods" => spreadsheet_to_csv(&p).map_err(|e| e.to_json_string())?,
        _ => {
            let info = build_file_info(&p).map_err(|e| e.to_json_string())?;
            if info.can_preview {
                text_preview(&p, 10_000).map_err(|e| e.to_json_string())?
            } else {
                return Err(ImpForgeError::validation(
                    "NOT_TEXT",
                    "Cannot generate AI digest for binary files",
                )
                .to_json_string());
            }
        }
    };

    if text.trim().is_empty() {
        return Ok("(No text content to digest)".to_string());
    }

    // Truncate to reasonable prompt size
    let truncated = if text.len() > 8000 {
        format!("{}...\n\n(Truncated — original is {} chars)", &text[..8000], text.len())
    } else {
        text
    };

    let prompt = format!(
        "Analyze this document and provide a structured digest:\n\
         1. **Summary** (2-3 sentences)\n\
         2. **Key Points** (bullet list)\n\
         3. **Document Type** (report, email, code, data, etc.)\n\
         4. **Action Items** (if any)\n\n\
         ---\n\n{}",
        truncated
    );

    // Call Ollama
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_DIGEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| ImpForgeError::internal("HTTP_CLIENT", format!("{e}")).to_json_string())?;

    let ollama_url = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let body = serde_json::json!({
        "model": "dolphin3:8b",
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 1024,
        }
    });

    let resp = client
        .post(format!("{ollama_url}/api/generate"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("OLLAMA_UNREACHABLE", format!("Cannot reach Ollama: {e}"))
                .with_suggestion("Start Ollama with: ollama serve")
                .to_json_string()
        })?;

    if !resp.status().is_success() {
        return Err(ImpForgeError::service(
            "OLLAMA_ERROR",
            format!("Ollama returned status {}", resp.status()),
        )
        .to_json_string());
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        ImpForgeError::internal("JSON_PARSE", format!("Cannot parse Ollama response: {e}"))
            .to_json_string()
    })?;

    Ok(json["response"]
        .as_str()
        .unwrap_or("(No response from AI)")
        .to_string())
}

/// Return all supported formats with capabilities.
#[tauri::command]
pub async fn file_supported_formats() -> Result<Vec<FormatInfo>, String> {
    let db = extension_db();
    let mut formats: Vec<FormatInfo> = db
        .iter()
        .map(|(ext, meta)| FormatInfo {
            extension: ext.to_string(),
            category: meta.category.clone(),
            mime_type: meta.mime.to_string(),
            can_preview: meta.can_preview,
            can_edit: meta.can_edit,
            can_convert_to: meta.convert_targets.iter().map(|s| s.to_string()).collect(),
            description: meta.description.to_string(),
        })
        .collect();

    formats.sort_by(|a, b| {
        a.category
            .to_string()
            .cmp(&b.category.to_string())
            .then(a.extension.cmp(&b.extension))
    });

    Ok(formats)
}

/// Recently opened files (from persistent storage).
#[tauri::command]
pub async fn file_recent() -> Result<Vec<FileInfo>, String> {
    Ok(load_recent_files())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal URL encoding for query parameters.
fn urlencoded(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_ext() {
        assert_eq!(normalize_ext(Path::new("file.DOCX")), "docx");
        assert_eq!(normalize_ext(Path::new("file.Rs")), "rs");
        assert_eq!(normalize_ext(Path::new("noext")), "");
    }

    #[test]
    fn test_strip_xml_tags() {
        let xml = "<w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:p><w:r><w:t>World</w:t></w:r></w:p>";
        let text = strip_xml_tags(xml);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_md_to_html() {
        let md = "# Hello\n\nThis is **bold** and *italic*.";
        let html = md_to_html(md);
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_inline_md() {
        assert_eq!(inline_md("**bold**"), "<strong>bold</strong>");
        assert_eq!(inline_md("`code`"), "<code>code</code>");
    }

    #[test]
    fn test_json_to_yaml_simple() {
        let json = r#"{"name": "test", "value": 42}"#;
        let yaml = json_to_yaml(json).unwrap();
        assert!(yaml.contains("name: test"));
        assert!(yaml.contains("value: 42"));
    }

    #[test]
    fn test_file_category_display() {
        assert_eq!(FileCategory::Document.to_string(), "Document");
        assert_eq!(FileCategory::ThreeD.to_string(), "3D Model");
        assert_eq!(FileCategory::Pdf.to_string(), "PDF");
    }

    #[test]
    fn test_extension_db_coverage() {
        let db = extension_db();
        // Verify key formats are present
        assert!(db.contains_key("docx"));
        assert!(db.contains_key("xlsx"));
        assert!(db.contains_key("pdf"));
        assert!(db.contains_key("rs"));
        assert!(db.contains_key("json"));
        assert!(db.contains_key("mp4"));
        assert!(db.contains_key("png"));
        assert!(db.contains_key("zip"));
        assert!(db.contains_key("ttf"));
        assert!(db.contains_key("obj"));
        assert!(db.contains_key("eml"));
        // Should have 100+ entries
        assert!(db.len() > 100, "Expected 100+ formats, got {}", db.len());
    }

    #[test]
    fn test_output_path_for() {
        let src = Path::new("/tmp/test.docx");
        let out = output_path_for(src, "md");
        assert_eq!(out, PathBuf::from("/tmp/test.md"));
    }

    #[test]
    fn test_urlencoded() {
        assert_eq!(urlencoded("/path/to file.txt"), "/path/to%20file.txt");
        assert_eq!(urlencoded("a=b&c"), "a%3Db%26c");
    }

    #[test]
    fn test_html_to_text() {
        let html = "<p>Hello</p><p>World</p>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_json_to_csv_simple() {
        let json = r#"[{"name":"Alice","age":"30"},{"name":"Bob","age":"25"}]"#;
        let csv = json_to_csv(json).unwrap();
        assert!(csv.contains("name"));
        assert!(csv.contains("Alice"));
        assert!(csv.contains("Bob"));
    }

    #[test]
    fn test_toml_to_json_simple() {
        let toml = "[package]\nname = \"test\"\nversion = \"0.1.0\"";
        let json = toml_to_json(toml).unwrap();
        assert!(json.contains("package.name"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_extract_slide_number() {
        assert_eq!(extract_slide_number("ppt/slides/slide1.xml"), 1);
        assert_eq!(extract_slide_number("ppt/slides/slide12.xml"), 12);
        assert_eq!(extract_slide_number("ppt/slides/slide100.xml"), 100);
    }
}
