// Public API — consumed via forge_memory::commands Tauri layer
#![allow(dead_code)]
//! Contextualized Chunk Headers (CCH) for ForgeMemory ingestion.
//!
//! Prepends programmatic metadata to each chunk before embedding and BM25
//! indexing. Both BM25 and vector embeddings index the contextualized text,
//! giving BM25 domain keywords from headers and embeddings semantic scope.
//!
//! This is purely programmatic — no LLM calls needed.
//!
//! Scientific basis:
//!   - Anthropic (2024). Contextual Retrieval — -49% retrieval failure rate
//!   - NirDiamant CCH: +27.9% retrieval score without LLM
//!   - Voyage-Context-3 (2025): contextualized embeddings

use std::path::Path;

/// Context metadata for a single chunk.
#[derive(Debug, Clone)]
pub struct ChunkContext {
    /// Shortened file path (last 3 segments)
    pub file_path: String,
    /// Detected language
    pub language: String,
    /// Module/namespace path extracted from the file
    pub module_path: Option<String>,
    /// First function/class/struct signature found in the chunk
    pub signature: Option<String>,
    /// Top imports from the file (up to 10)
    pub imports: Vec<String>,
    /// Section heading (for markdown)
    pub section: Option<String>,
}

/// Shorten a file path to the last N segments.
///
/// Example: `/home/user/project/src/lib/utils.rs` → `src/lib/utils.rs`
pub fn shorten_file_path(path: &str, segments: usize) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= segments {
        return parts.join("/");
    }
    parts[parts.len() - segments..].join("/")
}

/// Extract import statements from source code.
///
/// Supports 12+ language patterns:
/// - Rust: `use ...;`
/// - Python: `import ...`, `from ... import ...`
/// - JavaScript/TypeScript: `import ...`
/// - Go: `import ...`
/// - Java/Kotlin/Scala: `import ...`
/// - C/C++: `#include ...`
/// - Ruby: `require ...`
/// - PHP: `use ...;`
/// - Elixir: `import ...`, `use ...`
/// - Swift/Dart: `import ...`
/// - Lua: `require(...)`
/// - C#: `using ...;`
///
/// Returns up to `max_imports` import names.
pub fn extract_imports(source: &str, language: &str, max_imports: usize) -> Vec<String> {
    let mut imports = Vec::new();

    for line in source.lines() {
        if imports.len() >= max_imports {
            break;
        }
        let trimmed = line.trim();

        let is_import = match language {
            "rust" => trimmed.starts_with("use ") && trimmed.ends_with(';'),
            "python" => trimmed.starts_with("import ") || trimmed.starts_with("from "),
            "javascript" | "typescript" | "svelte" | "vue" => trimmed.starts_with("import "),
            "go" => trimmed.starts_with("import "),
            "java" | "kotlin" | "scala" | "dart" | "swift" => trimmed.starts_with("import "),
            "c" | "cpp" => trimmed.starts_with("#include"),
            "ruby" => trimmed.starts_with("require ") || trimmed.starts_with("require("),
            "php" => trimmed.starts_with("use ") && trimmed.ends_with(';'),
            "elixir" => {
                trimmed.starts_with("import ")
                    || trimmed.starts_with("use ")
                    || trimmed.starts_with("alias ")
            }
            "lua" => trimmed.contains("require(") || trimmed.contains("require \""),
            "csharp" => trimmed.starts_with("using ") && trimmed.ends_with(';'),
            _ => false,
        };

        if is_import {
            let simplified = simplify_import(trimmed, language);
            if !simplified.is_empty() {
                imports.push(simplified);
            }
        }
    }

    imports
}

/// Simplify an import line to just the key name/module.
fn simplify_import(line: &str, language: &str) -> String {
    let trimmed = line.trim().trim_end_matches(';');

    match language {
        "rust" => {
            // "use std::collections::HashMap" → "std::collections::HashMap"
            trimmed
                .strip_prefix("use ")
                .unwrap_or(trimmed)
                .trim()
                .to_string()
        }
        "python" => {
            if let Some(rest) = trimmed.strip_prefix("from ") {
                // "from os.path import join" → "os.path"
                rest.split_whitespace().next().unwrap_or("").to_string()
            } else {
                // "import os" → "os"
                trimmed
                    .strip_prefix("import ")
                    .unwrap_or(trimmed)
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            }
        }
        "c" | "cpp" => {
            // "#include <stdio.h>" → "stdio.h"
            trimmed
                .trim_start_matches("#include")
                .trim()
                .trim_matches(|c| c == '<' || c == '>' || c == '"')
                .to_string()
        }
        "csharp" => trimmed
            .strip_prefix("using ")
            .unwrap_or(trimmed)
            .trim()
            .to_string(),
        "javascript" | "typescript" | "svelte" | "vue" => {
            // Handle: import X from 'Y' → Y
            //         import { A } from 'Y' → Y
            //         import 'Y' → Y
            let rest = trimmed.strip_prefix("import ").unwrap_or(trimmed);
            if let Some(from_pos) = rest.find(" from ") {
                // Extract the module specifier after "from"
                rest[from_pos + 6..]
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string()
            } else {
                // Side-effect import: import 'module'
                rest.trim()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string()
            }
        }
        _ => {
            // Generic: "import foo" → "foo"
            trimmed
                .strip_prefix("import ")
                .unwrap_or(trimmed)
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '_' && c != '.' && c != '/' && c != ':' && c != '@'
                })
                .to_string()
        }
    }
}

