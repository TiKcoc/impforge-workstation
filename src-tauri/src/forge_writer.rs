// SPDX-License-Identifier: Apache-2.0
//! ForgeWriter -- Document Management & AI Writing Assistant
//!
//! Provides a full document lifecycle: create, read, update, delete, export.
//! Documents are persisted as individual files in `~/.impforge/documents/`.
//! AI-assisted text operations use the local Ollama inference backend.
//!
//! This module is part of ImpForge Phase 3 (Office/Writing tools).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds document files.
const DOCUMENTS_DIR: &str = "documents";

/// Extension for the metadata sidecar files.
const META_EXT: &str = "meta.json";

/// Subdirectory under the documents dir for version snapshots.
const VERSIONS_DIR: &str = "versions";

/// Ollama HTTP timeout for AI-assist requests (generous for large context).
const AI_ASSIST_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported document storage formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    Markdown,
    Html,
    #[serde(rename = "plaintext")]
    PlainText,
}

impl DocFormat {
    /// File extension for persisted content files.
    fn extension(self) -> &'static str {
        match self {
            DocFormat::Markdown => "md",
            DocFormat::Html => "html",
            DocFormat::PlainText => "txt",
        }
    }

    /// Attempt to parse a format string from the frontend.
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "markdown" | "md" => DocFormat::Markdown,
            "html" => DocFormat::Html,
            _ => DocFormat::PlainText,
        }
    }
}

/// Full document representation returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub format: DocFormat,
    pub word_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub auto_saved: bool,
}

/// Lightweight metadata for document listings (no content body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub title: String,
    pub format: DocFormat,
    pub word_count: u32,
    pub updated_at: String,
    pub tags: Vec<String>,
}

/// Persisted sidecar metadata (JSON next to the content file).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaFile {
    id: String,
    title: String,
    format: DocFormat,
    word_count: u32,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
}

/// Word-count statistics returned by `writer_word_count`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordCountStats {
    pub words: u32,
    pub characters: u32,
    pub sentences: u32,
    pub paragraphs: u32,
    pub reading_time_min: f64,
}

/// Pre-built document template with skeleton content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub format: DocFormat,
    pub content: String,
    pub category: String,
}

/// Detailed document statistics (superset of WordCountStats).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStats {
    pub words: u32,
    pub characters: u32,
    pub characters_no_spaces: u32,
    pub sentences: u32,
    pub paragraphs: u32,
    pub reading_time_min: f64,
    pub speaking_time_min: f64,
    pub reading_level: String,
    pub most_used_words: Vec<WordFrequency>,
    pub heading_count: u32,
    pub link_count: u32,
    pub avg_sentence_length: f64,
}

/// Word frequency entry for most-used-words analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordFrequency {
    pub word: String,
    pub count: u32,
}

/// A saved document version (snapshot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocVersion {
    pub version_id: String,
    pub document_id: String,
    pub message: String,
    pub word_count: u32,
    pub created_at: String,
}

/// Internal representation of a persisted version snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionFile {
    version_id: String,
    document_id: String,
    message: String,
    word_count: u32,
    content: String,
    created_at: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the documents directory, creating it if necessary.
fn documents_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(DOCUMENTS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create documents directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Build the content file path for a given document.
fn content_path(dir: &Path, id: &str, format: DocFormat) -> PathBuf {
    dir.join(format!("{}.{}", id, format.extension()))
}

/// Build the metadata sidecar path for a given document.
fn meta_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.{}", id, META_EXT))
}

/// Read and parse a metadata sidecar file.
fn read_meta(path: &Path) -> Result<MetaFile, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "META_READ_FAILED",
            format!("Cannot read document metadata: {e}"),
        )
    })?;
    serde_json::from_str::<MetaFile>(&data).map_err(|e| {
        ImpForgeError::internal(
            "META_PARSE_FAILED",
            format!("Corrupt document metadata: {e}"),
        )
    })
}

/// Persist a metadata sidecar file atomically.
fn write_meta(path: &Path, meta: &MetaFile) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| {
        ImpForgeError::internal("META_SERIALIZE", format!("Cannot serialize metadata: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "META_WRITE_FAILED",
            format!("Cannot write document metadata: {e}"),
        )
    })
}

