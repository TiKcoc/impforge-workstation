# ImpForge Adaptive Platform — Complete Design Document

> **Version**: 1.0 | **Date**: 2026-03-15
> **Vision**: Die erste vollständig adaptive, offline-fähige, persönliche KI-Workstation der Welt
> **Motto**: "Deine KI. Dein Rechner. Deine Daten. Dein Interface."

---

## 1. RECHTLICHE GRUNDLAGEN

### 1.1 Embedding von Drittanbieter-Inhalten (EU-Recht)

**EuGH-Entscheidung (Bestwater International)**: Embedding via iframe/WebView ist KEINE Urheberrechtsverletzung,
solange der Inhalt bereits öffentlich zugänglich ist und kein neues Publikum erreicht wird.

**ABER Einschränkung (VG Bild-Kunst vs Stiftung Preußischer Kulturbesitz, C-392/19)**:
Wenn der Rechteinhaber Anti-Framing-Maßnahmen implementiert hat, darf NICHT eingebettet werden.

**ImpForge-Lösung**: Wir nutzen das **Steam-Modell** — kein Embedding fremder Inhalte, sondern:
- **Launcher-Pattern**: User öffnet seine eigene Software/Website in einem integrierten WebView
- **User-initiated**: Der User entscheidet selbst was er integriert (wie Steam Non-Steam Games)
- **Keine Redistribution**: Wir liefern keine Drittanbieter-Software mit
- **Datenschutz**: Alle Daten bleiben lokal, kein Tracking, kein Proxy

### 1.2 WireGuard VPN Integration

**Lizenz**: WireGuard Kernel-Module = GPLv2, userspace Implementierungen = MIT/Apache-2.0/BSD
**Trademark**: "WireGuard" ist eine eingetragene Marke — darf NICHT in Marketing verwendet werden ohne Erlaubnis
**ImpForge-Lösung**:
- Nutze `boringtun` (Cloudflare, BSD-3-Clause) — userspace WireGuard in Rust, KEINE GPL-Abhängigkeit
- Bezeichnung im UI: "Secure Tunnel" oder "Private Network" — NICHT "WireGuard"
- User konfiguriert seinen eigenen VPN-Server (ImpForge stellt nur den Client)

### 1.3 MCP Server Integration

**Lizenz**: MCP = MIT (Anthropic Open Standard)
**Status 2026**: 10,000+ aktive MCP Server, 97M monatliche SDK-Downloads
**ImpForge-Lösung**: Eigener MCP Client in Rust, User installiert gewünschte MCP Server selbst

### 1.4 Externe Programme/Software

**Rechtliches Modell (wie Steam):**
- ImpForge startet externe Programme über System-Prozesse (kein Embedding des Codes)
- WebView für Websites = Browser-Äquivalent (rechtlich wie Chrome/Firefox)
- Keine Redistribution, keine Modifikation fremder Software
- User muss eigene Lizenzen für Drittanbieter-Software besitzen