/// Extract a module path from the file path.
///
/// Example: `src/forge_memory/watch.rs` → `forge_memory::watch`
pub fn extract_module_path(file_path: &str, language: &str) -> Option<String> {
    let path = Path::new(file_path);
    let stem = path.file_stem()?.to_str()?;

    // Skip index/mod files — they represent the parent module
    if stem == "mod" || stem == "index" || stem == "lib" || stem == "__init__" {
        return path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from);
    }

    let separator = match language {
        "rust" | "elixir" => "::",
        "python" | "java" | "kotlin" | "scala" => ".",
        "javascript" | "typescript" => "/",
        _ => "::",
    };

    // Build module path from directory + filename
    let parent = path.parent()?;
    let parent_name = parent.file_name()?.to_str()?;

    // Skip src/lib directories — they're not meaningful module names
    if parent_name == "src" || parent_name == "lib" {
        return Some(stem.to_string());
    }

    Some(format!("{}{}{}", parent_name, separator, stem))
}

/// Build the context header string from chunk metadata.
///
/// Format:
/// ```text
/// # file/path.rs
/// # Module: module::path
/// # Defines: fn function_name(...)
/// # Uses: dep1, dep2, dep3
/// ```
pub fn build_context_header(ctx: &ChunkContext) -> String {
    let mut lines = Vec::new();

    // Always include file path
    lines.push(format!("# {}", ctx.file_path));

    // Module path (if extracted)
    if let Some(ref module) = ctx.module_path {
        lines.push(format!("# Module: {}", module));
    }

    // Section heading (for markdown)
    if let Some(ref section) = ctx.section {
        lines.push(format!("# Section: {}", section));
    }

    // Signature (first symbol definition in chunk)
    if let Some(ref sig) = ctx.signature {
        lines.push(format!("# Defines: {}", sig));
    }

    // Imports (comma-separated, up to what we extracted)
    if !ctx.imports.is_empty() {
        lines.push(format!("# Uses: {}", ctx.imports.join(", ")));
    }

    lines.join("\n")
}

/// Contextualize a chunk by prepending its CCH header.
///
/// Returns the chunk content with a header prepended. This is the text
/// that should be stored, embedded, and indexed for retrieval.
pub fn contextualize_chunk(
    chunk_content: &str,
    file_path: &str,
    language: &str,
    full_source: &str,
) -> String {
    let short_path = shorten_file_path(file_path, 3);
    let module_path = extract_module_path(file_path, language);
    let imports = extract_imports(full_source, language, 10);

    // Extract first signature from the chunk itself
    let signature =
        chunk_content
            .lines()
            .find_map(|line| extract_signature_line(line.trim(), language));

    // Extract section heading for markdown
    let section = if language == "markdown" {
        chunk_content
            .lines()
            .find(|l| l.trim().starts_with('#'))
            .map(|l| l.trim().to_string())
    } else {
        None
    };

    let ctx = ChunkContext {
        file_path: short_path,
        language: language.to_string(),
        module_path,
        signature,
        imports,
        section,
    };

    let header = build_context_header(&ctx);

    format!("{}\n\n{}", header, chunk_content)
}