/// Count words in a text string (Unicode-aware, splits on whitespace).
fn count_words(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Compute detailed word-count statistics for a content string.
fn compute_stats(content: &str) -> WordCountStats {
    let words = count_words(content);
    let characters = content.chars().count() as u32;
    let sentences = content
        .chars()
        .filter(|c| matches!(c, '.' | '!' | '?'))
        .count()
        .max(if content.trim().is_empty() { 0 } else { 1 }) as u32;
    let paragraphs = content
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count()
        .max(if content.trim().is_empty() { 0 } else { 1 }) as u32;
    // Average adult reads ~238 words/min (Brysbaert 2019).
    let reading_time_min = if words > 0 {
        (words as f64 / 238.0 * 100.0).round() / 100.0
    } else {
        0.0
    };

    WordCountStats {
        words,
        characters,
        sentences,
        paragraphs,
        reading_time_min,
    }
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Resolve the document versions directory, creating it if necessary.
fn versions_dir(doc_id: &str) -> Result<PathBuf, ImpForgeError> {
    let base = documents_dir()?;
    let dir = base.join(VERSIONS_DIR).join(doc_id);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create versions directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Compute detailed document statistics.
fn compute_document_stats(content: &str) -> DocumentStats {
    let basic = compute_stats(content);

    let characters_no_spaces = content.chars().filter(|c| !c.is_whitespace()).count() as u32;

    // Average adult speaks ~150 words/min (Brysbaert 2019).
    let speaking_time_min = if basic.words > 0 {
        (basic.words as f64 / 150.0 * 100.0).round() / 100.0
    } else {
        0.0
    };

    let avg_sentence_length = if basic.sentences > 0 {
        (basic.words as f64 / basic.sentences as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    // Flesch-Kincaid approximation using avg sentence length
    // Simple heuristic: short sentences = easier reading
    let reading_level = match avg_sentence_length as u32 {
        0..=10 => "Elementary".to_string(),
        11..=15 => "Middle School".to_string(),
        16..=20 => "High School".to_string(),
        21..=25 => "College".to_string(),
        _ => "Graduate".to_string(),
    };

    // Count headings (lines starting with #)
    let heading_count = content
        .lines()
        .filter(|l| l.trim_start().starts_with('#'))
        .count() as u32;

    // Count markdown links [text](url)
    let link_count = content.matches("](").count() as u32;

    // Word frequency analysis (top 10, skip common stopwords)
    let stopwords: std::collections::HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "shall", "can", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "and", "but", "or", "nor", "not", "so", "yet", "both", "either",
        "neither", "each", "every", "all", "any", "few", "more", "most",
        "other", "some", "such", "no", "only", "own", "same", "than",
        "too", "very", "just", "it", "its", "this", "that", "these", "those",
        "i", "me", "my", "we", "our", "you", "your", "he", "him", "his",
        "she", "her", "they", "them", "their", "what", "which", "who",
    ].iter().copied().collect();

    let mut freq: HashMap<String, u32> = HashMap::new();
    for word in content.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if clean.len() >= 3 && !stopwords.contains(clean.as_str()) {
            *freq.entry(clean).or_insert(0) += 1;
        }
    }

    let mut most_used: Vec<WordFrequency> = freq
        .into_iter()
        .map(|(word, count)| WordFrequency { word, count })
        .collect();
    most_used.sort_by(|a, b| b.count.cmp(&a.count));
    most_used.truncate(10);

    DocumentStats {
        words: basic.words,
        characters: basic.characters,
        characters_no_spaces,
        sentences: basic.sentences,
        paragraphs: basic.paragraphs,
        reading_time_min: basic.reading_time_min,
        speaking_time_min,
        reading_level,
        most_used_words: most_used,
        heading_count,
        link_count,
        avg_sentence_length,
    }
}

/// Build the 10 built-in document templates.
fn builtin_templates() -> Vec<DocumentTemplate> {
    vec![
        DocumentTemplate {
            id: "business-letter".into(),
            name: "Business Letter".into(),
            description: "Professional business correspondence".into(),
            format: DocFormat::Markdown,
            category: "Business".into(),
            content: "# [Your Company Name]\n\n**Date:** [Date]\n\n**To:**\n[Recipient Name]\n[Company]\n[Address]\n\n**Subject:** [Subject Line]\n\nDear [Recipient],\n\nI am writing to [purpose of the letter].\n\n[Body paragraph 1 -- context and main message]\n\n[Body paragraph 2 -- supporting details]\n\n[Body paragraph 3 -- call to action]\n\nThank you for your time and consideration. I look forward to hearing from you.\n\nSincerely,\n\n[Your Name]\n[Your Title]\n[Contact Information]\n".into(),
        },
        DocumentTemplate {
            id: "report".into(),
            name: "Report".into(),
            description: "Structured business or technical report".into(),
            format: DocFormat::Markdown,
            category: "Business".into(),
            content: "# Report: [Title]\n\n**Author:** [Name] | **Date:** [Date] | **Version:** 1.0\n\n---\n\n## Executive Summary\n\n[Brief overview of findings and recommendations]\n\n## Introduction\n\n### Background\n[Context and background information]\n\n### Objectives\n- [Objective 1]\n- [Objective 2]\n\n## Methodology\n\n[Describe the approach taken]\n\n## Findings\n\n### Key Finding 1\n[Details and supporting data]\n\n### Key Finding 2\n[Details and supporting data]\n\n## Recommendations\n\n1. [Recommendation 1]\n2. [Recommendation 2]\n3. [Recommendation 3]\n\n## Conclusion\n\n[Summary of findings and next steps]\n\n## Appendix\n\n[Supporting materials, data tables, references]\n".into(),
        },
        DocumentTemplate {
            id: "resume".into(),
            name: "Resume / CV".into(),
            description: "Professional resume template".into(),
            format: DocFormat::Markdown,
            category: "Personal".into(),
            content: "# [Your Full Name]\n\n**[Professional Title]**\n\n[Email] | [Phone] | [City, Country] | [LinkedIn/Portfolio]\n\n---\n\n## Professional Summary\n\n[2-3 sentences summarizing your experience, skills, and career goals]\n\n## Experience\n\n### [Job Title] -- [Company Name]\n*[Start Date] -- [End Date]*\n\n- [Achievement/responsibility with quantified impact]\n- [Achievement/responsibility with quantified impact]\n- [Achievement/responsibility with quantified impact]\n\n### [Job Title] -- [Company Name]\n*[Start Date] -- [End Date]*\n\n- [Achievement/responsibility]\n- [Achievement/responsibility]\n\n## Education\n\n### [Degree] in [Field]\n**[University Name]** | *[Graduation Year]*\n\n## Skills\n\n- **Languages:** [List]\n- **Frameworks:** [List]\n- **Tools:** [List]\n- **Soft Skills:** [List]\n\n## Certifications\n\n- [Certification Name] -- [Issuer] ([Year])\n".into(),
        },
        DocumentTemplate {
            id: "invoice".into(),
            name: "Invoice".into(),
            description: "Professional invoice for freelancers and businesses".into(),
            format: DocFormat::Markdown,
            category: "Business".into(),
            content: "# INVOICE\n\n**Invoice #:** [INV-001]\n**Date:** [Date]\n**Due Date:** [Due Date]\n\n---\n\n**From:**\n[Your Name / Company]\n[Your Address]\n[Tax ID / VAT]\n\n**Bill To:**\n[Client Name]\n[Client Company]\n[Client Address]\n\n---\n\n## Services\n\n| Description | Quantity | Rate | Amount |\n|---|---|---|---|\n| [Service 1] | [Hrs/Units] | [Rate] | [Total] |\n| [Service 2] | [Hrs/Units] | [Rate] | [Total] |\n| [Service 3] | [Hrs/Units] | [Rate] | [Total] |\n\n---\n\n| | |\n|---|---|\n| **Subtotal** | [Amount] |\n| **Tax (X%)** | [Amount] |\n| **Total Due** | **[Amount]** |\n\n---\n\n**Payment Terms:** [Net 30 / Upon Receipt]\n**Payment Method:** [Bank Transfer / PayPal / etc.]\n\nThank you for your business!\n".into(),
        },
        DocumentTemplate {
            id: "meeting-notes".into(),
            name: "Meeting Notes".into(),
            description: "Structured meeting minutes with action items".into(),
            format: DocFormat::Markdown,
            category: "Business".into(),
            content: "# Meeting Notes: [Meeting Title]\n\n**Date:** [Date] | **Time:** [Start] -- [End]\n**Location:** [Room / Video Link]\n**Facilitator:** [Name]\n\n## Attendees\n\n- [Name 1] -- [Role]\n- [Name 2] -- [Role]\n- [Name 3] -- [Role]\n\n## Agenda\n\n1. [Topic 1]\n2. [Topic 2]\n3. [Topic 3]\n\n## Discussion\n\n### [Topic 1]\n- [Key point discussed]\n- [Decision made]\n\n### [Topic 2]\n- [Key point discussed]\n- [Open question]\n\n## Action Items\n\n| Action | Owner | Deadline | Status |\n|---|---|---|---|\n| [Task 1] | [Name] | [Date] | Pending |\n| [Task 2] | [Name] | [Date] | Pending |\n| [Task 3] | [Name] | [Date] | Pending |\n\n## Next Meeting\n\n**Date:** [Date] | **Agenda:** [Topics to cover]\n".into(),
        },
        DocumentTemplate {
            id: "contract".into(),
            name: "Contract".into(),
            description: "Basic service agreement / contract template".into(),
            format: DocFormat::Markdown,
            category: "Legal".into(),
            content: "# Service Agreement\n\n**Effective Date:** [Date]\n\n---\n\n## Parties\n\n**Provider:** [Your Name / Company] (\"Provider\")\n**Client:** [Client Name / Company] (\"Client\")\n\n## Scope of Work\n\nThe Provider agrees to deliver the following services:\n\n1. [Service description 1]\n2. [Service description 2]\n3. [Service description 3]\n\n## Timeline\n\n- **Start Date:** [Date]\n- **Milestones:** [Key dates]\n- **Completion Date:** [Date]\n\n## Compensation\n\n- **Total Fee:** [Amount] [Currency]\n- **Payment Schedule:** [e.g., 50% upfront, 50% on completion]\n- **Payment Method:** [Bank transfer / PayPal]\n\n## Terms and Conditions\n\n### Intellectual Property\n[IP ownership terms]\n\n### Confidentiality\n[NDA terms]\n\n### Termination\n[Termination conditions and notice period]\n\n### Limitation of Liability\n[Liability caps and exclusions]\n\n## Signatures\n\n**Provider:** _________________________ Date: _________\n\n**Client:** _________________________ Date: _________\n".into(),
        },
        DocumentTemplate {
            id: "proposal".into(),
            name: "Proposal".into(),
            description: "Project or business proposal".into(),
            format: DocFormat::Markdown,
            category: "Business".into(),
            content: "# Proposal: [Project Title]\n\n**Prepared for:** [Client Name]\n**Prepared by:** [Your Name / Company]\n**Date:** [Date]\n\n---\n\n## Executive Summary\n\n[1-2 paragraph overview of the proposed solution and its value]\n\n## Problem Statement\n\n[Describe the challenge or opportunity the client faces]\n\n## Proposed Solution\n\n### Overview\n[High-level description of your approach]\n\n### Key Features\n- [Feature 1]: [Benefit]\n- [Feature 2]: [Benefit]\n- [Feature 3]: [Benefit]\n\n## Methodology\n\n1. **Phase 1: Discovery** -- [Description]\n2. **Phase 2: Development** -- [Description]\n3. **Phase 3: Delivery** -- [Description]\n\n## Timeline\n\n| Phase | Duration | Deliverables |\n|---|---|---|\n| Discovery | [X weeks] | [Deliverables] |\n| Development | [X weeks] | [Deliverables] |\n| Delivery | [X weeks] | [Deliverables] |\n\n## Investment\n\n| Item | Cost |\n|---|---|\n| [Service 1] | [Amount] |\n| [Service 2] | [Amount] |\n| **Total** | **[Amount]** |\n\n## Why Us\n\n[Your qualifications, relevant experience, differentiators]\n\n## Next Steps\n\n1. [Step 1]\n2. [Step 2]\n3. [Step 3]\n".into(),
        },
        DocumentTemplate {
            id: "press-release".into(),
            name: "Press Release".into(),
            description: "Standard press release format".into(),
            format: DocFormat::Markdown,
            category: "Marketing".into(),
            content: "# FOR IMMEDIATE RELEASE\n\n## [Headline: Concise, Attention-Grabbing Statement]\n\n### [Subheadline: Additional Context]\n\n**[City, State]** -- **[Date]** -- [Opening paragraph: Who, What, When, Where, Why. This should summarize the entire story in 1-2 sentences.]\n\n[Second paragraph: Expand on the news. Provide more detail about the announcement, product, or event.]\n\n> \"[Quote from key spokesperson or executive],\" said [Name], [Title] at [Company]. \"[Additional context or vision statement.]\"\n\n[Third paragraph: Supporting details, statistics, background information.]\n\n[Fourth paragraph: Additional quotes, partner statements, or customer testimonials.]\n\n### About [Company Name]\n\n[Company boilerplate: 2-3 sentences about the company, its mission, and key facts.]\n\n### Media Contact\n\n[Name]\n[Title]\n[Email]\n[Phone]\n[Website]\n\n###\n".into(),
        },
        DocumentTemplate {
            id: "blog-post".into(),
            name: "Blog Post".into(),
            description: "Structured blog post with SEO considerations".into(),
            format: DocFormat::Markdown,
            category: "Content".into(),
            content: "# [Blog Post Title: Include Primary Keyword]\n\n*Published: [Date] | Author: [Name] | Reading time: X min*\n\n---\n\n**TL;DR:** [1-2 sentence summary for skimmers]\n\n## Introduction\n\n[Hook: Start with a question, statistic, or bold statement]\n\n[Context: Why this topic matters to your audience]\n\n[Thesis: What the reader will learn]\n\n## [Section 1: Main Point]\n\n[Explanation and context]\n\n- [Key takeaway 1]\n- [Key takeaway 2]\n\n## [Section 2: Supporting Point]\n\n[Details with examples or data]\n\n```\n[Code example or data if applicable]\n```\n\n## [Section 3: Practical Application]\n\n[How to apply this knowledge]\n\n### Step-by-step:\n\n1. [Step 1]\n2. [Step 2]\n3. [Step 3]\n\n## Key Takeaways\n\n- [Takeaway 1]\n- [Takeaway 2]\n- [Takeaway 3]\n\n## Conclusion\n\n[Summarize main points]\n\n[Call to action: What should the reader do next?]\n\n---\n\n*[Author bio and links]*\n".into(),
        },
        DocumentTemplate {
            id: "readme".into(),
            name: "README".into(),
            description: "Project README for software repositories".into(),
            format: DocFormat::Markdown,
            category: "Development".into(),
            content: "# [Project Name]\n\n[![License](badge-url)](license-url)\n\n> [One-line project description]\n\n## Features\n\n- [Feature 1]\n- [Feature 2]\n- [Feature 3]\n\n## Quick Start\n\n### Prerequisites\n\n- [Requirement 1]\n- [Requirement 2]\n\n### Installation\n\n```bash\n# Clone the repository\ngit clone [repo-url]\ncd [project-name]\n\n# Install dependencies\n[install command]\n\n# Run\n[run command]\n```\n\n## Usage\n\n```\n[Usage example]\n```\n\n## Configuration\n\n| Variable | Description | Default |\n|---|---|---|\n| `[VAR_1]` | [Description] | [Default] |\n| `[VAR_2]` | [Description] | [Default] |\n\n## Architecture\n\n```\n[project-name]/\n  src/           # Source code\n  tests/         # Test suite\n  docs/          # Documentation\n```\n\n## Contributing\n\n1. Fork the repository\n2. Create your feature branch (`git checkout -b feature/amazing-feature`)\n3. Commit your changes (`git commit -m 'Add amazing feature'`)\n4. Push to the branch (`git push origin feature/amazing-feature`)\n5. Open a Pull Request\n\n## License\n\n[License type] -- see [LICENSE](LICENSE) for details.\n".into(),
        },
    ]
}

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

// ---------------------------------------------------------------------------
// AI Assist (Ollama)
// ---------------------------------------------------------------------------

/// Send a text-manipulation prompt to Ollama and return the result.
async fn ollama_text_assist(
    instruction: &str,
    text: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let system_prompt = "You are a professional writing assistant inside ImpForge, \
        an AI Workstation. Follow the user's instruction precisely. \
        Return ONLY the improved text -- no explanations, no markdown fences, \
        no preamble. Preserve the original language unless told to translate.";

    let user_message = format!("{instruction}\n\n---\n\n{text}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_ASSIST_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for AI writing assist",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "AI writing assist timed out",
                )
                .with_suggestion("The text may be too long. Try selecting a shorter passage.")
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Ollama request failed: {e}"),
                )
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(
            ImpForgeError::service(
                "OLLAMA_HTTP_ERROR",
                format!("Ollama returned HTTP {status}"),
            )
            .with_details(body)
            .with_suggestion("Check Ollama logs. The model may not be downloaded yet."),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service(
            "OLLAMA_PARSE_ERROR",
            format!("Failed to parse Ollama response: {e}"),
        )
    })?;

    let content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(ImpForgeError::service(
            "OLLAMA_EMPTY_RESPONSE",
            "Ollama returned an empty response",
        ));
    }

    Ok(content)
}

