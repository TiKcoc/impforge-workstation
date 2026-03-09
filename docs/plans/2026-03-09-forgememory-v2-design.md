# ForgeMemory v2 + ForgeWatch + ForgeSunshine — Design Document

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform ImpForge from a local-only AI workstation into a self-learning, context-aware, mobile-accessible AI brain that never forgets and can be controlled from any device.

**Date:** 2026-03-09
**Status:** Approved by User

---

## 1. Executive Summary

Three major feature streams upgrading ImpForge to Enterprise++++++:

| Feature | What It Does | Scientific Basis |
|---------|-------------|-----------------|
| **ForgeMemory v2 Auto-Learn** | Dual-mode intelligent extraction (NLP + LLM) from all user input | Mem0 (arXiv:2504.19413), Memoria (arXiv:2512.12686), A-MEM (arXiv:2502.12110) |
| **ForgeWatch** | Filesystem monitoring + document ingestion pipeline | Archivar pattern, inotify/kqueue, hierarchical chunking |
| **ForgeSunshine** | Moonlight remote access — control ImpForge from phone/tablet | Sunshine (LizardByte), NVIDIA GameStream protocol |
| **Chat ↔ Memory Bridge** | Every chat message enriched with context + auto-learned | Contextual Retrieval (Anthropic 2024) |
| **Universal Input Digest** | IDE, Terminal, Chat — everything digested into SQLite | MemGPT (Packer et al. 2023) |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        ImpForge Application                         │
│                                                                     │
│  ┌───────────┐  ┌────────────┐  ┌────────────┐  ┌──────────────┐  │
│  │  Chat UI   │  │  IDE/PTY   │  │  Settings  │  │ Mobile View  │  │
│  │  (Svelte)  │  │ (Terminal) │  │   (Svelte) │  │ (Moonlight)  │  │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └──────┬───────┘  │
│        │               │               │                │          │
│  ══════╪═══════════════╪═══════════════╪════════════════╪══════    │
│        │        Tauri Command Layer (invoke)             │          │
│  ══════╪═══════════════╪═══════════════╪════════════════╪══════    │
│        ▼               ▼               ▼                ▼          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    ForgeMemory Engine v2                      │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────┐ ┌───────────────┐  │  │
│  │  │ AutoLearn│ │ Context  │ │ InputDigest│ │ ForgeWatch    │  │  │
│  │  │ NLP Mode │ │ Builder  │ │ (all UIs) │ │ (filesystem)  │  │  │
│  │  │ LLM Mode │ │          │ │           │ │               │  │  │
│  │  └────┬─────┘ └────┬─────┘ └─────┬─────┘ └───────┬───────┘  │  │
│  │       │            │             │               │           │  │
│  │  ┌────▼────────────▼─────────────▼───────────────▼────────┐  │  │
│  │  │              Document Ingestion Pipeline                │  │  │
│  │  │  Detect Type → Chunk → Embed → Dedup → Store → Index  │  │  │
│  │  └────────────────────────┬───────────────────────────────┘  │  │
│  │                           │                                   │  │
│  │  ┌────────────────────────▼───────────────────────────────┐  │  │
│  │  │                    SQLite Storage                       │  │  │
│  │  │  memories | knowledge | kg_nodes | file_chunks | ...    │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────┐ ┌─────────┐ ┌────────────┐ ┌──────────────┐   │  │
│  │  │  HNSW   │ │  BM25   │ │ Knowledge  │ │    FSRS-5    │   │  │
│  │  │ Vector  │ │ FullText│ │   Graph    │ │  Scheduling  │   │  │
│  │  └─────────┘ └─────────┘ └────────────┘ └──────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   ForgeSunshine Manager                       │  │
│  │  Install → Configure → Start/Stop → Pair Devices → Monitor  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### Core Principles:

1. **Single SQLite File** — Everything in `~/.impforge/forge_memory.db`
2. **Offline-First** — NLP Pipeline needs zero internet. LLM Mode only needs local Ollama.
3. **Universal Input Capture** — Every keystroke in Chat, IDE, Terminal flows through Ingestion Pipeline
4. **Customer Choice** — NLP and LLM Mode separately toggleable in Settings
5. **Zero Data Loss** — Everything the user types is persistently stored and indexed
6. **Mobile Access** — Sunshine lifecycle management for "AI from your couch"

---

## 3. Auto-Learn v2 — Dual Mode Architecture