/// Extract a function/method signature from a line of code.
fn extract_signature_line(line: &str, language: &str) -> Option<String> {
    let sig_prefixes: &[&str] = match language {
        "rust" => &[
            "pub async fn ",
            "async fn ",
            "pub fn ",
            "fn ",
            "pub struct ",
            "struct ",
            "pub enum ",
            "enum ",
            "pub trait ",
            "trait ",
            "impl ",
        ],
        "python" => &["async def ", "def ", "class "],
        "javascript" | "typescript" => &[
            "export default function ",
            "export async function ",
            "export function ",
            "async function ",
            "function ",
            "export class ",
            "class ",
        ],
        "go" => &["func "],
        "java" | "kotlin" | "scala" => &[
            "public ", "private ", "protected ", "class ", "interface ", "fun ",
        ],
        "csharp" => &[
            "public ", "private ", "protected ", "internal ", "class ", "interface ", "struct ",
        ],
        "ruby" => &["def ", "class ", "module "],
        "php" => &[
            "public function ",
            "private function ",
            "protected function ",
            "function ",
            "class ",
        ],
        _ => &["fn ", "def ", "func ", "function ", "class "],
    };

    for prefix in sig_prefixes {
        if line.starts_with(prefix) {
            // Return just the signature line (truncated if too long)
            let sig = if line.len() > 120 {
                format!("{}...", &line[..117])
            } else {
                line.to_string()
            };
            return Some(sig);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Path shortening tests ───────────────────────────────────────────

    #[test]
    fn test_shorten_path_long() {
        assert_eq!(
            shorten_file_path("/home/user/project/src/lib/utils.rs", 3),
            "src/lib/utils.rs"
        );
    }

    #[test]
    fn test_shorten_path_short() {
        assert_eq!(shorten_file_path("main.rs", 3), "main.rs");
    }

    #[test]
    fn test_shorten_path_exact() {
        assert_eq!(shorten_file_path("a/b/c.rs", 3), "a/b/c.rs");
    }

    #[test]
    fn test_shorten_path_empty() {
        assert_eq!(shorten_file_path("", 3), "");
    }

    // ── Import extraction tests ─────────────────────────────────────────

    #[test]
    fn test_extract_imports_rust() {
        let source = "use std::collections::HashMap;\nuse serde::{Serialize, Deserialize};\n\nfn main() {}";
        let imports = extract_imports(source, "rust", 10);
        assert_eq!(imports.len(), 2);
        assert!(imports[0].contains("std::collections::HashMap"));
        assert!(imports[1].contains("serde::{Serialize, Deserialize}"));
    }

    #[test]
    fn test_extract_imports_python() {
        let source = "import os\nfrom pathlib import Path\nimport sys\n\ndef main(): pass";
        let imports = extract_imports(source, "python", 10);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0], "os");
        assert_eq!(imports[1], "pathlib");
        assert_eq!(imports[2], "sys");
    }

    #[test]
    fn test_extract_imports_typescript() {
        let source =
            "import React from 'react';\nimport { useState } from 'react';\n\nconst App = () => {};";
        let imports = extract_imports(source, "typescript", 10);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], "react");
        assert_eq!(imports[1], "react");
    }

    #[test]
    fn test_extract_imports_cpp() {
        let source = "#include <stdio.h>\n#include \"myheader.h\"\n\nint main() {}";
        let imports = extract_imports(source, "cpp", 10);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], "stdio.h");
        assert_eq!(imports[1], "myheader.h");
    }

    #[test]
    fn test_extract_imports_csharp() {
        let source = "using System;\nusing System.Collections.Generic;\n\nclass Foo {}";
        let imports = extract_imports(source, "csharp", 10);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], "System");
        assert_eq!(imports[1], "System.Collections.Generic");
    }

    #[test]
    fn test_extract_imports_max_limit() {
        let source = "use a;\nuse b;\nuse c;\nuse d;\nuse e;\n";
        let imports = extract_imports(source, "rust", 3);
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn test_extract_imports_no_match() {
        let source = "fn main() {}\nlet x = 42;";
        let imports = extract_imports(source, "rust", 10);
        assert!(imports.is_empty());
    }

    // ── Module path tests ───────────────────────────────────────────────

    #[test]
    fn test_module_path_rust() {
        assert_eq!(
            extract_module_path("src/forge_memory/watch.rs", "rust"),
            Some("forge_memory::watch".to_string())
        );
    }

    #[test]
    fn test_module_path_python() {
        assert_eq!(
            extract_module_path("src/utils/helpers.py", "python"),
            Some("utils.helpers".to_string())
        );
    }

    #[test]
    fn test_module_path_typescript() {
        assert_eq!(
            extract_module_path("src/components/Button.tsx", "typescript"),
            Some("components/Button".to_string())
        );
    }

    #[test]
    fn test_module_path_mod_file() {
        assert_eq!(
            extract_module_path("src/forge_memory/mod.rs", "rust"),
            Some("forge_memory".to_string())
        );
    }

    #[test]
    fn test_module_path_init_file() {
        assert_eq!(
            extract_module_path("src/utils/__init__.py", "python"),
            Some("utils".to_string())
        );
    }

    #[test]
    fn test_module_path_src_root() {
        assert_eq!(
            extract_module_path("src/main.rs", "rust"),
            Some("main".to_string())
        );
    }

    // ── Signature extraction tests ──────────────────────────────────────

    #[test]
    fn test_signature_rust_pub_fn() {
        assert_eq!(
            extract_signature_line("pub fn process(data: &[u8]) -> Result<()> {", "rust"),
            Some("pub fn process(data: &[u8]) -> Result<()> {".to_string())
        );
    }

    #[test]
    fn test_signature_rust_async() {
        assert_eq!(
            extract_signature_line("pub async fn fetch(url: &str) -> Result<Response> {", "rust"),
            Some("pub async fn fetch(url: &str) -> Result<Response> {".to_string())
        );
    }

    #[test]
    fn test_signature_python_def() {
        assert_eq!(
            extract_signature_line("def hello(name: str) -> None:", "python"),
            Some("def hello(name: str) -> None:".to_string())
        );
    }

    #[test]
    fn test_signature_python_class() {
        assert_eq!(
            extract_signature_line("class MyClass(Base):", "python"),
            Some("class MyClass(Base):".to_string())
        );
    }

    #[test]
    fn test_signature_no_match() {
        assert_eq!(extract_signature_line("let x = 42;", "rust"), None);
    }

    #[test]
    fn test_signature_truncation() {
        let long_line = format!("pub fn very_long_function_name({}) -> Result<()> {{", "a: u32, ".repeat(20));
        let result = extract_signature_line(&long_line, "rust").unwrap();
        assert!(result.len() <= 120);
        assert!(result.ends_with("..."));
    }

    // ── Header building tests ───────────────────────────────────────────

    #[test]
    fn test_build_header_code() {
        let ctx = ChunkContext {
            file_path: "src/lib/utils.rs".to_string(),
            language: "rust".to_string(),
            module_path: Some("lib::utils".to_string()),
            signature: Some("pub fn process(data: &[u8]) -> Result<()>".to_string()),
            imports: vec!["std::io".to_string(), "serde".to_string()],
            section: None,
        };
        let header = build_context_header(&ctx);
        assert!(header.contains("# src/lib/utils.rs"));
        assert!(header.contains("# Module: lib::utils"));
        assert!(header.contains("# Defines: pub fn process"));
        assert!(header.contains("# Uses: std::io, serde"));
    }

    #[test]
    fn test_build_header_markdown() {
        let ctx = ChunkContext {
            file_path: "docs/guide.md".to_string(),
            language: "markdown".to_string(),
            module_path: None,
            signature: None,
            imports: vec![],
            section: Some("## Installation".to_string()),
        };
        let header = build_context_header(&ctx);
        assert!(header.contains("# docs/guide.md"));
        assert!(header.contains("# Section: ## Installation"));
        assert!(!header.contains("Uses:"));
        assert!(!header.contains("Defines:"));
    }

    #[test]
    fn test_build_header_minimal() {
        let ctx = ChunkContext {
            file_path: "file.txt".to_string(),
            language: "text".to_string(),
            module_path: None,
            signature: None,
            imports: vec![],
            section: None,
        };
        let header = build_context_header(&ctx);
        assert_eq!(header, "# file.txt");
    }

    // ── Full contextualization tests ────────────────────────────────────

    #[test]
    fn test_contextualize_rust_chunk() {
        let source = "use std::io;\nuse serde::Serialize;\n\npub fn hello() {\n    println!(\"hello\");\n}\n";
        let chunk = "pub fn hello() {\n    println!(\"hello\");\n}";
        let result =
            contextualize_chunk(chunk, "/project/src/utils/greet.rs", "rust", source);
        assert!(
            result.contains("# src/utils/greet.rs"),
            "Should have file path"
        );
        assert!(
            result.contains("# Defines: pub fn hello()"),
            "Should have signature, got: {}",
            result
        );
        assert!(
            result.contains("# Uses: std::io"),
            "Should have imports"
        );
        assert!(
            result.contains("pub fn hello()"),
            "Should have original chunk"
        );
    }

    #[test]
    fn test_contextualize_markdown_chunk() {
        let source = "# Guide\n\nIntro text.\n\n## Setup\n\nSetup instructions here.\n";
        let chunk = "## Setup\n\nSetup instructions here.";
        let result = contextualize_chunk(chunk, "/docs/guide.md", "markdown", source);
        assert!(result.contains("# docs/guide.md"));
        assert!(result.contains("# Section: ## Setup"));
        assert!(result.contains("Setup instructions here."));
    }

    #[test]
    fn test_contextualize_python_chunk() {
        let source = "import os\nfrom typing import List\n\ndef process(items: List[str]) -> None:\n    pass\n";
        let chunk = "def process(items: List[str]) -> None:\n    pass";
        let result =
            contextualize_chunk(chunk, "/project/src/utils/process.py", "python", source);
        assert!(result.contains("# src/utils/process.py"));
        assert!(result.contains("# Module: utils.process"));
        assert!(result.contains("# Defines: def process"));
        assert!(result.contains("# Uses: os, typing"));
    }

    #[test]
    fn test_contextualize_preserves_chunk_content() {
        let chunk = "    let x = 42;\n    let y = x + 1;\n    println!(\"{}\", y);";
        let result = contextualize_chunk(chunk, "src/main.rs", "rust", "fn main() {}");
        // The original chunk must appear verbatim after the header
        assert!(result.ends_with(chunk));
    }
}