/// Map a user-facing action name to an Ollama instruction prompt.
fn action_to_instruction(action: &str) -> Result<&'static str, ImpForgeError> {
    match action {
        "improve" => Ok("Improve the following text. Fix grammar, enhance clarity, and make it more professional while keeping the original meaning and tone."),
        "shorten" => Ok("Shorten the following text significantly while preserving the key information. Be concise."),
        "expand" => Ok("Expand the following text with more detail, examples, and depth. Keep the same style and tone."),
        "fix_grammar" => Ok("Fix all grammar, spelling, and punctuation errors in the following text. Do not change the meaning or style."),
        "translate_en" => Ok("Translate the following text to English. Preserve formatting."),
        "translate_de" => Ok("Translate the following text to German (Deutsch). Preserve formatting."),
        "summarize" => Ok("Write a concise summary of the following text. Capture the main points in 2-4 sentences."),
        other => Err(ImpForgeError::validation(
            "INVALID_ACTION",
            format!("Unknown AI action: '{other}'. Valid actions: improve, shorten, expand, fix_grammar, translate_en, translate_de, summarize"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all documents (metadata only, no content bodies).
#[tauri::command]
pub async fn writer_list_documents() -> AppResult<Vec<DocumentMeta>> {
    let dir = documents_dir()?;
    let mut docs: Vec<DocumentMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read documents dir: {e}"))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // Only process meta files
        if !name.ends_with(META_EXT) {
            continue;
        }
        if let Ok(meta) = read_meta(&path) {
            docs.push(DocumentMeta {
                id: meta.id,
                title: meta.title,
                format: meta.format,
                word_count: meta.word_count,
                updated_at: meta.updated_at,
                tags: meta.tags,
            });
        }
    }

    // Sort by updated_at descending (most recent first).
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(docs)
}

/// Get a single document by ID (full content included).
#[tauri::command]
pub async fn writer_get_document(id: String) -> AppResult<Document> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found"))
                .with_suggestion("The document may have been deleted or moved."),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    Ok(Document {
        id: meta.id,
        title: meta.title,
        content,
        format: meta.format,
        word_count: meta.word_count,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        tags: meta.tags,
        auto_saved: false,
    })
}

/// Create a new blank document.
#[tauri::command]
pub async fn writer_create_document(
    title: String,
    format: Option<String>,
) -> AppResult<Document> {
    let dir = documents_dir()?;
    let id = Uuid::new_v4().to_string();
    let fmt = format.as_deref().map(DocFormat::from_str_loose).unwrap_or(DocFormat::Markdown);
    let now = now_iso();

    let meta = MetaFile {
        id: id.clone(),
        title: title.clone(),
        format: fmt,
        word_count: 0,
        created_at: now.clone(),
        updated_at: now.clone(),
        tags: Vec::new(),
    };

    write_meta(&meta_path(&dir, &id), &meta)?;

    // Create empty content file
    let cp = content_path(&dir, &id, fmt);
    std::fs::write(&cp, "").map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot create document file: {e}"),
        )
    })?;

    log::info!("ForgeWriter: created document '{title}' ({})", id);

    Ok(Document {
        id,
        title,
        content: String::new(),
        format: fmt,
        word_count: 0,
        created_at: now.clone(),
        updated_at: now,
        tags: Vec::new(),
        auto_saved: false,
    })
}

