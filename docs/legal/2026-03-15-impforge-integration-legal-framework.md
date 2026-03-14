# ImpForge Integration Legal Framework

> **Version**: 1.0 | **Date**: 2026-03-15
> **Purpose**: Rechtliche Grundlagen für die Integration von Drittanbieter-Software, Websites und Services in ImpForge
> **Disclaimer**: Dies ist Rechtsrecherche, keine Rechtsberatung. Vor kommerziellem Launch qualifizierten Rechtsbeistand konsultieren.

---

## 1. EMBEDDING VON DRITTANBIETER-WEBSITES (WebView)

### EU-Rechtslage

**EuGH-Entscheidung C-348/13 (BestWater International, 2014):**
- Embedding via iframe/WebView ist KEINE Urheberrechtsverletzung
- Voraussetzung: Inhalt ist bereits öffentlich zugänglich
- Kein "neues Publikum" wird erreicht (Art. 3 InfoSoc-Richtlinie 2001/29/EG)

**EuGH-Entscheidung C-392/19 (VG Bild-Kunst vs Stiftung Preußischer Kulturbesitz, 2021):**
- EINSCHRÄNKUNG: Wenn Rechteinhaber Anti-Framing-Maßnahmen implementiert hat
  (z.B. X-Frame-Options Header, CSP frame-ancestors), darf NICHT eingebettet werden
- Das Umgehen solcher Maßnahmen = Urheberrechtsverletzung