### Mode A: NLP Pipeline (Offline, ~2ms per message)

```
Message → Sentence Split → Pattern Matching (80+ DE/EN patterns)
       → Entity Extraction (regex: paths, URLs, IPs, emails, code refs)
       → Relation Detection ("X uses Y", "X depends on Y")
       → Importance Scoring (position + entity density + markers)
       → Dedup Check (embedding cosine similarity > 0.92 = skip)
       → Store in memories table + KG node/edge creation
```

**Pattern Categories (80+):**

| Category | Example Patterns (DE/EN) | Count |
|----------|-------------------------|-------|
| Preferences | "i prefer", "ich bevorzuge", "my favorite" | 15 |
| Decisions | "let's use", "wir nutzen", "we decided" | 12 |
| Explicit Notes | "remember:", "merke dir:", "important:" | 10 |
| Technical Facts | "runs on port", "version is", "API key" | 12 |
| Code Patterns | import/use/require statements, file paths | 10 |
| Negations | "don't use", "never", "avoid", "nicht verwenden" | 8 |
| Questions (implicit needs) | "how do I", "wie kann ich" | 8 |
| Corrections | "actually it's", "nein, das ist", "I meant" | 5 |

**Entity Types Extracted:**
- File paths (`/home/user/project/src/main.rs`)
- URLs (`https://api.example.com/v2`)
- Package names (`tokio`, `react`, `numpy`)
- Model names (`qwen2.5-coder:7b`, `gpt-4o`)
- IP addresses, ports
- Email addresses
- Code identifiers (function names, class names)

**Relation Types for KG:**
- `uses` — "I use Rust for backend"
- `depends_on` — "This requires tokio"
- `prefers_over` — "I prefer Rust over Python"
- `located_at` — "Config is at ~/.config/app"
- `version_is` — "Running Python 3.12"

### Mode B: LLM Extraction (Ollama, ~300ms GPU)

```
Message → Prompt Template → Local LLM → Structured JSON:
{
  "facts": [
    {"content": "User prefers dark mode", "importance": 0.8, "category": "preference"}
  ],
  "entities": [
    {"name": "Rust", "type": "language", "relations": ["preferred_by:user"]}
  ],
  "summary": "User discussed coding preferences",
  "sentiment": "positive"
}
→ JSON Validation → Dedup → Store + KG Update
```

**LLM Prompt Template:**
```
Extract structured information from this conversation turn.
Return JSON with: facts (content, importance 0-1, category),
entities (name, type, relations), a one-line summary, and sentiment.
Categories: preference, decision, fact, technical, correction, question.
Only extract genuinely important information. Skip small talk.

User: {user_message}
AI: {ai_response}
```

**Supported Models (customer choice):**
- `qwen2.5-coder:1.5b` (fastest, code-focused)
- `phi-3-mini` (balanced)
- `hermes-3:8b` (best quality, needs more RAM)
- Any Ollama model the user has installed

### Settings UI for Auto-Learn:

```
┌─ Auto-Learn Settings ───────────────────────────┐
│                                                   │
│  [x] NLP Pipeline (offline, instant)              │
│  [ ] LLM Extraction (requires Ollama)             │
│                                                   │
│  LLM Model: [qwen2.5-coder:1.5b ▼]              │
│  Extraction Sensitivity: [████░░░░░░] Medium      │
│                                                   │
│  Categories to Extract:                           │
│  [x] Preferences  [x] Decisions  [x] Facts       │
│  [x] Technical    [x] Code Patterns               │
│  [ ] Questions    [ ] Corrections                 │
│                                                   │
│  Dedup Threshold: [0.92]                          │
│  Max Memories per Day: [100]                      │
└───────────────────────────────────────────────────┘
```

---

## 4. Chat ↔ Memory Bridge

### The Critical Missing Piece

Currently `chat.rs` and `chat.svelte.ts` have ZERO ForgeMemory integration.

### Integration Points:

**Before AI call (context enrichment):**
```typescript
// In chat.svelte.ts sendMessage():
const context = await invoke('forge_memory_get_context', {
  query: userMessage,
  maxResults: 5
});
// Prepend to system prompt
const enrichedSystem = baseSystemPrompt + "\n\n" + context.system_supplement;
```

**After AI response (auto-learning):**
```typescript
// After streaming completes:
await invoke('forge_memory_auto_learn', {
  userMessage: content,
  aiResponse: assistantMessage.content,
  conversationId: activeConversationId
});
```