/// Save (update) a document's title and/or content.
#[tauri::command]
pub async fn writer_save_document(
    id: String,
    title: Option<String>,
    content: String,
) -> AppResult<Document> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found"))
                .with_suggestion("Cannot save a document that does not exist. Create it first."),
        );
    }

    let mut meta = read_meta(&mp)?;
    let now = now_iso();

    if let Some(t) = title {
        meta.title = t;
    }
    meta.word_count = count_words(&content);
    meta.updated_at = now.clone();

    write_meta(&mp, &meta)?;

    // Write content file
    let cp = content_path(&dir, &id, meta.format);
    std::fs::write(&cp, &content).map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot save document content: {e}"),
        )
    })?;

    Ok(Document {
        id: meta.id,
        title: meta.title,
        content,
        format: meta.format,
        word_count: meta.word_count,
        created_at: meta.created_at,
        updated_at: now,
        tags: meta.tags,
        auto_saved: true,
    })
}

/// Delete a document and its sidecar metadata.
#[tauri::command]
pub async fn writer_delete_document(id: String) -> AppResult<()> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);

    // Remove both files (ignore errors on content -- it may not exist).
    let _ = std::fs::remove_file(&cp);
    std::fs::remove_file(&mp).map_err(|e| {
        ImpForgeError::filesystem(
            "DELETE_FAILED",
            format!("Cannot delete document metadata: {e}"),
        )
    })?;

    log::info!("ForgeWriter: deleted document '{}'", id);

    Ok(())
}