**Quellen:**
- [EU Embedding Ruling](https://policyreview.info/articles/news/eu-ruling-embedding-does-not-equal-copyright-infringement/337)
- [KPW Law: Embedding](https://kpw.law/en/embedding-third-party-content-on-the-web-is-not-a-copyright-infringement/)
- [Wiggin LLP: CJEU Framing](https://www.wiggin.co.uk/insight/court-of-justice-of-european-union-finds-that-the-embedding-of-a-copyright-work-in-a-third-party-web-page-by-means-of-framing-constitutes-making-that-work-available-to-a-new-public-if-the-copyright-ho/)

### ImpForge-Implementierung

ImpForge verwendet das **Steam-Launcher-Modell**:
1. **Kein Embedding fremder Inhalte** — User öffnet seine eigenen Accounts/Software
2. **WebView = Browser-Äquivalent** — rechtlich wie Chrome/Firefox
3. **User-initiated** — der User entscheidet selbst was er öffnet
4. **Keine Redistribution** — keine Drittanbieter-Software wird mitgeliefert
5. **Respektierung von X-Frame-Options** — wenn eine Website Framing verbietet, wird sie im externen Browser geöffnet

### Technische Safeguards
- Prüfung von `X-Frame-Options` und `Content-Security-Policy: frame-ancestors` Headers
- Bei Blockierung: Automatischer Fallback auf externen Browser
- User-Consent vor erstmaligem Öffnen einer externen Website
- Kein Caching/Speicherung von Drittanbieter-Inhalten

---

## 2. LAUNCHER FÜR EXTERNE SOFTWARE

### Rechtliches Modell

ImpForge startet externe Programme als **System-Prozesse** — identisch mit:
- Windows Explorer → Doppelklick auf .exe
- macOS Finder → Öffne mit...
- Linux File Manager → xdg-open

**Keine rechtlichen Bedenken**, da:
- Kein Code der Drittanbieter-Software wird modifiziert
- Kein Code wird in ImpForge eingebettet oder redistributed
- User muss eigene Lizenzen für die Software besitzen
- ImpForge fungiert nur als Launcher/Organizer

### Steam-Präzedenz
Steam erlaubt seit Jahren das Hinzufügen von "Non-Steam Games" — jede beliebige .exe kann in die Steam Library aufgenommen werden. Dieses Modell ist rechtlich unbedenklich und industriell etabliert.

---

## 3. VPN/SECURE TUNNEL

### WireGuard-Protokoll

**Kernel-Implementierung**: GPLv2 — NICHT für kommerzielle Einbettung geeignet
**Userspace-Implementierungen**:

| Implementierung | Lizenz | Kommerziell nutzbar? |
|----------------|--------|---------------------|
| `boringtun` (Cloudflare) | BSD-3-Clause | JA |
| `wireguard-go` | MIT | JA |
| `wireguard-nt` (Windows) | GPLv2 | NEIN (als Library) |

**ImpForge-Entscheidung**: `boringtun` (BSD-3-Clause)
- Pure Rust, kein C-Dependency
- Cross-Platform (Linux, macOS, Windows)
- Keine GPL-Kontamination
- Kommerziell ohne Einschränkung nutzbar

**Trademark-Hinweis:**
- "WireGuard" ist eine eingetragene Marke von Jason A. Donenfeld
- DARF NICHT in Marketing, UI-Labels oder Dokumentation verwendet werden ohne Erlaubnis
- ImpForge verwendet stattdessen: "Secure Tunnel" oder "Private Network"
- Bei Bedarf: Erlaubnis via wireguard-trademark-usage@zx2c4.com anfragen

**Quelle:** [WireGuard Trademark Policy](https://www.wireguard.com/trademark-policy/)

---

## 4. MCP (MODEL CONTEXT PROTOCOL)

### Lizenz
- **MCP Specification**: MIT License (Anthropic)
- **MCP SDKs**: MIT License
- **Kommerziell nutzbar**: JA, ohne Einschränkung

### Integration
- ImpForge implementiert einen eigenen MCP Client in Rust
- User installiert gewünschte MCP Server selbst
- ImpForge redistributed KEINE MCP Server
- Auto-Discovery für lokal laufende MCP Server

**Quelle:** [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)

---

## 5. AI MODEL INTEGRATION

### Ollama
- **Lizenz**: MIT License
- **Kommerziell nutzbar**: JA
- **Modelle**: Unterliegen individuellen Lizenzen (Llama Community, Apache 2.0, etc.)
- **ImpForge zeigt Modell-Lizenz VOR Download an** (Compliance)

### OpenRouter
- **API Terms**: Standard API Terms of Service
- **User verwendet eigenen API Key** — kein ImpForge API Key
- **Keine Redistribution** von API Keys oder Modellen

### HuggingFace Hub
- **Lizenz**: Apache 2.0
- **`hf-hub` Crate**: MIT/Apache-2.0
- **Modelle**: Individuelle Lizenzen, ImpForge zeigt Lizenz-Info an

---

## 6. SOCIAL MEDIA INTEGRATION

### API Terms of Service

| Platform | API | Kommerzielle Nutzung | Einschränkungen |
|----------|-----|---------------------|-----------------|
| **LinkedIn** | LinkedIn API | Ja (mit App Review) | Rate Limits, User-Auth required |
| **X/Twitter** | X API v2 | Ja (Free/Basic/Pro Tiers) | Rate Limits, Content Policy |
| **Instagram** | Instagram Graph API | Ja (Meta App Review) | Business/Creator Accounts only |
| **TikTok** | TikTok API | Ja (Developer Portal) | Content Policy, Review required |
| **Facebook** | Meta Graph API | Ja (App Review) | Privacy Policy required |
| **GitHub** | GitHub REST/GraphQL API | Ja (OAuth App) | Rate Limits, Scope-based |
| **Fiverr** | Fiverr API | Ja (Partner Program) | Approval required |
| **Upwork** | Upwork API | Ja (Developer Portal) | OAuth2, Rate Limits |

### ImpForge-Modell
- **User authentifiziert sich mit eigenem Account** (OAuth2/API Key)
- **ImpForge speichert KEINE Credentials auf eigenen Servern** — nur lokal in SQLite
- **Jede Aktion ist User-initiated** — kein autonomes Posten ohne Bestätigung
- **Rate Limits werden respektiert** — Circuit Breaker Pattern

---

## 7. DATENSCHUTZ & DSGVO-KONFORMITÄT

### Privacy by Design (Art. 25 DSGVO)
- **Lokale Datenhaltung**: Alle Daten in SQLite auf dem Gerät des Users
- **Keine Telemetrie**: Kein Tracking, keine Analytics, keine Datenerhebung
- **Verschlüsselung**: SQLite mit SQLCipher (AES-256-CBC) für sensible Daten

### Datenminimierung (Art. 5 DSGVO)
- Nur Daten die der User explizit eingibt werden verarbeitet
- Keine automatische Datenerhebung von Drittanbieter-Services

### Recht auf Löschung (Art. 17 DSGVO)
- One-Click Deletion: Alle AI-Daten, Memory, History
- Komplette App-Deinstallation entfernt alle Daten

### Transparenz (Art. 13/14 DSGVO)
- Klare Kennzeichnung von AI-generierten Inhalten
- User weiß welches Model seine Daten verarbeitet
- Keine versteckten Cloud-Calls — alles sichtbar im Activity Log

---

## 8. EU AI ACT COMPLIANCE (Regulation 2024/1689)

### Risikoklassifizierung
- **ImpForge = Begrenzt/Minimal Risk** (nicht High-Risk per Annex III)
- Keine biometrische Erkennung, kein Social Scoring, kein kritische Infrastruktur

### Art. 50 Transparenzpflichten
- Users werden informiert dass sie mit AI-Systemen interagieren
- AI-generierte Inhalte werden als solche gekennzeichnet
- Modell-Info (Name, Version, Provider) wird angezeigt

### Vollständige Anwendbarkeit: 2. August 2026

---

## 9. PRODUKTHAFTUNG

### EU Product Liability Directive 2024/2853
- **Software = Produkt** (explizit in der Richtlinie)
- **Umsetzungsfrist**: 9. Dezember 2026
- **ImpForge-Maßnahmen**:
  - Comprehensive Error Handling (AppError mit Suggestions)
  - Audit Logging für alle AI-Entscheidungen
  - Versionierung aller Modelle und Outputs
  - Klare Haftungsausschlüsse in AGB

### BGB Updatepflicht (§327f)
- Minimum 2 Jahre Sicherheits- und Funktionsupdates
- ImpForge Commitment: Mindestens 3 Jahre ab Kaufdatum

---

## 10. LIZENZMODELL

### ImpForge Dual-License
- **src-tauri/**: Apache-2.0 (Open Source)
- **crates/impforge-engine/**: BUSL-1.1 (Business Source License)
  - Automatische Umwandlung zu Apache-2.0 nach 4 Jahren
  - Verhindert direktes Kopieren des Engine-Kerns

### Dependency Audit
- Alle Dependencies: MIT, Apache-2.0, BSD, ISC, MPL-2.0
- KEINE GPL/AGPL/LGPL Dependencies (geprüft via `cargo-deny`)
- Drittanbieter-Lizenzen gebündelt in `THIRD_PARTY_LICENSES`

---

*Letzte Aktualisierung: 2026-03-15*
*Nächste Review: Vor Commercial Launch (qualifizierter Rechtsbeistand)*
