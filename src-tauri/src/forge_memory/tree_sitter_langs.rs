//! Tree-sitter language registry for AST-aware chunking.
//!
//! Maps language name strings (from `detect_language()` in watch.rs) to tree-sitter
//! `Language` objects for use with text-splitter's `CodeSplitter`.
//!
//! Covers 32 languages via tree-sitter grammars. SCSS is handled by the CSS
//! grammar (detect_language maps .scss→"css").
//!
//! All grammar crates used here expose a `LANGUAGE` constant of type `LanguageFn`
//! (from the `tree-sitter-language` bridge crate), which converts to
//! `tree_sitter::Language` via `.into()`.
//!
//! Scientific basis: cAST (arXiv:2506.15655) — AST-aware chunking achieves
//! +4.3 Recall@5 vs fixed-size splitting.

use tree_sitter::Language;

/// Get the tree-sitter `Language` for a given language name.
///
/// Language names match the strings returned by `detect_language()` in `watch.rs`.
/// Returns `None` for languages without a compatible tree-sitter grammar.
///
/// # Unsupported languages
///
/// Languages like `"text"`, `"latex"`, `"clojure"`, `"protobuf"` return `None`
/// as they have no tree-sitter grammar. SCSS is handled by the CSS grammar
/// since `detect_language()` maps `.scss` → `"css"`.
///
/// # Examples
///
/// ```rust,ignore
/// use impforge_lib::forge_memory::tree_sitter_langs::get_tree_sitter_language;
///
/// let lang = get_tree_sitter_language("rust").expect("Rust grammar available");
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&lang).unwrap();
/// ```
pub fn get_tree_sitter_language(lang: &str) -> Option<Language> {
    match lang {
        // -----------------------------------------------------------------
        // Modern grammar crates (tree-sitter-language LanguageFn → .into())
        // -----------------------------------------------------------------
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "csharp" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        "ruby" => Some(tree_sitter_ruby::LANGUAGE.into()),
        "php" => Some(tree_sitter_php::LANGUAGE_PHP.into()),
        "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        "scala" => Some(tree_sitter_scala::LANGUAGE.into()),
        "lua" => Some(tree_sitter_lua::LANGUAGE.into()),
        "r" => Some(tree_sitter_r::LANGUAGE.into()),
        "julia" => Some(tree_sitter_julia::LANGUAGE.into()),
        "elixir" => Some(tree_sitter_elixir::LANGUAGE.into()),
        "shell" => Some(tree_sitter_bash::LANGUAGE.into()),
        "html" => Some(tree_sitter_html::LANGUAGE.into()),
        "css" => Some(tree_sitter_css::LANGUAGE.into()),
        "json" => Some(tree_sitter_json::LANGUAGE.into()),
        "yaml" => Some(tree_sitter_yaml::LANGUAGE.into()),
        "xml" => Some(tree_sitter_xml::LANGUAGE_XML.into()),
        "graphql" => Some(tree_sitter_graphql::LANGUAGE.into()),
        "zig" => Some(tree_sitter_zig::LANGUAGE.into()),

        // -----------------------------------------------------------------
        // Upgraded "-ng" grammar crates (modern replacements for old crates)
        // -----------------------------------------------------------------
        "kotlin" => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
        "svelte" => Some(tree_sitter_svelte_ng::LANGUAGE.into()),
        "toml" => Some(tree_sitter_toml_ng::LANGUAGE.into()),
        "markdown" => Some(tree_sitter_md::LANGUAGE.into()),

        // -----------------------------------------------------------------
        // Alternative grammar crates (grammar-orchard / community forks)
        // -----------------------------------------------------------------
        "dart" => Some(tree_sitter_dart_orchard::LANGUAGE.into()),
        "sql" => Some(tree_sitter_sequel::LANGUAGE.into()),
        "vue" => Some(tree_sitter_vue_next::LANGUAGE.into()),

        // No grammar available (text, latex, clojure, protobuf, etc.)
        // SCSS handled by CSS grammar (detect_language maps .scss→"css")
        _ => None,
    }
}