/// Export a document to a specified format. Returns the exported file path.
#[tauri::command]
pub async fn writer_export_document(
    id: String,
    export_format: String,
) -> AppResult<String> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    // Determine export directory (~/Documents or fallback to docs dir)
    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "EXPORT_DIR_FAILED",
                format!("Cannot create export directory: {e}"),
            )
        })?;
    }

    // Sanitize title for filename
    let safe_title: String = meta
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    let safe_title = if safe_title.is_empty() {
        "untitled".to_string()
    } else {
        safe_title
    };

    let (ext, body) = match export_format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => ("md", content.clone()),
        "html" => {
            let html_body = if meta.format == DocFormat::Markdown {
                // Basic markdown-to-HTML conversion (line-by-line)
                basic_md_to_html(&content)
            } else {
                content.clone()
            };
            let full_html = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"UTF-8\">\n  \
                 <title>{}</title>\n  <style>\n    body {{ font-family: 'Inter', sans-serif; \
                 max-width: 720px; margin: 2rem auto; padding: 0 1rem; line-height: 1.7; \
                 color: #e0e0e0; background: #1a1a2e; }}\n    h1,h2,h3 {{ color: #00f0ff; }}\n    \
                 pre {{ background: #16162a; padding: 1rem; border-radius: 6px; overflow-x: auto; }}\n    \
                 code {{ font-family: 'JetBrains Mono', monospace; }}\n  </style>\n</head>\n\
                 <body>\n{}\n</body>\n</html>",
                meta.title, html_body
            );
            ("html", full_html)
        }
        "txt" | "plaintext" | "text" => ("txt", strip_markdown(&content)),
        "pdf" => {
            return Err(
                ImpForgeError::validation(
                    "PDF_NOT_SUPPORTED",
                    "PDF export is not yet available",
                )
                .with_suggestion(
                    "Export as HTML and print to PDF from your browser, or use pandoc.",
                ),
            );
        }
        other => {
            return Err(ImpForgeError::validation(
                "INVALID_EXPORT_FORMAT",
                format!(
                    "Unsupported export format: '{other}'. Use: md, html, txt"
                ),
            ));
        }
    };

    let export_path = export_dir.join(format!("{safe_title}.{ext}"));
    std::fs::write(&export_path, body).map_err(|e| {
        ImpForgeError::filesystem(
            "EXPORT_WRITE_FAILED",
            format!("Cannot write exported file: {e}"),
        )
    })?;

    log::info!(
        "ForgeWriter: exported '{}' as {} to {}",
        meta.title,
        ext,
        export_path.display()
    );

    Ok(export_path.to_string_lossy().to_string())
}

/// AI-assisted text operations using Ollama.
#[tauri::command]
pub async fn writer_ai_assist(
    document_id: String,
    selection: String,
    action: String,
) -> AppResult<String> {
    if selection.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_SELECTION",
            "Select some text to use AI assist",
        ));
    }

    let instruction = action_to_instruction(&action)?;

    log::info!(
        "ForgeWriter: AI assist '{}' on {} chars (doc {})",
        action,
        selection.len(),
        document_id
    );

    let result = ollama_text_assist(instruction, &selection, None).await?;

    Ok(result)
}

/// Compute word count statistics for arbitrary content.
#[tauri::command]
pub async fn writer_word_count(content: String) -> AppResult<WordCountStats> {
    Ok(compute_stats(&content))
}

/// Get the list of built-in document templates.
#[tauri::command]
pub async fn writer_get_templates() -> AppResult<Vec<DocumentTemplate>> {
    Ok(builtin_templates())
}