**Quellen:**
- [EU Embedding Ruling](https://policyreview.info/articles/news/eu-ruling-embedding-does-not-equal-copyright-infringement/337)
- [WireGuard Trademark Policy](https://www.wireguard.com/trademark-policy/)
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [EU Product Liability Directive 2024/2853](https://www.reedsmith.com/our-insights/blogs/viewpoints/102lyiv/2026-update-eu-regulations-for-tech-and-online-businesses/)

---

## 2. ADAPTIVE PLATFORM ARCHITEKTUR

### 2.1 User-Profiling bei Ersteinrichtung (OnboardGPT-Pattern)

**Wissenschaftliche Basis:**
- arXiv:2412.16837 — "Adaptive UI Generation Through Reinforcement Learning"
- arXiv:2602.03154 — "Intelligent Front-End Personalization: AI-Driven UI Adaptation"
- arXiv:2504.20782 — "Integrating Human Feedback into RL for Adaptive UIs"
- MDPI — "OnboardGPT v1.0: Conversational AI for Personalized Onboarding"

**Statistiken (2026):**
- 40% mehr Umsatz durch AI-Personalisierung (McKinsey)
- 30% bessere Feature-Adoption durch adaptive Interfaces (Gartner)
- 50% bessere Retention durch verhaltensbasiertes Onboarding

**Onboarding Flow (5 Fragen → Profil → Adaptives UI):**

```
Frage 1: "Was beschreibt dich am besten?"
  → Developer | Designer | Manager | Freelancer | Student | Unternehmer | Office User

Frage 2: "Wie erfahren bist du mit KI?"
  → Anfänger | Fortgeschritten | Experte

Frage 3: "Was möchtest du hauptsächlich tun?"
  → Code schreiben | Texte/Docs erstellen | Business automatisieren |
    Social Media managen | Alles zusammen

Frage 4: "Welche Tools nutzt du bereits?"
  → [Checkboxen: Docker, GitHub, VS Code, Office, Slack, n8n, etc.]

Frage 5: "Arbeitest du alleine oder im Team?"
  → Solo | Team (2-10) | Enterprise (10+)
```

**Resultierende Profile:**

| Profil | Sichtbare Module | Versteckte Module |
|--------|-----------------|-------------------|
| **Developer** | IDE, Terminal, Git, Docker, Agents, Debug | Office, Social Media |
| **Office User** | Docs, Chat, Browser, Calendar | IDE, Terminal, Docker, Debug |
| **Freelancer** | Chat, Fiverr, Upwork, Portfolio, Invoicing | Docker, Debug, Metrics |
| **Manager** | Dashboard, Team, Reports, Calendar, CRM | IDE, Terminal, Docker |
| **Marketing** | Social Media, Analytics, Content Creator | IDE, Docker, Debug |
| **Unternehmer** | Dashboard, Finance, Marketing, Team, CRM | Debug, Metrics-Detail |
| **Student** | Chat, Docs, Research, IDE-Light | Docker, Team, CRM |
| **Custom** | Alles (User wählt selbst) | Nichts |

**Technische Umsetzung:**
- `UserProfile` struct in Rust mit Serialisierung zu SQLite
- `ModuleVisibility` Map: `module_id → visible: bool`
- Svelte 5 `$derived` reactive Filterung der Navigation
- Profile jederzeit in Settings änderbar
- AI lernt mit: häufig genutzte Module steigen automatisch in Priorität

### 2.2 Modulares App-Launcher System (Steam-Pattern)

**Konzept**: User kann JEDE externe Software/Website als "App" in ImpForge integrieren

**App-Typen:**

| Typ | Rendering | Beispiele |
|-----|-----------|-----------|
| **Native App** | System-Prozess starten | VS Code, Blender, LibreOffice |
| **Web App** | Integrierter WebView | Google Docs, Figma, Notion, Trello |
| **Web Service** | API-Integration + UI | GitHub, Docker Hub, Slack, Discord |
| **MCP Tool** | MCP Protocol | Datenbanken, File Systems, APIs |
| **ImpForge Module** | Native Svelte Component | IDE, Chat, Agents, etc. |

**Datenmodell:**

```rust
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub icon: Option<PathBuf>,        // Custom Icon
    pub app_type: AppType,
    pub launch_config: LaunchConfig,
    pub category: String,             // User-defined
    pub pinned: bool,
    pub last_used: Option<DateTime>,
    pub usage_count: u64,
    pub automation_rules: Vec<AutomationRule>,
}

pub enum AppType {
    NativeProcess { executable: PathBuf, args: Vec<String> },
    WebView { url: String, inject_css: Option<String> },
    WebService { api_url: String, auth: AuthConfig },
    McpServer { transport: McpTransport, capabilities: Vec<String> },
    ImpForgeModule { module_id: String },
}

pub struct AutomationRule {
    pub trigger: Trigger,             // OnOpen, OnClose, Cron, Event
    pub actions: Vec<Action>,         // LLM Query, API Call, Script
    pub enabled: bool,
}
```

**UI-Konzept:**
- App Library (wie Steam Library) — Grid/List View mit Kategorien
- Quick Launch Bar (wie macOS Dock) — pinned Apps
- App Store (Phase 4) — Community Plugins/Integrations
- Drag & Drop zum Hinzufügen von .desktop Dateien (Linux), .lnk (Windows), .app (macOS)

### 2.3 AI-Gesteuerte Automatisierung

**Flow: User → Intent → LLM → MCP → Aktion**

```
User: "Poste meinen letzten Blog-Artikel auf LinkedIn und Twitter"
  ↓
ImpForge Router: Erkennt → Social Media Task
  ↓
LLM (Ollama lokal): Generiert Post-Texte, passt pro Plattform an
  ↓
MCP Actions:
  → LinkedIn MCP: Post erstellen mit Bild + Hashtags
  → Twitter/X MCP: Thread erstellen, kürzer, mit Link
  ↓
User: Bestätigt oder editiert → Veröffentlicht
```

**Automation Engine:**
- Workflow Builder (visual, wie n8n aber simpler)
- Trigger-basiert: Cron, Event, Manual, AI-suggested
- Jede App kann Automations-Regeln haben
- LLM lernt User-Patterns und schlägt Automationen vor

### 2.4 Secure Tunnel (VPN) für Teams

**Technologie**: `boringtun` (Cloudflare, BSD-3-Clause) — WireGuard-kompatibel in Rust

**Auto-Setup Flow:**
```
1. Team-Leader klickt "Team erstellen"
2. ImpForge generiert Schlüsselpaar (Curve25519)
3. ImpForge konfiguriert VPN-Server (lokal oder Cloud)
4. Einladungslink/QR-Code wird generiert
5. Team-Mitglieder scannen/klicken → Auto-Konfiguration
6. Ende-zu-Ende verschlüsselt, P2P wenn möglich
```

**Features:**
- Zero-Config für Team-Mitglieder
- File Sharing über Tunnel (verschlüsselt)
- Shared Workspaces (CRDT-sync über Tunnel)
- Team-Chat (verschlüsselt, kein Cloud-Server)
- Team-Dashboard mit Online-Status

---

## 3. PHASEN-PLAN (VOLLSTÄNDIG)

### Phase 1: AI Workstation für Developer (AKTUELL — 90% fertig)
- [x] Multi-Model Chat (Ollama + OpenRouter) ✅
- [x] CodeForge IDE (15 Panels, LSP, Git, Debug) ✅
- [x] NeuralSwarm Agents (42 Workers, Hebbian Trust) ✅
- [x] Docker + GitHub + n8n Integration ✅
- [x] BenikUI Style Engine ✅
- [x] ForgeMemory (BM25 + Semantic + KG) ✅
- [ ] Adaptive Onboarding (User-Profiling) ← NÄCHSTER SCHRITT
- [ ] App Launcher System (Steam-Pattern)
- [ ] Cross-Platform Builds (Linux + Windows + macOS)

### Phase 2: Universal Business Automation
- [ ] Social Media Hub (LinkedIn, X, Instagram, TikTok, Facebook)
- [ ] Freelancer Tools (Fiverr, Upwork — Profile, Gigs, Proposals)
- [ ] Workflow Automation Engine (Visual Builder)
- [ ] MCP Server Marketplace (Install mit 1-Click)
- [ ] CRM Module (Kontakte, Deals, Pipeline)
- [ ] Calendar/Scheduling Integration
- [ ] Email Client (IMAP/SMTP, AI-Compose)
- [ ] Invoice/Billing Module

### Phase 3: Office Revolution
- [ ] ForgeWriter (Word-Ersatz mit AI-Assist)
- [ ] ForgeSheets (Excel-Ersatz mit AI-Analyse)
- [ ] ForgeSlides (PowerPoint-Ersatz mit AI-Design)
- [ ] ForgePDF (Acrobat-Ersatz mit AI-Extraktion)
- [ ] ForgeNotes (Notion-Ersatz mit Knowledge Graph)
- [ ] ForgeDraw (Whiteboard mit AI-Diagramme)

### Phase 4: Enterprise & Team Platform
- [ ] Secure Tunnel (boringtun VPN, Zero-Config)
- [ ] Team Workspaces (CRDT-sync)
- [ ] Admin Dashboard (User Management, Audit Logs)
- [ ] On-Premise Deployment Option
- [ ] Plugin Marketplace (Community + Verified)
- [ ] White-Label für Partner
- [ ] Custom Model Training in-App
- [ ] SSO/SAML Integration

---

## 4. TECHNISCHE ARCHITEKTUR (ERWEITERT)

### 4.1 Module Registry System

```rust
pub struct ModuleDefinition {
    pub id: String,                    // "social-media-hub"
    pub name: String,                  // "Social Media Hub"
    pub description: String,
    pub icon: String,                  // Lucide icon name
    pub category: ModuleCategory,
    pub phase: u8,                     // 1-4
    pub requires_profile: Vec<UserProfileType>,
    pub components: Vec<ComponentDef>,
    pub tauri_commands: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub settings_schema: serde_json::Value,
    pub enabled: bool,
}

pub enum ModuleCategory {
    Core,           // Chat, Settings, Dashboard
    Development,    // IDE, Terminal, Git, Docker
    Productivity,   // Docs, Sheets, Calendar
    Business,       // CRM, Invoice, Freelancer
    Social,         // Social Media, Marketing
    Team,           // VPN, Collab, Admin
    Custom,         // User-installed Apps
}
```

### 4.2 Adaptive UI Engine

```
User Profile → Module Filter → Layout Generator → BenikUI Renderer
     ↓              ↓                ↓                    ↓
  SQLite       Visibility Map    Grid/Panel Config    CSS Variables
     ↓              ↓                ↓                    ↓
  Settings     Navigation         Layout              Themed UI
```

**RL-basierte Anpassung (arXiv:2412.16837):**
- Track: Welche Module der User öffnet, wie lange, wie oft
- Learn: Reinforcement Learning Agent optimiert Layout
- Suggest: "Du nutzt Docker oft nach Git — soll ich die nebeneinander legen?"
- Adapt: Interface reorganisiert sich nach Nutzungsmustern

### 4.3 MCP Integration Layer

```rust
pub struct McpManager {
    pub servers: HashMap<String, McpServerConnection>,
    pub registry: McpServerRegistry,       // Available servers
    pub auto_discovery: bool,              // Scan for local MCP servers
}

pub struct McpServerConnection {
    pub transport: McpTransport,           // Stdio, HTTP, SSE
    pub capabilities: Vec<McpCapability>,  // Tools, Resources, Prompts
    pub health: HealthStatus,
    pub auto_restart: bool,
}
```

---

## 5. UI/INTERFACE KONZEPT

### 5.1 Adaptive Navigation

**Anfänger-Mode:**
```
┌──────────────────────────────────────────────┐
│  🏠 Home  │  💬 Chat  │  📝 Docs  │  ⚙ Settings │
└──────────────────────────────────────────────┘
  Nur 4 Buttons, große Icons, Tooltips
```

**Experten-Mode:**
```
┌──────────────────────────────────────────────┐
│ 🏠│💬│📝│💻│🐳│📦│🤖│📊│🌐│📰│🔧│⚡│🔒│📱│+│
└──────────────────────────────────────────────┘
  Alle Module, kompakte Icons, Keyboard Shortcuts
```

**Auto-Adapt Mode:**
- Navigation zeigt die 5 meistgenutzten Module
- Overflow-Menu "Mehr..." für selten genutzte
- AI schlägt vor: "Du hast GitHub 3 Tage nicht geöffnet — ausblenden?"

### 5.2 App Library (Steam-Pattern)

```
┌─ App Library ──────────────────────────────────┐
│ 🔍 Suchen...                    [+ App hinzufügen] │
│                                                    │
│ ★ FAVORITEN                                       │
│   [VS Code]  [Chrome]  [Slack]  [Notion]          │
│                                                    │
│ 💻 DEVELOPMENT                                    │
│   [Docker Desktop] [Postman] [DBeaver] [GitKraken]│
│                                                    │
│ 📄 OFFICE                                         │
│   [Google Docs] [Excel Online] [PDF Editor]       │
│                                                    │
│ 🌐 WEB SERVICES                                   │
│   [GitHub] [Fiverr] [Upwork] [LinkedIn]           │
│                                                    │
│ 🔌 MCP SERVERS                                    │
│   [PostgreSQL] [Redis] [Slack MCP] [GitHub MCP]   │
└────────────────────────────────────────────────────┘
```

### 5.3 BenikUI Anpassung

Jede Komponente hat:
- **Container**: Hintergrund, Border, Padding, Radius
- **Header**: Titel-Font, Farbe, Icon
- **Content**: Text-Farbe, Font-Size, Spacing
- **Footer**: Actions, Status-Bar

User kann:
- Drag & Drop Layout ändern
- Farben per Color Picker anpassen
- Themes importieren/exportieren (VS Code kompatibel)
- Profile speichern/laden (wie ElvUI in WoW)

---

## 6. WISSENSCHAFTLICHE REFERENZEN

| Paper | Relevanz | Anwendung in ImpForge |
|-------|----------|----------------------|
| arXiv:2412.16837 | Adaptive UI via RL | User Profile → Interface Adaptation |
| arXiv:2602.03154 | AI-Driven UI Personalization | Module Visibility, Layout Optimization |
| arXiv:2504.20782 | Human Feedback in RL for UIs | User korrigiert AI-Vorschläge |
| arXiv:2601.16596 | Mixture-of-Agents | Multi-Model Consensus (Convergence) |
| arXiv:2602.06039 | Agent Topology Discovery | NeuralSwarm Worker Organization |
| arXiv:2504.05341 | Three-Factor Hebbian Trust | Worker Trust Scoring |
| Bi & Poo 1998 | STDP Synaptic Plasticity | Trust Decay/Potentiation |
| Kephart & Chess 2003 | MAPE-K Self-Healing | Autonomous Service Recovery |
| McClelland et al. 1995 | CLS Memory Consolidation | Fast→Slow Memory in ForgeMemory |
| Robertson/Zaragoza 2009 | BM25 | ForgeMemory Text Search |
| Ye et al. 2024 | FSRS-5 | Spaced Repetition Scheduling |
| OnboardGPT (MDPI) | Conversational AI Onboarding | Adaptive Ersteinrichtung |

---

## 7. KOMMERZIELLES KONZEPT

### Warum ImpForge den Markt erobern wird:

1. **Einziger Anbieter** der Chat + IDE + Agents + Office + Social Media + CRM in EINER App vereint
2. **100% Offline-fähig** — kein Cloud-Lock-in, kein Datenschutz-Risiko
3. **DSGVO-konform by Design** — Daten verlassen nie den Rechner
4. **Adaptive UI** — passt sich dem User an, nicht umgekehrt
5. **BenikUI** — 100% anpassbar (kein Produkt bietet das)
6. **Native Performance** — Tauri statt Electron (10x kleiner, 3x schneller)
7. **Wissenschaftlich fundiert** — Neuroscience-basierte Agenten, RL-basierte UI
8. **Cross-Platform** — Linux, Windows, macOS mit einem Codebase
9. **Ab €0 nutzbar** — Free Tier mit lokalen Models, keine Kreditkarte nötig
10. **EU AI Act konform** — Transparenz, Datenschutz, Audit Logs

### Zielgruppe (TAM):

| Segment | Größe | ImpForge-Fit |
|---------|-------|-------------|
| Freelancer/Solo-Developer | 50M+ weltweit | Perfekt — All-in-One |
| KMU (1-50 Mitarbeiter) | 400M+ Unternehmen | Team-Features, Office |
| Enterprise | 100K+ Unternehmen | On-Premise, SSO, Audit |
| Studenten | 200M+ weltweit | Free Tier, Lern-Mode |