/// Returns a list of all language names that have tree-sitter grammar support.
///
/// Useful for diagnostics and UI display. Languages not in this list will
/// fall back to text-based chunking.
pub fn supported_languages() -> &'static [&'static str] {
    &[
        "rust", "python", "javascript", "typescript", "c", "cpp", "go",
        "java", "csharp", "ruby", "php", "swift", "kotlin", "scala",
        "dart", "lua", "r", "julia", "elixir", "shell", "html", "css",
        "svelte", "vue", "markdown", "toml", "yaml", "json",
        "xml", "sql", "graphql", "zig",
    ]
}

/// Languages detected by `detect_language()` that lack a compatible tree-sitter grammar.
///
/// These fall back to text-based splitting. Includes both languages with no grammar
/// at all and those whose grammar crates have tree-sitter version conflicts.
pub fn unsupported_languages() -> &'static [&'static str] {
    &[
        // No tree-sitter grammar exists (or no crate on crates.io)
        "text", "latex", "clojure", "protobuf",
        // SCSS: handled by CSS grammar (detect_language maps .scss→"css")
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_rust_language() {
        let lang = get_tree_sitter_language("rust");
        assert!(lang.is_some(), "Rust grammar should be available");
    }

    #[test]
    fn test_get_python_language() {
        let lang = get_tree_sitter_language("python");
        assert!(lang.is_some(), "Python grammar should be available");
    }

    #[test]
    fn test_get_typescript_language() {
        let lang = get_tree_sitter_language("typescript");
        assert!(lang.is_some(), "TypeScript grammar should be available");
    }

    #[test]
    fn test_all_supported_languages() {
        for lang_name in supported_languages() {
            assert!(
                get_tree_sitter_language(lang_name).is_some(),
                "Language '{}' should have a tree-sitter grammar",
                lang_name
            );
        }
    }

    #[test]
    fn test_unknown_language_returns_none() {
        assert!(get_tree_sitter_language("unknown").is_none());
        assert!(get_tree_sitter_language("text").is_none());
        assert!(get_tree_sitter_language("latex").is_none());
        assert!(get_tree_sitter_language("clojure").is_none());
        assert!(get_tree_sitter_language("protobuf").is_none());
    }

    #[test]
    fn test_replacement_grammars() {
        // Verify -ng and alternative replacement grammars work
        assert!(get_tree_sitter_language("kotlin").is_some(), "Kotlin (ng) grammar should be available");
        assert!(get_tree_sitter_language("svelte").is_some(), "Svelte (ng) grammar should be available");
        assert!(get_tree_sitter_language("toml").is_some(), "TOML (ng) grammar should be available");
        assert!(get_tree_sitter_language("markdown").is_some(), "Markdown (md) grammar should be available");
        assert!(get_tree_sitter_language("dart").is_some(), "Dart (orchard) grammar should be available");
        assert!(get_tree_sitter_language("sql").is_some(), "SQL (sequel) grammar should be available");
        assert!(get_tree_sitter_language("vue").is_some(), "Vue (next) grammar should be available");
    }

    #[test]
    fn test_parser_validation() {
        // Verify we can actually create a parser from the language
        let lang = get_tree_sitter_language("rust").unwrap();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).expect("Rust parser should initialize");

        let source = r#"fn main() { println!("hello"); }"#;
        let tree = parser.parse(source, None).expect("Should parse Rust code");
        assert!(!tree.root_node().has_error(), "Parsed Rust should have no errors");
    }

    #[test]
    fn test_supported_languages_count() {
        // 25 modern + 4 ng + 3 alternative = 32 supported languages
        assert_eq!(
            supported_languages().len(),
            32,
            "Should support exactly 32 languages"
        );
    }
}