/// Create a new document from a template.
#[tauri::command]
pub async fn writer_create_from_template(
    template_id: String,
    title: String,
) -> AppResult<Document> {
    let templates = builtin_templates();
    let template = templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "TEMPLATE_NOT_FOUND",
                format!("Template '{}' not found", template_id),
            )
        })?;

    let dir = documents_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let content = template.content.clone();
    let wc = count_words(&content);

    let meta = MetaFile {
        id: id.clone(),
        title: title.clone(),
        format: template.format,
        word_count: wc,
        created_at: now.clone(),
        updated_at: now.clone(),
        tags: vec![template.category.clone()],
    };

    write_meta(&meta_path(&dir, &id), &meta)?;
    let cp = content_path(&dir, &id, template.format);
    std::fs::write(&cp, &content).map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot create document from template: {e}"),
        )
    })?;

    log::info!(
        "ForgeWriter: created '{}' from template '{}'",
        title,
        template.name
    );

    Ok(Document {
        id,
        title,
        content,
        format: template.format,
        word_count: wc,
        created_at: now.clone(),
        updated_at: now,
        tags: vec![template.category.clone()],
        auto_saved: false,
    })
}

/// Find and replace text within a document.
/// Returns the number of replacements made.
#[tauri::command]
pub async fn writer_find_replace(
    id: String,
    find: String,
    replace: String,
    replace_all: Option<bool>,
) -> AppResult<u32> {
    if find.is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_SEARCH",
            "Search text cannot be empty",
        ));
    }

    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let mut meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    let do_all = replace_all.unwrap_or(false);
    let (new_content, count) = if do_all {
        let count = content.matches(&find).count() as u32;
        (content.replace(&find, &replace), count)
    } else {
        // Replace only the first occurrence
        if let Some(pos) = content.find(&find) {
            let mut result = String::with_capacity(content.len());
            result.push_str(&content[..pos]);
            result.push_str(&replace);
            result.push_str(&content[pos + find.len()..]);
            (result, 1)
        } else {
            (content, 0)
        }
    };

    if count > 0 {
        meta.word_count = count_words(&new_content);
        meta.updated_at = now_iso();
        write_meta(&mp, &meta)?;
        std::fs::write(&cp, &new_content).map_err(|e| {
            ImpForgeError::filesystem(
                "CONTENT_WRITE_FAILED",
                format!("Cannot save after find/replace: {e}"),
            )
        })?;
    }

    log::info!(
        "ForgeWriter: find/replace in '{}': {} replacements",
        id,
        count
    );

    Ok(count)
}

/// Compute detailed document statistics for a document by ID.
#[tauri::command]
pub async fn writer_statistics(id: String) -> AppResult<DocumentStats> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    Ok(compute_document_stats(&content))
}

/// Generate a Markdown table of contents from headings in a document.
#[tauri::command]
pub async fn writer_generate_toc(id: String) -> AppResult<String> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    let mut toc = String::from("## Table of Contents\n\n");
    let mut found_any = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 || level > 6 {
            continue;
        }
        let heading_text = trimmed[level..].trim();
        if heading_text.is_empty() {
            continue;
        }

        found_any = true;

        // Create an anchor slug: lowercase, spaces to hyphens, strip non-alnum
        let slug: String = heading_text
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else if c == ' ' || c == '-' {
                    '-'
                } else {
                    ' '
                }
            })
            .filter(|c| *c != ' ')
            .collect();

        // Indent: level 1 = no indent, level 2 = 2 spaces, etc.
        let indent = "  ".repeat(level.saturating_sub(1));
        toc.push_str(&format!("{indent}- [{heading_text}](#{slug})\n"));
    }

    if !found_any {
        return Err(
            ImpForgeError::validation(
                "NO_HEADINGS",
                "No headings found in the document. Add Markdown headings (# Title) first.",
            )
        );
    }

    Ok(toc)
}

/// Save a named version snapshot of a document.
#[tauri::command]
pub async fn writer_save_version(id: String, message: String) -> AppResult<()> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    let ver_dir = versions_dir(&id)?;
    let version_id = Uuid::new_v4().to_string();
    let now = now_iso();

    let version = VersionFile {
        version_id: version_id.clone(),
        document_id: id.clone(),
        message: if message.is_empty() {
            format!("Version saved at {now}")
        } else {
            message
        },
        word_count: meta.word_count,
        content,
        created_at: now,
    };

    let vp = ver_dir.join(format!("{version_id}.json"));
    let json = serde_json::to_string_pretty(&version).map_err(|e| {
        ImpForgeError::internal("VERSION_SERIALIZE", format!("Cannot serialize version: {e}"))
    })?;
    std::fs::write(&vp, json).map_err(|e| {
        ImpForgeError::filesystem(
            "VERSION_WRITE_FAILED",
            format!("Cannot save version: {e}"),
        )
    })?;

    log::info!("ForgeWriter: saved version '{}' for doc '{}'", version_id, id);

    Ok(())
}

/// List all saved versions of a document, newest first.
#[tauri::command]
pub async fn writer_list_versions(id: String) -> AppResult<Vec<DocVersion>> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let ver_dir = versions_dir(&id)?;
    let mut versions: Vec<DocVersion> = Vec::new();

    let entries = std::fs::read_dir(&ver_dir).map_err(|e| {
        ImpForgeError::filesystem(
            "DIR_READ_FAILED",
            format!("Cannot read versions directory: {e}"),
        )
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.ends_with(".json") {
            continue;
        }
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Ok(ver) = serde_json::from_str::<VersionFile>(&data) {
            versions.push(DocVersion {
                version_id: ver.version_id,
                document_id: ver.document_id,
                message: ver.message,
                word_count: ver.word_count,
                created_at: ver.created_at,
            });
        }
    }

    versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(versions)
}

/// Restore a document to a specific saved version.
#[tauri::command]
pub async fn writer_restore_version(id: String, version_id: String) -> AppResult<Document> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let ver_dir = versions_dir(&id)?;
    let vp = ver_dir.join(format!("{version_id}.json"));

    if !vp.exists() {
        return Err(
            ImpForgeError::filesystem(
                "VERSION_NOT_FOUND",
                format!("Version '{version_id}' not found"),
            )
            .with_suggestion("Use writer_list_versions to see available versions."),
        );
    }

    let data = std::fs::read_to_string(&vp).map_err(|e| {
        ImpForgeError::filesystem(
            "VERSION_READ_FAILED",
            format!("Cannot read version file: {e}"),
        )
    })?;
    let ver: VersionFile = serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal(
            "VERSION_PARSE_FAILED",
            format!("Corrupt version file: {e}"),
        )
    })?;

    // Write the version content back as the current document
    let mut meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let now = now_iso();

    meta.word_count = count_words(&ver.content);
    meta.updated_at = now.clone();
    write_meta(&mp, &meta)?;

    std::fs::write(&cp, &ver.content).map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot restore version: {e}"),
        )
    })?;

    log::info!(
        "ForgeWriter: restored doc '{}' to version '{}'",
        id,
        version_id
    );

    Ok(Document {
        id: meta.id,
        title: meta.title,
        content: ver.content,
        format: meta.format,
        word_count: meta.word_count,
        created_at: meta.created_at,
        updated_at: now,
        tags: meta.tags,
        auto_saved: false,
    })
}