**Rust-side enhancement (`chat.rs`):**
- Accept optional `system_supplement` parameter
- Prepend memory context to the system prompt sent to OpenRouter/Ollama
- Return conversation_id for persistence tracking

---

## 5. Universal Input Digest

### Input Sources and Capture Methods:

| Source | Capture Point | What's Stored |
|--------|--------------|---------------|
| **Chat Messages** | `chat_stream` command | User msg + AI response + extracted facts |
| **IDE File Saves** | `ide_write_file` command | File path, diff summary, language |
| **Terminal Commands** | PTY output events | Command + output (filtered, max 1KB) |
| **Search Queries** | `forge_memory_search` | What user searched for (search_history table) |
| **File Opens** | `ide_read_file` command | File path + recent files list |
| **Git Operations** | `git_*` commands | Commits, branches, diffs |

### Filtering Rules (to avoid noise):
- Terminal output: only store commands, not full output (unless flagged)
- Repeated identical inputs within 60s: skip (dedup)
- System/binary output: skip
- Passwords/tokens: detect and NEVER store (regex: `password=`, `token=`, `api_key=`)

---

## 6. ForgeWatch — Filesystem Watcher

### Smart Auto-Discovery:

```
App Startup → Scan $HOME (max depth 3)
  → Detect:
    - Git repositories (.git directory present)
    - Document collections (>5 .md/.txt files in a dir)
    - Config directories (~/.config, ~/.ssh metadata only)
    - Project directories (Cargo.toml, package.json, pyproject.toml present)
  → Present discovery results in Settings UI
  → User confirms / adds / removes paths
  → Start filesystem watches on confirmed paths
```

### Watch Mechanisms (cross-platform):
- **Linux:** inotify via `notify` crate (IN_MODIFY, IN_CREATE, IN_DELETE)
- **macOS:** kqueue via `notify` crate (FSEvents)
- **Windows:** ReadDirectoryChangesW via `notify` crate

### Document Ingestion Pipeline:

| File Type | Chunking Strategy | Max Chunk Size | Embedding |
|-----------|------------------|----------------|-----------|
| `.rs/.ts/.py/.cs/.go` | By function/class (regex-based AST-like) | 2000 chars | fastembed |
| `.md/.txt/.rst` | By heading (## sections) | 1500 chars | fastembed |
| `.json/.yaml/.toml` | By top-level key | 1000 chars | fastembed |
| `.html/.css` | By tag/selector block | 1500 chars | fastembed |
| `.png/.jpg/.svg` | Metadata only (EXIF, filename, path) | N/A | metadata only |
| `.pdf` | Page-based (1 chunk per page) | 3000 chars | fastembed |
| Binaries, node_modules, .git | **SKIP** | — | — |

### SQLite Schema Addition:

```sql
CREATE TABLE IF NOT EXISTS watched_paths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    label TEXT,
    enabled INTEGER DEFAULT 1,
    scan_mode TEXT DEFAULT 'realtime' CHECK(scan_mode IN ('realtime','hourly','manual')),
    last_scanned TEXT,
    file_count INTEGER DEFAULT 0,
    total_chunks INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS file_index (
    id TEXT PRIMARY KEY,                -- sha256 of path
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    extension TEXT,
    size_bytes INTEGER,
    mtime TEXT,                          -- last modified time
    content_hash TEXT,                   -- sha256 of content (for change detection)
    chunk_count INTEGER DEFAULT 0,
    language TEXT,                        -- detected programming language
    watched_path_id INTEGER REFERENCES watched_paths(id),
    indexed_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS file_chunks (
    id TEXT PRIMARY KEY,                 -- uuid
    file_id TEXT NOT NULL REFERENCES file_index(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    chunk_type TEXT DEFAULT 'text',      -- text, function, class, heading, config_key
    start_line INTEGER,
    end_line INTEGER,
    embedding BLOB,                      -- f32 vector bytes
    created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_file_chunks_file ON file_chunks(file_id);
```

### Rate Limiting:
- Active use: max 10 files/second indexing
- Idle mode: burst up to 50 files/second
- Total index budget: configurable (default 500MB SQLite)
- Incremental: only re-index files where `content_hash` changed

### .forgeignore:
```
# Default .forgeignore (auto-created in watched paths)
node_modules/
.git/objects/
target/
__pycache__/
*.pyc
*.o
*.so
*.dll
*.exe
dist/
build/
.next/
.nuxt/
*.lock
```

### Settings UI:

```
┌─ ForgeWatch Settings ───────────────────────────┐
│                                                   │
│  Watched Paths:                                   │
│  [x] ~/Projects          342 files   1.2k chunks │
│  [x] ~/Documents         89 files    456 chunks  │
│  [ ] ~/.config           12 files    34 chunks   │
│  [+] Add Path...   [Auto-Discover]               │
│                                                   │
│  Scan Mode: [Realtime ▼]                         │
│  Index Size: 127 MB / 500 MB limit               │
│                                                   │
│  File Types:                                      │
│  [x] Code (.rs,.ts,.py,.cs,.go,.js)              │
│  [x] Docs (.md,.txt,.rst,.pdf)                   │
│  [x] Config (.json,.yaml,.toml)                  │
│  [ ] Images (.png,.jpg,.svg)                     │
│                                                   │
│  [Reindex All]  [Clear Index]  [Export Config]   │
└───────────────────────────────────────────────────┘
```

---

## 7. ForgeSunshine — Moonlight Remote Access

### Lifecycle Management:

```
ImpForge Settings → "Remote Access" Tab
  → detect_sunshine() — check if installed
  → If not: install_sunshine() — platform-specific package manager
  → configure_sunshine() — resolution, bitrate, encoder
  → start_sunshine() — spawn as child process
  → pair_device() — generate PIN, show QR code
  → monitor_sunshine() — connection status, clients, bandwidth
  → stop_sunshine() — graceful shutdown
```

### Tauri Commands:

```rust
#[tauri::command] async fn sunshine_detect() -> Result<SunshineStatus, String>
#[tauri::command] async fn sunshine_install() -> Result<bool, String>
#[tauri::command] async fn sunshine_configure(config: SunshineConfig) -> Result<(), String>
#[tauri::command] async fn sunshine_start() -> Result<(), String>
#[tauri::command] async fn sunshine_stop() -> Result<(), String>
#[tauri::command] async fn sunshine_pair() -> Result<String, String>  // returns PIN
#[tauri::command] async fn sunshine_status() -> Result<SunshineInfo, String>
```

### SunshineConfig:

```rust
pub struct SunshineConfig {
    pub resolution: String,      // "1920x1080", "2560x1440", etc.
    pub fps: u32,                // 30, 60, 120
    pub bitrate: u32,            // kbps (default 10000)
    pub encoder: String,         // "vaapi" (AMD), "nvenc" (NVIDIA), "software"
    pub audio: bool,             // stream audio
    pub auto_start: bool,        // start with ImpForge
}
```

### Platform Install Commands:

| OS | Install Command | Package |
|----|----------------|---------|
| Ubuntu/PopOS | `sudo apt install sunshine` | sunshine deb |
| Arch | `sudo pacman -S sunshine` | AUR/community |
| Fedora | `sudo dnf install sunshine` | RPM |
| Windows | `winget install LizardByte.Sunshine` | winget |
| macOS | `brew install sunshine` | Homebrew |

### Settings UI:

```
┌─ Remote Access (Moonlight) ─────────────────────┐
│                                                   │
│  Status: ● Running (2 clients connected)         │
│                                                   │
│  Resolution: [1920x1080 ▼]  FPS: [60 ▼]         │
│  Bitrate: [████████░░] 10 Mbps                   │
│  Encoder: [AMD VAAPI ▼]                          │
│  [x] Stream Audio    [x] Auto-start              │
│                                                   │
│  Connected Devices:                               │
│  📱 Pixel 8 Pro — 32ms latency, 8.2 Mbps        │
│  📱 iPad Air — 28ms latency, 9.1 Mbps            │
│                                                   │
│  [Pair New Device]  PIN: 4729                    │
│                                                   │
│  Moonlight Client Downloads:                      │
│  Android: Google Play | iOS: App Store            │
│  Steam Deck: Flatpak | PC: moonlight-stream.org  │
│                                                   │
│  [Stop Streaming]  [Open Sunshine Web UI]        │
└───────────────────────────────────────────────────┘
```

### Security:
- PIN-based pairing (Sunshine standard, encrypted)
- Local network only by default
- Optional: Hint for Tailscale/WireGuard for remote access
- HTTPS for Sunshine web UI (localhost:47990)
- No credentials stored in ImpForge (Sunshine manages its own auth)

---

## 8. Data Flow Summary

```
USER INPUT (any source)
    │
    ├─ Chat Message ──────────────────┐
    ├─ IDE File Edit ─────────────────┤
    ├─ Terminal Command ──────────────┤
    ├─ Search Query ──────────────────┤
    │                                 ▼
    │                    ┌─────────────────────┐
    │                    │   Input Digest       │
    │                    │   (capture + filter) │
    │                    └──────────┬──────────┘
    │                               │
    │                    ┌──────────▼──────────┐
    │                    │   Auto-Learn v2      │
    │                    │   NLP ─── and/or ──  │
    │                    │   LLM (if enabled)   │
    │                    └──────────┬──────────┘
    │                               │
    │                    ┌──────────▼──────────┐
    │                    │  Ingestion Pipeline  │
    │                    │  Chunk → Embed →     │
    │                    │  Dedup → Store →     │
    │                    │  Index (HNSW+BM25)   │
    │                    └──────────┬──────────┘
    │                               │
    │                    ┌──────────▼──────────┐
    │                    │     SQLite DB        │
    │                    │  memories + chunks + │
    │                    │  kg_nodes + file_idx │
    │                    └──────────┬──────────┘
    │                               │
    ▼                               ▼
NEXT AI QUERY ────────► Context Builder
                        (core memories +
                         relevant search +
                         KG neighborhood)
                              │
                              ▼
                        ENRICHED SYSTEM PROMPT
                        → sent to AI model
```

---

## 9. Dependencies (New)

### Rust (Cargo.toml additions):
```toml
# Filesystem watching
notify = "7"              # Cross-platform fs events (inotify/kqueue/ReadDirectoryChanges)

# PDF text extraction (optional)
# pdf-extract = "0.7"    # Only if PDF support enabled
```

### Frontend (package.json additions):
- No new npm dependencies needed (existing UI components sufficient)

### External (optional):
- Sunshine (installed by ForgeSunshine manager, not bundled)
- Ollama (for LLM Mode auto-learn, already supported)

---

## 10. Testing Strategy

| Feature | Test Type | Coverage Target |
|---------|-----------|----------------|
| NLP Pipeline patterns | Unit tests (80+ patterns × 2 languages) | 95% |
| LLM extraction | Integration test with mock Ollama | 80% |
| Dedup detection | Unit tests (similar/different pairs) | 90% |
| Chat bridge | Integration test (mock chat flow) | 85% |
| Input digest | Unit tests per source type | 90% |
| ForgeWatch | Integration test with temp directory | 85% |
| Ingestion pipeline | Unit tests per file type chunker | 90% |
| ForgeSunshine | Integration test (detect/configure) | 70% |
| Settings persistence | Unit tests | 95% |

---

## 11. Scientific References

1. **Mem0** — Chhikara et al. (arXiv:2504.19413, 2025). Production-ready agent memory with graph extension. 26% improvement over OpenAI.
2. **Memoria** — (arXiv:2512.12686, 2025). Modular memory framework with weighted KG user modeling.
3. **A-MEM** — Xu et al. (arXiv:2502.12110, 2025). Zettelkasten-inspired agentic memory with dynamic linking.
4. **MemGPT** — Packer et al. (arXiv:2310.08560, 2023). LLMs as Operating Systems with tiered memory.
5. **Contextual Retrieval** — Anthropic (2024). Prepending relevant context to AI prompts.
6. **FSRS-5** — Ye (2022-2024). Free Spaced Repetition Scheduler for memory scheduling.
7. **HNSW** — Malkov & Yashunin (2018). Hierarchical Navigable Small World graphs.
8. **BM25** — Robertson & Zaragoza (2009). Probabilistic relevance framework.
9. **RRF** — Cormack et al. (2009). Reciprocal Rank Fusion for hybrid search.
10. **MemR3** — (arXiv:2512.20237, 2025). Memory retrieval via reflective reasoning.
11. **Sunshine** — LizardByte. Open-source game streaming host for Moonlight.

---

## 12. User Decisions (Confirmed)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Moonlight approach | Option 1: ImpForge manages Sunshine directly | "It just works" experience |
| Filesystem scanning | Option 2: Smart Auto-Discovery + manual add/remove | Privacy-respecting, DSGVO-konform |
| Auto-Learn modes | Both NLP + LLM as separate toggles | Customer choice, graceful degradation |
| Storage architecture | SQLite only (no RLM dependency) | 100% standalone, portable |
| Ingestion pipeline | Unified pipeline for all input sources | Single code path, consistent quality |