// ---------------------------------------------------------------------------
// Text Conversion Helpers
// ---------------------------------------------------------------------------

/// Very basic Markdown to HTML conversion (handles headings, bold, italic,
/// code blocks, lists, links, paragraphs). Not a full parser -- just enough
/// for clean export. A proper library (pulldown-cmark) could replace this
/// once added to Cargo.toml.
fn basic_md_to_html(md: &str) -> String {
    let mut html = String::with_capacity(md.len() * 2);
    let mut in_code_block = false;
    let mut in_list = false;
    let mut paragraph_buffer = String::new();

    for line in md.lines() {
        // Fenced code blocks
        if line.trim_start().starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                flush_paragraph(&mut html, &mut paragraph_buffer);
                html.push_str("<pre><code>");
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            html.push_str(&html_escape(line));
            html.push('\n');
            continue;
        }

        let trimmed = line.trim();

        // Empty line -- flush paragraph
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            flush_paragraph(&mut html, &mut paragraph_buffer);
            continue;
        }

        // Headings
        if let Some(heading) = parse_heading(trimmed) {
            flush_paragraph(&mut html, &mut paragraph_buffer);
            html.push_str(&heading);
            continue;
        }

        // Unordered lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            flush_paragraph(&mut html, &mut paragraph_buffer);
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            let item_text = inline_format(&trimmed[2..]);
            html.push_str(&format!("  <li>{item_text}</li>\n"));
            continue;
        }

        // Ordered lists
        if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some(pos) = trimmed.find(". ") {
                if trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                    flush_paragraph(&mut html, &mut paragraph_buffer);
                    if !in_list {
                        html.push_str("<ul>\n");
                        in_list = true;
                    }
                    let item_text = inline_format(&trimmed[pos + 2..]);
                    html.push_str(&format!("  <li>{item_text}</li>\n"));
                    continue;
                }
            }
        }

        // Close list if we were in one
        if in_list {
            html.push_str("</ul>\n");
            in_list = false;
        }

        // Regular text -- accumulate into paragraph
        if !paragraph_buffer.is_empty() {
            paragraph_buffer.push(' ');
        }
        paragraph_buffer.push_str(trimmed);
    }

    // Flush remaining
    if in_code_block {
        html.push_str("</code></pre>\n");
    }
    if in_list {
        html.push_str("</ul>\n");
    }
    flush_paragraph(&mut html, &mut paragraph_buffer);

    html
}

fn flush_paragraph(html: &mut String, buf: &mut String) {
    if buf.is_empty() {
        return;
    }
    let formatted = inline_format(buf);
    html.push_str(&format!("<p>{formatted}</p>\n"));
    buf.clear();
}

fn parse_heading(line: &str) -> Option<String> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let text = line[level..].trim();
    if text.is_empty() {
        return None;
    }
    Some(format!(
        "<h{level}>{}</h{level}>\n",
        inline_format(text)
    ))
}

/// Apply inline formatting: **bold**, *italic*, `code`, [links](url).
fn inline_format(text: &str) -> String {
    let mut result = html_escape(text);
    // Bold: **text**
    result = regex_replace_simple(&result, "\\*\\*(.+?)\\*\\*", "<strong>$1</strong>");
    // Italic: *text*
    result = regex_replace_simple(&result, "\\*(.+?)\\*", "<em>$1</em>");
    // Inline code: `text`
    result = regex_replace_simple(&result, "`(.+?)`", "<code>$1</code>");
    // Links: [text](url)
    result = regex_replace_simple(&result, "\\[(.+?)\\]\\((.+?)\\)", "<a href=\"$2\">$1</a>");
    result
}

/// Minimal regex-free pattern replacement for simple Markdown inline patterns.
/// This is intentionally simple -- it handles the most common cases without
/// pulling in the `regex` crate just for this module.
fn regex_replace_simple(text: &str, _pattern: &str, _replacement: &str) -> String {
    // For production, we do manual replacement of the four patterns above.
    let mut result = text.to_string();

    // Bold: **text** -> <strong>text</strong>
    if _pattern.contains("\\*\\*") {
        while let Some(start) = result.find("**") {
            if let Some(end) = result[start + 2..].find("**") {
                let inner = result[start + 2..start + 2 + end].to_string();
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
        return result;
    }

    // Italic: *text* -> <em>text</em>
    if _pattern.contains("\\*(.+?)\\*") && !_pattern.contains("\\*\\*") {
        while let Some(start) = result.find('*') {
            if let Some(end) = result[start + 1..].find('*') {
                let inner = result[start + 1..start + 1 + end].to_string();
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
        return result;
    }

    // Inline code: `text` -> <code>text</code>
    if _pattern.contains('`') {
        while let Some(start) = result.find('`') {
            if let Some(end) = result[start + 1..].find('`') {
                let inner = result[start + 1..start + 1 + end].to_string();
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
        return result;
    }

    // Links: [text](url) -> <a href="url">text</a>
    if _pattern.contains("\\[") {
        while let Some(bracket_start) = result.find('[') {
            if let Some(bracket_end) = result[bracket_start..].find("](") {
                let abs_bracket_end = bracket_start + bracket_end;
                if let Some(paren_end) = result[abs_bracket_end + 2..].find(')') {
                    let link_text =
                        result[bracket_start + 1..abs_bracket_end].to_string();
                    let url = result
                        [abs_bracket_end + 2..abs_bracket_end + 2 + paren_end]
                        .to_string();
                    result = format!(
                        "{}<a href=\"{}\">{}</a>{}",
                        &result[..bracket_start],
                        url,
                        link_text,
                        &result[abs_bracket_end + 2 + paren_end + 1..]
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        return result;
    }

    result
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Strip Markdown formatting for plain-text export.
fn strip_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut in_code_block = false;

    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let trimmed = line.trim();
        // Strip heading markers
        let text = trimmed.trim_start_matches('#').trim_start();
        // Strip bold/italic markers
        let text = text.replace("**", "").replace('*', "");
        // Strip inline code backticks
        let text = text.replace('`', "");

        result.push_str(&text);
        result.push('\n');
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_words_empty() {
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn test_count_words_normal() {
        assert_eq!(count_words("Hello world, this is a test."), 6);
    }

    #[test]
    fn test_count_words_multiline() {
        assert_eq!(count_words("Line one\nLine two\n\nLine three"), 6);
    }

    #[test]
    fn test_compute_stats() {
        let stats = compute_stats("Hello world. This is a test! How are you?");
        assert_eq!(stats.words, 9);
        assert_eq!(stats.sentences, 3);
        assert!(stats.reading_time_min > 0.0);
    }

    #[test]
    fn test_compute_stats_empty() {
        let stats = compute_stats("");
        assert_eq!(stats.words, 0);
        assert_eq!(stats.characters, 0);
        assert_eq!(stats.sentences, 0);
        assert_eq!(stats.paragraphs, 0);
        assert_eq!(stats.reading_time_min, 0.0);
    }

    #[test]
    fn test_doc_format_extension() {
        assert_eq!(DocFormat::Markdown.extension(), "md");
        assert_eq!(DocFormat::Html.extension(), "html");
        assert_eq!(DocFormat::PlainText.extension(), "txt");
    }

    #[test]
    fn test_doc_format_from_str() {
        assert_eq!(DocFormat::from_str_loose("markdown"), DocFormat::Markdown);
        assert_eq!(DocFormat::from_str_loose("MD"), DocFormat::Markdown);
        assert_eq!(DocFormat::from_str_loose("html"), DocFormat::Html);
        assert_eq!(DocFormat::from_str_loose("whatever"), DocFormat::PlainText);
    }

    #[test]
    fn test_action_to_instruction_valid() {
        assert!(action_to_instruction("improve").is_ok());
        assert!(action_to_instruction("shorten").is_ok());
        assert!(action_to_instruction("expand").is_ok());
        assert!(action_to_instruction("fix_grammar").is_ok());
        assert!(action_to_instruction("translate_en").is_ok());
        assert!(action_to_instruction("translate_de").is_ok());
        assert!(action_to_instruction("summarize").is_ok());
    }

    #[test]
    fn test_action_to_instruction_invalid() {
        let err = action_to_instruction("dance").unwrap_err();
        assert_eq!(err.code, "INVALID_ACTION");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), "&lt;script&gt;alert('xss')&lt;/script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_strip_markdown() {
        let md = "# Hello\n\n**Bold** text and *italic* text.\n\n```\ncode\n```\n";
        let plain = strip_markdown(md);
        assert!(plain.contains("Hello"));
        assert!(plain.contains("Bold text and italic text."));
        assert!(plain.contains("code"));
        assert!(!plain.contains('#'));
        assert!(!plain.contains("**"));
    }

    #[test]
    fn test_basic_md_to_html_heading() {
        let html = basic_md_to_html("# Title\n\nSome text.");
        assert!(html.contains("<h1>"));
        assert!(html.contains("Title"));
        assert!(html.contains("<p>"));
    }

    #[test]
    fn test_basic_md_to_html_list() {
        let html = basic_md_to_html("- Item one\n- Item two");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("Item one"));
    }

    #[test]
    fn test_basic_md_to_html_code_block() {
        let html = basic_md_to_html("```\nlet x = 1;\n```");
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("let x = 1;"));
    }

    #[test]
    fn test_builtin_templates_count() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 10);
        // Verify each template has a non-empty id, name, and content
        for t in &templates {
            assert!(!t.id.is_empty());
            assert!(!t.name.is_empty());
            assert!(!t.content.is_empty());
            assert!(!t.category.is_empty());
        }
    }

    #[test]
    fn test_builtin_templates_unique_ids() {
        let templates = builtin_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "template IDs must be unique");
    }

    #[test]
    fn test_document_stats_detailed() {
        let content = "# Hello World\n\nThis is a test. Another sentence here! And a question?\n\nSecond paragraph with more words.";
        let stats = compute_document_stats(content);
        assert!(stats.words > 0);
        assert!(stats.characters > 0);
        assert!(stats.characters_no_spaces < stats.characters);
        // 4 sentence-ending punctuation: "test." "here!" "question?" "words."
        assert_eq!(stats.sentences, 4);
        // 3 paragraphs: "# Hello World", sentence block, "Second paragraph..."
        assert_eq!(stats.paragraphs, 3);
        assert_eq!(stats.heading_count, 1);
        assert!(stats.reading_time_min > 0.0);
        assert!(stats.speaking_time_min > 0.0);
        assert!(!stats.reading_level.is_empty());
        assert!(stats.avg_sentence_length > 0.0);
    }

    #[test]
    fn test_document_stats_empty() {
        let stats = compute_document_stats("");
        assert_eq!(stats.words, 0);
        assert_eq!(stats.characters, 0);
        assert_eq!(stats.characters_no_spaces, 0);
        assert_eq!(stats.heading_count, 0);
        assert_eq!(stats.link_count, 0);
        assert!(stats.most_used_words.is_empty());
    }

    #[test]
    fn test_document_stats_word_frequency() {
        let content = "rust rust rust code code test rust code";
        let stats = compute_document_stats(content);
        assert!(!stats.most_used_words.is_empty());
        assert_eq!(stats.most_used_words[0].word, "rust");
        assert_eq!(stats.most_used_words[0].count, 4);
    }

    #[test]
    fn test_document_stats_links() {
        let content = "Visit [Google](https://google.com) and [GitHub](https://github.com).";
        let stats = compute_document_stats(content);
        assert_eq!(stats.link_count, 2);
    }
}
