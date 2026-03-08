# Nexus AI Workstation Builder -- Legal Foundations Research

**Date**: 2026-03-08
**Product**: Nexus (Tauri + Svelte + Rust Desktop Application)
**Markets**: EU (primary), Global
**Status**: Pre-release legal compliance research

---

## Table of Contents

1. [EU AI Act (Regulation 2024/1689)](#1-eu-ai-act-regulation-20241689)
2. [GDPR (Regulation 2016/679)](#2-gdpr-regulation-2016679)
3. [Open Source License Compliance](#3-open-source-license-compliance)
4. [Model Licensing](#4-model-licensing)
5. [Software Distribution Law (EU/Germany)](#5-software-distribution-law-eugermany)
6. [Data Protection for AI (Specific)](#6-data-protection-for-ai-specific)
7. [Action Items and Recommendations](#7-action-items-and-recommendations)

---

## 1. EU AI Act (Regulation 2024/1689)

**Full Title**: Regulation (EU) 2024/1689 laying down harmonised rules on artificial intelligence
**Entry into force**: 1 August 2024
**Full applicability**: 2 August 2026 (with phased provisions)
**Official text**: [EUR-Lex](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689)

### 1.1 Risk Classification for Nexus

The EU AI Act uses a four-tier risk-based classification system:

| Risk Level | Description | Nexus Applicability |
|------------|-------------|---------------------|
| **Unacceptable** | Banned (social scoring, subliminal manipulation) | NOT APPLICABLE |
| **High-Risk** | Safety-critical sectors per Annex III | NOT APPLICABLE (see analysis below) |
| **Limited Risk** | Transparency obligations apply | LIKELY APPLICABLE |
| **Minimal Risk** | No mandatory obligations | PARTIALLY APPLICABLE |

**Analysis for Nexus**: As a desktop AI workstation builder / orchestrator tool, Nexus does NOT fall into any Annex III high-risk category (biometrics, critical infrastructure, education, employment, law enforcement, migration, justice). Nexus is a **developer tool** that orchestrates local AI models for code generation, chat, and workflow automation.

However, Nexus DOES interact with users via AI (chatbots, code suggestions) and therefore falls under **limited risk** transparency obligations.

**Reference**: [Article 6 -- Classification Rules](https://artificialintelligenceact.eu/article/6/) | [Annex III -- High-Risk AI Systems](https://artificialintelligenceact.eu/annex/3/)

#### Article 6: Classification Rules for High-Risk AI Systems

Key provisions relevant to Nexus:

- **Art. 6(1)**: An AI system is high-risk when it serves as a safety component of a product covered by EU harmonisation legislation (Annex I) AND requires third-party conformity assessment. Nexus is standalone software, not a safety component.
- **Art. 6(2)**: AI systems in Annex III are high-risk. Nexus does not match any Annex III category.
- **Art. 6(3)**: Derogation -- even Annex III systems are NOT high-risk if they perform narrow procedural tasks, improve human activity results, detect decision-making pattern deviations, or perform preparatory assessment tasks. Nexus qualifies under multiple derogation criteria.

**Conclusion**: Nexus is classified as **limited risk / minimal risk** under the EU AI Act.

### 1.2 Transparency Requirements (Articles 50, 53)

Even as a limited-risk system, Nexus must comply with Article 50 transparency obligations:

**Article 50 -- Transparency Obligations for Providers and Deployers**
([Full text](https://artificialintelligenceact.eu/article/50/) | [EU AI Act Service Desk](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-50))

| Requirement | Description | Nexus Action Required |
|-------------|-------------|----------------------|
| **Art. 50(1)** | AI systems interacting with users must inform them they are interacting with AI | YES -- Chat interface must disclose AI nature |
| **Art. 50(2)** | Synthetic content (text, image, audio, video) must be machine-readable labeled | YES -- AI-generated code/text must be identifiable |
| **Art. 50(4)** | Deepfake/manipulated content must be disclosed | NOT APPLICABLE (Nexus does not generate deepfakes) |

**Article 53 -- Obligations for Providers of General-Purpose AI Models**
([Full text](https://artificialintelligenceact.eu/article/53/) | [EU AI Act Service Desk](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-53))

Nexus is a **deployer/integrator** of general-purpose AI models, not a **provider** of GPAI models. Article 53 obligations fall on the model providers (Meta, Alibaba, Mistral, etc.), not on Nexus. However, Nexus should:
- Document which GPAI models it integrates
- Pass through any model-specific transparency information to users
- Maintain a registry of integrated models and their capabilities/limitations

### 1.3 Technical Documentation Requirements

Even for limited/minimal risk systems, the EU AI Act encourages (but does not mandate) documentation of:

- System architecture and design choices
- Training data sources (for models Nexus integrates)
- Known limitations and failure modes
- Human oversight mechanisms
- Intended use and reasonably foreseeable misuse

**Recommended**: Create technical documentation following the Annex IV template even though not legally required, as this demonstrates good practice and prepares for potential regulatory inquiries.

### 1.4 Applicable Articles Summary

| Article | Topic | Applicable to Nexus? |
|---------|-------|---------------------|
| Art. 4 | AI literacy | YES -- voluntary but recommended |
| Art. 5 | Prohibited practices | NO -- Nexus does not use prohibited techniques |
| Art. 6 | High-risk classification | NO -- Nexus is not high-risk |
| Art. 50 | Transparency obligations | YES -- must inform users of AI interaction |
| Art. 52 | GPAI notification procedure | NO -- Nexus is not a GPAI provider |
| Art. 53 | GPAI provider obligations | NO -- falls on model providers, not Nexus |
| Art. 95 | Codes of conduct | YES -- voluntary, recommended for trust |

### 1.5 Implementation Timeline

| Date | Milestone | Nexus Impact |
|------|-----------|--------------|
| 2 Feb 2025 | Prohibited practices + AI literacy | Ensure no prohibited practices |
| 2 Aug 2025 | GPAI model obligations | Model providers must comply; Nexus verifies |
| 2 Aug 2026 | Full applicability (all remaining) | Art. 50 transparency fully enforceable |
| 2 Aug 2027 | High-risk embedded products | Not applicable to Nexus |

---

## 2. GDPR (Regulation 2016/679)

**Full Title**: General Data Protection Regulation
**Applicable since**: 25 May 2018
**Official text**: [EUR-Lex](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32016R0679)

### 2.1 Key Articles for Local AI Processing

Nexus has a **significant privacy advantage** as an offline-first, local AI processing tool. Personal data stays on the user's machine and is never transmitted to cloud servers (when using local models). However, GDPR still applies to the processing itself.

#### Article 5 -- Principles Relating to Processing
([Full text](https://gdpr-info.eu/art-5-gdpr/))

| Principle | GDPR Requirement | Nexus Implementation |
|-----------|-------------------|----------------------|
| **Lawfulness, fairness, transparency** | Process data lawfully with clear information | Privacy notice in app, clear data handling disclosure |
| **Purpose limitation** | Collect for specified, explicit purposes | AI memory/context only for stated features |
| **Data minimization** | Adequate, relevant, limited to what is necessary | Only process data user explicitly provides |
| **Accuracy** | Keep data accurate and up to date | Allow users to edit/delete AI memories |
| **Storage limitation** | Keep only as long as necessary | Configurable retention periods |
| **Integrity and confidentiality** | Ensure appropriate security | Local encryption, no unnecessary network access |

#### Article 6 -- Lawful Basis for Processing
([Full text](https://gdpr-info.eu/art-6-gdpr/))

For Nexus, the most relevant lawful bases are:

1. **Art. 6(1)(a) -- Consent**: For optional features like AI memory, cloud model access
2. **Art. 6(1)(b) -- Contract**: Processing necessary to fulfill the software's core functionality
3. **Art. 6(1)(f) -- Legitimate interests**: Analytics, crash reporting (with opt-out)

**Recommendation**: Use **contract** as the primary basis for core AI features, and **consent** for optional data processing (telemetry, cloud features, memory systems).

#### Articles 13 and 14 -- Right to Information / Privacy Notice
([Article 13](https://gdpr-info.eu/art-13-gdpr/) | [Article 14](https://gdpr-info.eu/art-14-gdpr/))

Nexus must provide a privacy notice containing:
- Identity and contact details of the controller
- Purposes of processing and lawful basis
- Categories of personal data processed
- Recipients or categories of recipients
- Retention periods
- Data subject rights (access, rectification, erasure, portability, objection)
- Right to withdraw consent
- Right to lodge a complaint with a supervisory authority

### 2.2 Data Minimization (Article 5(1)(c))

**Nexus advantage**: Local-first architecture naturally supports data minimization:
- AI inference runs locally -- no personal data leaves the device
- No cloud training on user data
- User controls what data the AI processes
- No persistent storage unless user explicitly enables memory features

**Implementation requirements**:
- Default to minimal data collection
- Make AI memory/context features opt-in, not opt-out
- Clearly document what data is processed at each stage
- Provide granular controls for data retention

### 2.3 Right to Erasure (Article 17)
([Full text](https://gdpr-info.eu/art-17-gdpr/) | [Erasure in AI era - Leiden Law Blog](https://www.leidenlawblog.nl/articles/erasing-personal-data-in-an-ai-era))

The "Right to be Forgotten" has specific implications for AI memory systems:

| Requirement | Implementation for Nexus |
|-------------|--------------------------|
| Delete personal data on request | One-click deletion of all AI memories containing personal data |
| Data no longer necessary | Automatic cleanup of outdated context/memories |
| Consent withdrawn | Immediately stop processing and delete associated data |
| Notification to third parties | If data was shared with cloud APIs, notify them of erasure |

**AI-specific challenge**: If AI models have been fine-tuned on user data (future feature), "machine unlearning" may be required. For Nexus's current architecture (no local fine-tuning, only retrieval-based memory), simple database deletion suffices.

**Recommendation**: Implement a "Delete All AI Data" button that:
1. Clears all SQLite AI memory/context tables
2. Removes any cached embeddings
3. Clears conversation history
4. Provides confirmation of deletion

### 2.4 Privacy by Design (Article 25)
([Full text](https://gdpr-info.eu/art-25-gdpr/) | [EDPB Guidelines](https://www.edpb.europa.eu/sites/default/files/files/file1/edpb_guidelines_201904_dataprotection_by_design_and_by_default_v2.0_en.pdf))

Article 25 requires "data protection by design and by default":

**By Design** (Art. 25(1)):
- Implement technical measures from the design phase
- Use pseudonymization where possible
- Integrate data protection into processing activities

**By Default** (Art. 25(2)):
- Process only data necessary for each specific purpose
- Minimize amount collected, extent of processing, storage period, accessibility
- Default settings must be the most privacy-protective

**Nexus compliance measures**:
- Local-first processing (strongest privacy-by-design measure)
- No telemetry by default
- AI memory disabled by default, opt-in
- Cloud model access requires explicit user configuration
- All data stored in local SQLite (user-controlled)
- No account registration required for core functionality

---

## 3. Open Source License Compliance

### 3.1 MIT License

**Used by**: Ollama, rig-core, fsrs-rs, many Rust crates
**Full text**: [opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)

| Permission | Condition | Limitation |
|------------|-----------|------------|
| Commercial use | Include copyright notice | No warranty |
| Modification | Include license text | No liability |
| Distribution | | |
| Private use | | |

**Nexus obligations**:
- Include MIT license text and copyright notices in distribution
- Maintain attribution in "About" section or LICENSES file
- No obligation to open-source Nexus itself

### 3.2 Apache License 2.0

**Used by**: Many Rust crates, Qwen models (most variants), Mistral models
**Full text**: [apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0)

| Permission | Condition | Limitation |
|------------|-----------|------------|
| Commercial use | Include copyright notice | No trademark use |
| Modification | Include license text | No warranty |
| Distribution | State changes made | No liability |
| Patent use | Include NOTICE file if present | Patent retaliation clause |

**Key advantage**: Apache 2.0 includes an **explicit patent grant**, meaning contributors grant users a license to any patents necessary to use the code. This provides stronger legal protection than MIT for commercial products.

**Patent retaliation clause**: If Nexus sues any contributor over patents related to the licensed code, Nexus loses its patent license. This is protective, not restrictive.

**Nexus obligations**:
- Include Apache 2.0 license text
- Include any NOTICE files from dependencies
- Document modifications to Apache-licensed code
- Bundle all applicable licenses with distribution

### 3.3 GPL Contamination Risks

**Critical concern for commercial Rust projects**.

([Cargo license discussion](https://users.rust-lang.org/t/cargo-only-bsd-mit-no-gpl-crates/22861) | [LGPL in Rust](https://github.com/pythops/iwdrs/issues/1))

**Key risk**: Rust's static linking model means LGPL code is effectively treated as GPL, because LGPL requires users to be able to relink with modified library versions -- which is impractical with Rust's static compilation.

**Mitigation tools**:
- **`cargo-deny`** ([GitHub](https://github.com/EmbarkStudios/cargo-deny)): Automated license auditing
  - Detects GPL, AGPL, LGPL in dependency tree
  - Integrates into CI/CD pipeline
  - Configurable allow/deny lists per license

**Recommended `deny.toml` configuration for Nexus**:
```toml
[licenses]
unlicensed = "deny"
copyleft = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "BSL-1.0",
    "CC0-1.0",
    "OpenSSL",
    "MPL-2.0",           # Weak copyleft, acceptable for Rust
]
deny = [
    "GPL-2.0",
    "GPL-3.0",
    "AGPL-3.0",
    "LGPL-2.1",          # Problematic with Rust static linking
    "LGPL-3.0",
    "SSPL-1.0",
]
```

**Action item**: Run `cargo deny check licenses` on the Nexus project and resolve any violations before release.

### 3.4 Ollama Commercial Use

**License**: MIT ([GitHub LICENSE](https://github.com/ollama/ollama/blob/main/LICENSE))
**Confirmed**: [GitHub Issue #8218](https://github.com/ollama/ollama/issues/8218) -- Ollama is free for commercial use.

| Question | Answer |
|----------|--------|
| Can Nexus bundle Ollama? | YES -- MIT allows bundling |
| Can Nexus recommend Ollama installation? | YES -- no license restriction |
| Can Nexus modify Ollama? | YES -- with attribution |
| Does Nexus need to open-source? | NO -- MIT is permissive |

**Important**: Ollama's MIT license covers the **runtime/server software only**. Individual AI models downloaded through Ollama have their own separate licenses (see Section 4).

**Nexus obligations**:
- Include Ollama's MIT license and copyright notice
- Clearly separate Ollama's license from model licenses in documentation
- Include attribution in "About" or third-party licenses section

### 3.5 License Compliance Checklist

- [ ] Run `cargo deny check licenses` and resolve all violations
- [ ] Create a `THIRD_PARTY_LICENSES` file bundled with the application
- [ ] Add an "Open Source Licenses" section in Settings/About
- [ ] Document all dependencies and their licenses
- [ ] Set up CI/CD to automatically check licenses on every build
- [ ] Review any new dependencies before adding them

---

## 4. Model Licensing

### 4.1 Meta Llama 3 / 3.1 Community License

**License**: [Llama 3.1 Community License Agreement](https://www.llama.com/llama3_1/license/)
**Analysis**: [WCR Legal -- 700M MAU Limit](https://wcr.legal/llama-3-license-700m-mau-limit/)

| Term | Details | Nexus Impact |
|------|---------|--------------|
| **Commercial use** | ALLOWED below 700M MAU | Safe for Nexus (far below threshold) |
| **700M MAU threshold** | Free license expires at 700M monthly active users | Not a concern for desktop software |
| **Competitor restriction** | Cannot use in products competing with Meta (social networking, messaging, AI assistants) | Nexus is a developer tool, NOT a competing product |
| **Model training ban** | Cannot use Llama outputs to train non-Llama models | Nexus does not train models |
| **Redistribution** | Derivatives must stay under Llama Community License | Nexus does not redistribute the model itself |
| **Relicensing prohibition** | Cannot relicense as MIT/Apache/proprietary | Nexus does not relicense |

**Can Nexus recommend/download Llama models?** YES, with disclaimers:
- Nexus does not redistribute the model weights
- User downloads models directly via Ollama/Hugging Face
- User must accept Llama license themselves
- Nexus should display model license information in the UI

**Required disclaimers**:
- "Llama models are provided under Meta's Llama Community License Agreement"
- "Users must accept Meta's license terms before downloading Llama models"
- Link to full license text

### 4.2 Qwen License (Alibaba / Tongyi Qianwen)

**License**: Varies by model size
**Reference**: [Qwen License on HuggingFace](https://huggingface.co/Qwen/Qwen2.5-7B/blob/main/LICENSE) | [Qwen GitHub](https://github.com/QwenLM/Qwen/blob/main/LICENSE)

| Model | License | Commercial Use |
|-------|---------|---------------|
| Qwen2.5-0.5B to 14B | **Apache 2.0** | YES -- fully permissive |
| Qwen2.5-3B | Tongyi Qianwen License | YES -- with conditions |
| Qwen2.5-72B | Tongyi Qianwen License | YES -- may require agreement |
| Qwen2.5-Coder (most) | **Apache 2.0** | YES -- fully permissive |
| Qwen3 (most) | **Apache 2.0** | YES -- fully permissive |

**Nexus strategy**: Recommend **Apache 2.0 licensed** Qwen models (7B, 14B, Coder variants) as defaults. For models under the Tongyi Qianwen License, display the license and require user acknowledgment.

### 4.3 Mistral License

**License**: Apache 2.0 (most models)
**Reference**: [Mistral Help Center](https://help.mistral.ai/en/articles/347393-under-which-license-are-mistral-s-open-models-available) | [Mistral Legal](https://legal.mistral.ai/terms)

| Model | License | Commercial Use |
|-------|---------|---------------|
| Mistral Small 3 | Apache 2.0 | YES |
| Mistral NeMo (12B) | Apache 2.0 | YES |
| Mistral Large 3 | Apache 2.0 | YES |
| Mistral Commercial API | Proprietary terms | Separate agreement needed |

**Note**: Some Mistral models use a modified MIT license with a revenue threshold: companies with monthly revenue exceeding $20M USD may need a commercial agreement. Verify the specific license for each model variant.

**Nexus strategy**: Default to Apache 2.0 Mistral models. Display license information per model.

### 4.4 Model Bundling / Recommendation Rules

Nexus does NOT bundle model weights. Instead, it facilitates download through Ollama or Hugging Face Hub. This approach:

1. **Avoids redistribution obligations** -- user downloads directly from the model provider
2. **Shifts license acceptance to the user** -- user must agree to terms
3. **Reduces binary size** -- no multi-GB model files in the installer
4. **Enables model updates** -- users get latest versions independently

**Required UI elements**:
- Model selection screen showing license type per model
- License text viewer before first download
- "Accept License" checkbox for non-Apache models
- Clear attribution to model creators
- Warning when models have usage restrictions

### 4.5 Model License Disclaimers (Required)

The following disclaimers should be included in Nexus documentation and UI:

```
DISCLAIMER: AI models available through Nexus are provided by third parties
under their respective licenses. Nexus does not create, train, or modify
these models. Users are responsible for complying with each model's license
terms. Model outputs may be inaccurate, biased, or inappropriate. Nexus
provides AI models as tools and makes no warranty regarding model output
quality, accuracy, or fitness for any particular purpose.

Supported model licenses:
- Apache 2.0 (Qwen, Mistral, many others) -- permissive commercial use
- Llama Community License (Meta) -- commercial use with conditions
- Model-specific licenses -- see individual model pages for details
```

---

## 5. Software Distribution Law (EU/Germany)

### 5.1 Impressum (Legal Notice) Requirements

**Legal basis**: DDG Section 5 (formerly TMG Section 5, updated 14 May 2024)
**Reference**: [MTH Partner -- Imprint Obligation](https://www.mth-partner.de/en/internet-law-imprint-obligation-according-to-the-german-gdpr-create-a-legally-compliant-imprint/)

**Applies when**: Offering software commercially (even for free with ads/data collection)

**Required information**:
- Full legal name (person or company name with legal form)
- Postal address (no P.O. boxes)
- Email address for electronic contact
- Phone number or other quick electronic contact
- VAT identification number (Umsatzsteuer-ID) if applicable
- Commercial register entry (Handelsregister) if applicable
- Responsible person for editorial content (if applicable)

**Where to place it**: "About" section in the app + website, easily accessible

**Penalty for non-compliance**: Fines up to EUR 50,000 (Ordnungswidrigkeit)

### 5.2 AGB (Allgemeine Geschaeftsbedingungen / Terms of Service)

**Legal basis**: BGB Sections 305-310
**Reference**: [eRecht24 -- Digital Products AGB](https://www.e-recht24.de/agb/12841-digitale-waren-und-dienstleistungen.html)

**Required for commercial software distribution**:

| Section | Content | Notes |
|---------|---------|-------|
| Scope | What the AGB covers | Must not be buried |
| Product description | What Nexus is and does | Clear feature description |
| Pricing | License fees, subscription terms | Transparent pricing |
| Warranty | Gewaeherleistung terms | Cannot exclude statutory rights |
| Liability | Haftungsbeschraenkung | Limited but not fully excluded |
| Withdrawal | Widerrufsbelehrung | Mandatory for B2C |
| Data protection | Reference to privacy policy | Link to Datenschutzerklaerung |
| Applicable law | German law / EU law | For German/EU customers |
| Dispute resolution | Streitbeilegung | Link to EU ODR platform |

**Critical rules for AGB in Germany**:
- AGB clauses that surprise the customer or are unreasonably disadvantageous are VOID (Section 305c, 307 BGB)
- Cannot exclude statutory warranty rights for consumers
- Must be available BEFORE contract conclusion
- Must use clear, understandable language

### 5.3 Gewaehrleistung (Statutory Warranty)

**Legal basis**: BGB Sections 327-327u (digital products, since 01.01.2022)
**Reference**: [Hogan Lovells -- German BGB Digital Products](https://www.hoganlovells.com/en/publications/changes-to-the-german-civil-code-from-january-1-2022-new-contract-law) | [Verbraucherzentrale](https://www.verbraucherzentrale.de/wissen/vertraege-reklamation/kundenrechte/softwaregewaehrleistung-welche-rechte-habe-ich-bei-fehlenden-updates-74911)

**Scope**: Applies to ALL consumer contracts for digital products after 01.01.2022

| Requirement | Details | Duration |
|-------------|---------|----------|
| **Conformity** | Software must match description and be fit for purpose | At time of delivery |
| **Update obligation** | Must provide necessary functional AND security updates | "Relevant period" (usually 2 years for one-time purchase) |
| **Burden of proof** | Defects appearing within 12 months presumed to have existed at delivery | 12 months |
| **Warranty period** | Statutory minimum for consumers | 2 years from delivery |
| **Supplementary performance** | Fix bugs or replace within reasonable time | No deadline specified |

**Critical: Update Obligation (Updatepflicht, Section 327f BGB)**

This is unique to digital products law and highly relevant for Nexus:
- Must provide security updates for the "relevant period"
- Must provide compatibility updates (e.g., after OS updates)
- Must provide functional updates to maintain conformity
- Cannot charge extra for updates required to maintain conformity
- Failure to update = warranty breach

**Nexus implications**:
- Plan for a minimum 2-year update cycle after each sale
- Budget for security patching and compatibility maintenance
- Clearly communicate the update policy in the EULA/AGB
- Consider subscription model to fund ongoing updates

### 5.4 Widerrufsrecht (Right of Withdrawal)

**Legal basis**: BGB Sections 312g, 355, 356
**Reference**: [Heuking -- Widerrufsbelehrung Digital](https://www.heuking.de/de/news-events/newsletter-fachbeitraege/artikel/vorsicht-bei-widerrufsbelehrungen-bei-digitalen-inhalten-und-dienstleistungen.html) | [alfima -- Widerrufsrecht Digital](https://alfima.io/2025/09/17/widerrufsrecht-digitale-produkte/)

**General rule**: Consumers have a **14-day right of withdrawal** for distance contracts (online purchases).

**Exception for digital downloads (Section 356(5) BGB)**:

The withdrawal right expires early IF all three conditions are met:
1. Consumer **expressly consents** to begin performance before the withdrawal period expires
2. Consumer **acknowledges** that they lose their withdrawal right by giving consent
3. Seller provides confirmation on a **durable medium** (email)

**Nexus implementation**:
- Before download: Display withdrawal notice and obtain explicit consent
- Checkbox: "I agree that Nexus begins the download immediately and I understand that I lose my right of withdrawal once the download starts"
- Send confirmation email with withdrawal information
- If no consent obtained: full 14-day withdrawal right applies
- If consumer not properly informed: withdrawal right extends to **12 months + 14 days**

**Required Widerrufsbelehrung (withdrawal notice)** must include:
- Clear statement of the 14-day withdrawal right
- Conditions under which the right expires
- Model withdrawal form (Muster-Widerrufsformular)
- How to exercise the right (contact details)

### 5.5 Produkthaftung (Product Liability)

**Current law**: Produkthaftungsgesetz (ProdHaftG)
**New law**: EU Product Liability Directive 2024/2853 (applicable from 9 December 2026)
**Reference**: [DLA Piper -- German Reform](https://www.dlapiper.com/en/insights/blogs/cortex-life-sciences-insights/2025/germanys-draft-product-liability-reform) | [Reed Smith -- EU PLD Software](https://www.reedsmith.com/articles/eu-product-liability-directive-software-digital-products-cybersecurity/)

#### Current Law (until December 2026)
Under the existing ProdHaftG, software was traditionally not considered a "product" for liability purposes. However, German courts have increasingly applied product liability principles to software.

#### New EU Product Liability Directive 2024/2853 (from December 2026)

**Major change**: Software and AI systems are now EXPLICITLY covered as "products".

| Aspect | Old Law | New Directive |
|--------|---------|---------------|
| Software as product | Debated | YES -- explicitly included |
| AI systems covered | No | YES -- specifically mentioned |
| Open source exemption | N/A | Only if non-commercial |
| Defect standard | Manufacturing/design defect | Includes cybersecurity failures |
| Learning AI | N/A | Post-market changes considered |
| Burden of proof | On claimant | Shifted in some cases (disclosure) |

**Critical for Nexus**:

1. **Software = Product**: Nexus will be explicitly treated as a product under liability law
2. **AI learning behavior**: Courts must consider effects of AI's ability to learn after market placement when assessing defects
3. **Open source components**: When Nexus bundles open source AI components commercially, **Nexus bears strict manufacturer liability** for ALL defects, including those originating in open source components
4. **Update obligation**: Failure to provide security updates that the manufacturer has the ability to provide = potential liability
5. **Cybersecurity**: Cybersecurity vulnerabilities count as product defects

**Open source exemption details** ([Ferner Alsdorf analysis](https://www.ferner-alsdorf.com/the-new-eu-product-liability-landscape-for-software-ai-and-open-source/)):
- Non-commercial open source is exempt
- As soon as code is integrated into a commercial product, the integrator becomes the "manufacturer"
- No mechanism to shift liability back to open source authors
- Each open source import = risk assumption requiring evaluation

**Nexus mitigation**:
- Maintain a Software Bill of Materials (SBOM)
- Track security advisories for all dependencies
- Implement automated vulnerability scanning (RustSec, cargo-audit)
- Maintain product liability insurance
- Document quality assurance processes

### 5.6 EU Online Dispute Resolution (ODR)

**Requirement**: All EU online sellers must link to the EU ODR platform
**Link**: [https://ec.europa.eu/consumers/odr](https://ec.europa.eu/consumers/odr)
**Placement**: In AGB and on website/app

---

## 6. Data Protection for AI (Specific)

### 6.1 DSGVO Article 22 -- Automated Decision-Making

**Full text**: [GDPR Article 22](https://gdpr-info.eu/art-22-gdpr/)
**Analysis**: [GDPR Local](https://gdprlocal.com/automated-decision-making-gdpr/) | [EU Commission FAQ](https://commission.europa.eu/law/law-topic/data-protection/rules-business-and-organisations/dealing-citizens/are-there-restrictions-use-automated-decision-making_en)

**Core right**: Data subjects have the right NOT to be subject to decisions based solely on automated processing that produce legal effects or similarly significant effects.

**Applicability to Nexus**:

| Nexus Feature | Art. 22 Relevant? | Reason |
|---------------|-------------------|--------|
| AI code suggestions | NO | No legal/significant effect on user |
| AI chat responses | NO | Informational, no decision-making |
| Automated task routing | UNLIKELY | User-initiated, no significant effect |
| AI-based hiring/HR tools | YES (if built) | Legal effect on candidates |
| Credit/financial decisions | YES (if built) | Legal effect on data subjects |

**Current assessment**: Nexus in its current form does NOT make automated decisions that produce legal or similarly significant effects. However, if Nexus is extended to support enterprise workflows involving decisions about people, Article 22 obligations would apply.

**Safeguards (if Art. 22 applies)**:
- Right to obtain human intervention
- Right to express one's point of view
- Right to contest the decision
- Explanation of the logic involved
- Information about potential consequences

### 6.2 AI-Specific Data Protection Impact Assessment (DPIA)

**Legal basis**: GDPR Article 35
**Reference**: [GDPR Article 35](https://gdpr-info.eu/art-35-gdpr/) | [EU Commission -- When DPIA Required](https://commission.europa.eu/law/law-topic/data-protection/rules-business-and-organisations/obligations/when-data-protection-impact-assessment-dpia-required_en)

**When is a DPIA mandatory?** When processing is "likely to result in a high risk to the rights and freedoms of natural persons." The EDPB identifies 9 criteria; meeting 2 or more triggers a DPIA requirement:

| Criterion | Nexus Assessment |
|-----------|-----------------|
| 1. Evaluation/scoring | NO -- Nexus does not score/evaluate people |
| 2. Automated decision-making with legal effect | NO -- see Art. 22 analysis above |
| 3. Systematic monitoring | NO -- Nexus does not monitor people |
| 4. Sensitive data processing | POSSIBLE -- if users input sensitive data into AI |
| 5. Large-scale processing | NO -- local processing, single user |
| 6. Matching/combining datasets | POSSIBLE -- if AI combines data sources |
| 7. Vulnerable data subjects | NO -- enterprise/developer tool |
| 8. Innovative use of technology | YES -- AI/ML technology |
| 9. Processing preventing exercise of rights | NO |

**Current assessment**: Nexus meets 1-2 criteria maximum (innovative technology, possibly sensitive data). A formal DPIA is **recommended but likely not mandatory** for the current feature set.

**Recommendation**: Conduct a DPIA anyway as best practice, documenting:
- Processing operations and purposes
- Necessity and proportionality assessment
- Risk assessment for data subjects
- Mitigation measures (local processing, encryption, data minimization)

### 6.3 User Consent for Local Model Inference

**Key question**: Does local AI inference require user consent under GDPR?

**Analysis**:
- If processing personal data: YES -- a lawful basis is required (consent, contract, or legitimate interest)
- If processing only non-personal data (code, generic text): NO -- GDPR does not apply
- Nexus's offline-first architecture is a **privacy strength**: data never leaves the device

**Recommendation**:
- Use **contract** (Art. 6(1)(b)) as the lawful basis for core AI features
- Use **consent** (Art. 6(1)(a)) for optional features that process personal data
- Provide clear in-app privacy controls
- Display first-run privacy notice explaining data processing
- Allow users to disable AI features that process personal data

### 6.4 Interaction Between EU AI Act and GDPR

The EU AI Act and GDPR are complementary, not replacements:

| Requirement | GDPR | EU AI Act | Nexus Must |
|-------------|------|-----------|------------|
| Transparency | Art. 13-14 (privacy notice) | Art. 50 (AI disclosure) | Both |
| Data minimization | Art. 5(1)(c) | Implicit in risk management | GDPR controls |
| Right to explanation | Art. 22 (automated decisions) | Art. 50 (AI interaction) | Depends on features |
| Impact assessment | Art. 35 (DPIA) | Art. 9 (risk management for high-risk) | DPIA recommended |
| Record keeping | Art. 30 (processing records) | Art. 12 (logging for high-risk) | GDPR controls |

---

## 7. Action Items and Recommendations

### 7.1 Pre-Release Legal Checklist

#### Mandatory (Legal Requirements)

- [ ] **Impressum**: Create DDG Section 5 compliant legal notice for app and website
- [ ] **AGB/Terms of Service**: Draft B2C-compliant terms including warranty, liability, withdrawal
- [ ] **Datenschutzerklaerung / Privacy Policy**: GDPR-compliant privacy notice (Art. 13/14)
- [ ] **Widerrufsbelehrung**: Withdrawal notice with model form (BGB Section 355/356)
- [ ] **AI Transparency Notice**: Inform users they are interacting with AI (EU AI Act Art. 50)
- [ ] **Third-Party Licenses File**: Bundle all OSS licenses (MIT, Apache 2.0, etc.)
- [ ] **Model License Display**: Show model licenses before first download
- [ ] **Cookie/Tracking Consent**: If website uses cookies or analytics (ePrivacy)
- [ ] **EU ODR Link**: Link to EU Online Dispute Resolution platform

#### Strongly Recommended (Best Practices)

- [ ] **DPIA**: Conduct Data Protection Impact Assessment
- [ ] **cargo-deny**: Set up license auditing in CI/CD
- [ ] **SBOM**: Generate Software Bill of Materials
- [ ] **Security scanning**: Automated vulnerability detection (cargo-audit, RustSec)
- [ ] **Technical documentation**: AI system documentation following Annex IV template
- [ ] **Product liability insurance**: Obtain coverage for software product liability
- [ ] **Data deletion feature**: One-click AI data erasure (Art. 17 compliance)
- [ ] **Update policy**: Document and commit to update timeline (Section 327f BGB)

### 7.2 Key Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| GPL contamination in Rust deps | HIGH | Run cargo-deny, review all dependencies |
| Product liability for AI outputs | HIGH | Disclaimers, insurance, quality processes |
| Missing update obligation compliance | MEDIUM | Plan 2-year minimum update cycle |
| GDPR violation (data handling) | HIGH | Privacy by design, local-first, DPIA |
| EU AI Act non-compliance | LOW | Limited risk classification, transparency notice |
| Model license violation | MEDIUM | Display licenses, user accepts terms |
| Missing Widerrufsbelehrung | MEDIUM | Implement proper withdrawal flow |
| Missing Impressum | LOW | Add to About section + website |

### 7.3 Recommended Legal Counsel

Before commercial launch, consult specialized attorneys for:
- **IT-Recht / Software Law**: AGB review, liability assessment
- **Datenschutzrecht / Data Protection**: GDPR compliance review, DPIA
- **AI Regulation**: EU AI Act classification confirmation
- **Lizenzrecht / License Law**: Open source compliance audit

### 7.4 Regulatory Links and References

#### EU AI Act
- [Official Regulation Text (EUR-Lex)](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689)
- [EU AI Act Portal](https://artificialintelligenceact.eu/)
- [EU AI Act Service Desk](https://ai-act-service-desk.ec.europa.eu/)
- [Article 6 -- High-Risk Classification](https://artificialintelligenceact.eu/article/6/)
- [Article 50 -- Transparency Obligations](https://artificialintelligenceact.eu/article/50/)
- [Article 53 -- GPAI Provider Obligations](https://artificialintelligenceact.eu/article/53/)
- [Annex III -- High-Risk AI Systems](https://artificialintelligenceact.eu/annex/3/)
- [EU AI Act High-Level Summary](https://artificialintelligenceact.eu/high-level-summary/)
- [SecurePrivacy -- AI Act 2026 Compliance Guide](https://secureprivacy.ai/blog/eu-ai-act-2026-compliance)
- [SIG -- AI Act Summary (Jan 2026)](https://www.softwareimprovementgroup.com/blog/eu-ai-act-summary/)
- [Linux Foundation -- Open Source and the EU AI Act](https://linuxfoundation.eu/newsroom/ai-act-explainer)

#### GDPR
- [Official Regulation Text (EUR-Lex)](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32016R0679)
- [GDPR Info -- Complete Text](https://gdpr-info.eu/)
- [Article 5 -- Processing Principles](https://gdpr-info.eu/art-5-gdpr/)
- [Article 6 -- Lawful Basis](https://gdpr-info.eu/art-6-gdpr/)
- [Article 13 -- Information at Collection](https://gdpr-info.eu/art-13-gdpr/)
- [Article 17 -- Right to Erasure](https://gdpr-info.eu/art-17-gdpr/)
- [Article 22 -- Automated Decision-Making](https://gdpr-info.eu/art-22-gdpr/)
- [Article 25 -- Privacy by Design](https://gdpr-info.eu/art-25-gdpr/)
- [Article 35 -- DPIA](https://gdpr-info.eu/art-35-gdpr/)
- [EDPB Guidelines on Article 25](https://www.edpb.europa.eu/sites/default/files/files/file1/edpb_guidelines_201904_dataprotection_by_design_and_by_default_v2.0_en.pdf)
- [WilmerHale -- AI and GDPR Compliance by Design](https://www.wilmerhale.com/en/insights/blogs/wilmerhale-privacy-and-cybersecurity-law/20250801-ai-and-gdpr-a-road-map-to-compliance-by-design-episode-5-using-ai)
- [Leiden Law Blog -- Erasure in AI Era](https://www.leidenlawblog.nl/articles/erasing-personal-data-in-an-ai-era)

#### Open Source Licensing
- [MIT License Text](https://opensource.org/licenses/MIT)
- [Apache 2.0 License Text](https://www.apache.org/licenses/LICENSE-2.0)
- [Ollama License (MIT)](https://github.com/ollama/ollama/blob/main/LICENSE)
- [Ollama Commercial Use Discussion](https://github.com/ollama/ollama/issues/8218)
- [cargo-deny -- License Auditing](https://github.com/EmbarkStudios/cargo-deny)
- [Rust License Compliance Discussion](https://users.rust-lang.org/t/cargo-only-bsd-mit-no-gpl-crates/22861)
- [LGPL in Rust -- Static Linking Issue](https://github.com/pythops/iwdrs/issues/1)
- [Snyk -- Apache 2.0 Explained](https://snyk.io/articles/apache-license/)
- [Copyleft Contamination Risk](https://www.lawgratis.com/blog-detail/copyleft-contamination-risk-management)

#### Model Licenses
- [Meta Llama 3.1 Community License](https://www.llama.com/llama3_1/license/)
- [Llama 3 700M MAU Limit Analysis](https://wcr.legal/llama-3-license-700m-mau-limit/)
- [Qwen2.5 License (HuggingFace)](https://huggingface.co/Qwen/Qwen2.5-7B/blob/main/LICENSE)
- [Qwen License Discussion](https://github.com/QwenLM/Qwen/issues/778)
- [Mistral License FAQ](https://help.mistral.ai/en/articles/347393-under-which-license-are-mistral-s-open-models-available)
- [Mistral Legal Terms](https://legal.mistral.ai/terms)

#### German Software Distribution Law
- [DDG Section 5 -- Impressum (Imprint Obligation)](https://www.mth-partner.de/en/internet-law-imprint-obligation-according-to-the-german-gdpr-create-a-legally-compliant-imprint/)
- [BGB Sections 327-327u -- Digital Products](https://www.hoganlovells.com/en/publications/changes-to-the-german-civil-code-from-january-1-2022-new-contract-law)
- [BGB Section 356 -- Widerrufsrecht Digital](https://www.gesetze-im-internet.de/bgb/__356.html)
- [eRecht24 -- Digital Products AGB](https://www.e-recht24.de/agb/12841-digitale-waren-und-dienstleistungen.html)
- [Verbraucherzentrale -- Software Warranty](https://www.verbraucherzentrale.de/wissen/vertraege-reklamation/kundenrechte/softwaregewaehrleistung-welche-rechte-habe-ich-bei-fehlenden-updates-74911)
- [Heuking -- Widerrufsbelehrung Digital](https://www.heuking.de/de/news-events/newsletter-fachbeitraege/artikel/vorsicht-bei-widerrufsbelehrungen-bei-digitalen-inhalten-und-dienstleistungen.html)
- [alfima -- Widerrufsrecht Digital Products](https://alfima.io/2025/09/17/widerrufsrecht-digitale-produkte/)

#### Product Liability
- [EU Product Liability Directive 2024/2853 (EUR-Lex)](https://eur-lex.europa.eu/eli/dir/2024/2853/oj/eng)
- [DLA Piper -- German Product Liability Reform](https://www.dlapiper.com/en/insights/blogs/cortex-life-sciences-insights/2025/germanys-draft-product-liability-reform)
- [Reed Smith -- EU PLD and Software](https://www.reedsmith.com/articles/eu-product-liability-directive-software-digital-products-cybersecurity/)
- [Ferner Alsdorf -- Product Liability for Software, AI, and Open Source](https://www.ferner-alsdorf.com/the-new-eu-product-liability-landscape-for-software-ai-and-open-source/)
- [SKW Schwarz -- AI and Product Liability](https://www.skwschwarz.de/en/news/ki-flash-eu-produkthaftungsrichtlinie)
- [White & Case -- AI Product Liability in Germany](https://www.whitecase.com/insight-alert/navigating-product-liability-high-security-sectors-addressing-ai-driven-risks-under)
- [EU ODR Platform](https://ec.europa.eu/consumers/odr)

---

---

## 8. Docker Integration Legal Foundations

### 8.1 Docker Engine License

**License**: Apache License 2.0
**Repository**: [github.com/moby/moby](https://github.com/moby/moby/blob/master/LICENSE)

Docker Engine (Moby) is fully open source under Apache 2.0. Commercial use, modification, and distribution are permitted.

| Question | Answer |
|----------|--------|
| Can Nexus interact with Docker Engine? | YES -- Apache 2.0 permits all use |
| Can Nexus recommend Docker Engine? | YES -- no restriction |
| Must Nexus include Apache 2.0 text? | YES -- for Docker Engine attribution |

### 8.2 Docker Desktop License (IMPORTANT!)

**License**: Docker Subscription Service Agreement (proprietary)
**Reference**: [Docker Pricing](https://www.docker.com/pricing/) | [Docker Legal](https://www.docker.com/legal/docker-subscription-service-agreement/)

**Key change (August 2021, enforced January 2022)**:
- **Free** for: individuals, small businesses (<250 employees AND <$10M annual revenue), education, open source
- **Paid** for: larger businesses ($5-24/user/month depending on tier)

**Nexus strategy**: Nexus uses the `bollard` crate (MIT license) to communicate directly with the Docker Engine API. Nexus does NOT require Docker Desktop. Users who have Docker Engine installed (Linux) or any OCI-compatible runtime (Podman, containerd) can use Nexus's container management features.

**Required disclaimer in Nexus**:
```
Docker Desktop may require a paid subscription for commercial use in
organizations with 250+ employees or $10M+ annual revenue. Nexus
communicates with the Docker Engine API and does not require Docker Desktop.
See https://www.docker.com/pricing/ for current licensing terms.
```

### 8.3 Bollard Crate (Rust Docker Client)

**License**: MIT
**Repository**: [github.com/fussybeaver/bollard](https://github.com/fussybeaver/bollard)

Fully permissive for commercial use. Include MIT license text in THIRD_PARTY_LICENSES.

### 8.4 Container Image Licensing

**Important**: Docker/OCI container images have their OWN licenses independent of Docker's license.

| Image | Typical License | Commercial Use |
|-------|----------------|---------------|
| Official base images (ubuntu, alpine) | Varies (GPL for Ubuntu userland) | Permitted for running |
| Custom application images | User-defined | User's responsibility |
| Third-party images | Check each | User must verify |

**Nexus's role**: Nexus manages containers but does not redistribute images. License compliance for container contents is the user's responsibility.

### 8.5 DSGVO/GDPR for Docker Management

| Concern | Risk | Mitigation |
|---------|------|------------|
| Container logs may contain personal data | MEDIUM | Nexus does not persist container logs by default |
| Container names/IDs are not personal data | LOW | No GDPR concern |
| Docker API responses are technical metadata | LOW | No personal data processing |
| Environment variables may contain secrets | HIGH | Nexus never displays or logs env vars with secret patterns |

---

## 9. GitHub Integration Legal Foundations

### 9.1 GitHub API Terms of Service

**Reference**: [GitHub Terms of Service](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service) | [GitHub API Terms](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features)

**Key provisions for Nexus**:

| Provision | Details | Nexus Compliance |
|-----------|---------|-----------------|
| **Section H: API Terms** | No excessive load, respect rate limits | YES -- respects X-RateLimit headers |
| **Rate Limits** | 5,000 requests/hour (authenticated) | YES -- built-in rate limiter |
| **Abuse Rate Limits** | No concurrent requests abuse | YES -- sequential API calls |
| **User Authentication** | Must use user's own tokens/OAuth | YES -- users provide their own PAT/OAuth |
| **Data Collection** | Cannot collect user data beyond what's needed | YES -- minimal data processing |
| **Scraping** | May use API for data gathering, not scraping | YES -- uses API, not HTML scraping |

### 9.2 GitHub App vs OAuth App

| Aspect | OAuth App | GitHub App |
|--------|-----------|------------|
| Authentication | User grants access to their account | App installed on repos |
| Permissions | Broad scopes | Fine-grained per-repo |
| Rate Limits | 5,000/hr (per user token) | 5,000/hr (per installation) |
| Best for Nexus | Simple start, personal use | Enterprise, organization use |

**Recommendation**: Start with OAuth App (PAT-based), upgrade to GitHub App for enterprise tier.

### 9.3 Octocrab Crate License

**License**: MIT OR Apache-2.0 (dual license)
**Repository**: [github.com/XAMPPRocky/octocrab](https://github.com/XAMPPRocky/octocrab)

Dual-licensed under permissive licenses. Safe for commercial use.

### 9.4 GitHub Automation Ethics

GitHub's Acceptable Use Policy prohibits:
- Automated bulk creation of accounts
- Automated spam (issues, comments, PRs)
- Disrupting other users' use of GitHub
- Using bots to inflate stars/followers

**Nexus safeguards**:
- All GitHub actions require explicit user initiation
- No automated issue/PR creation without user approval
- Rate limiting to prevent accidental abuse
- Clear labeling of any automated actions

### 9.5 DSGVO/GDPR for GitHub Data

| Data Type | Personal Data? | Handling |
|-----------|---------------|----------|
| Username, email | YES | Display only, not stored persistently |
| Repository names | NO | Technical data |
| Issue/PR content | POSSIBLY | May contain personal data -- user's responsibility |
| Commit messages | POSSIBLY | Author names are personal data |

**Recommendation**: Display GitHub data in real-time (API calls), minimize local caching. Privacy notice should mention GitHub data processing.

---

## 10. n8n Workflow Automation Legal Foundations

### 10.1 n8n License (CRITICAL!)

**License**: [Sustainable Use License](https://github.com/n8n-io/n8n/blob/master/LICENSE.md) + [n8n Enterprise License](https://github.com/n8n-io/n8n/blob/master/LICENSE_EE.md)
**Model**: Fair-code (NOT open source!)
**Reference**: [n8n Fair-code](https://docs.n8n.io/hosting/community-edition/faircode/) | [n8n Pricing](https://n8n.io/pricing/)

**Key restrictions**:

| Use Case | Allowed? | Details |
|----------|----------|---------|
| Self-hosting for own use | YES | Community Edition is free |
| Embedding in commercial product | **NO** -- not without license | Sustainable Use License restricts this |
| Providing n8n as a service (SaaS) | **NO** | Requires Enterprise license |
| Internal business automation | YES | Self-hosted Community Edition |
| Redistribution | **NO** | Not permitted |
| Modification for own use | YES | But cannot redistribute modifications |

### 10.2 Nexus's Approach to n8n (IMPORTANT!)

**Nexus does NOT bundle, embed, or redistribute n8n.** Instead:

1. Nexus provides a **WebView browser panel** that connects to the user's own n8n instance
2. Users must install and run n8n separately (self-hosted or cloud)
3. Nexus merely provides a browser window — similar to any web browser accessing n8n
4. No n8n code is included in Nexus's distribution

**Legal analysis**: This approach is equivalent to a user opening n8n in Chrome/Firefox. Nexus does not provide n8n functionality — it provides a browser. The Sustainable Use License applies to n8n's code, not to browser access.

**Required disclaimer in Nexus**:
```
n8n is a third-party workflow automation platform licensed under the
Sustainable Use License. Nexus provides browser access to your n8n
instance but does not include or redistribute n8n. Users must comply
with n8n's license terms. Commercial use may require an n8n Enterprise
license. See https://n8n.io/pricing/ for details.
```

### 10.3 n8n Enterprise Requirements

If Nexus customers want to use n8n features that require Enterprise Edition:

| Feature | Community | Enterprise |
|---------|-----------|------------|
| Core workflows | YES | YES |
| >5 active workflows | NO | YES |
| Environments (dev/staging/prod) | NO | YES |
| SAML SSO | NO | YES |
| Source control | NO | YES |
| External secrets | NO | YES |

**Nexus recommendation**: Clearly communicate that n8n access in Nexus is for the user's existing n8n installation. Nexus is not an n8n reseller.

### 10.4 DSGVO/GDPR for n8n Integration

| Concern | Risk | Mitigation |
|---------|------|------------|
| n8n workflows may process personal data | HIGH (n8n's concern) | User's responsibility as data controller |
| Nexus WebView loads n8n UI | LOW | No data processing by Nexus, pure display |
| n8n API credentials in Nexus | MEDIUM | Store encrypted, never log, user-controlled |
| Workflow data visible in WebView | LOW | Equivalent to browser -- no Nexus-side processing |

---

## 11. Browser Automation Legal Considerations

### 11.1 WebView / Internal Browser

Nexus uses Tauri's built-in WebView (WRY) for rendering web content:

**License**: MIT/Apache-2.0 (Tauri/WRY)

**Legal considerations**:
- WebView renders web content but does not redistribute websites
- Same legal standing as any browser (Chrome, Firefox)
- Users are responsible for complying with websites' Terms of Service
- Nexus should respect `robots.txt` if implementing any automated browsing

### 11.2 Automated Browsing Restrictions

| Law/Policy | Restriction | Nexus Impact |
|------------|-------------|--------------|
| CFAA (US) | No unauthorized access | User authenticates with their own credentials |
| EU Directive 2019/790 Art. 4 | Text and Data Mining exceptions | Research/analysis permitted |
| Website ToS | Vary per site | User's responsibility |
| robots.txt | Convention, not law | Should be respected for automated features |

---

## 12. Compliance Checklist (Complete)

### Pre-Release (Mandatory)

- [ ] **Impressum** (DDG §5) in app About + website
- [ ] **AGB/Terms of Service** (BGB §§305-310) — B2C compliant
- [ ] **Datenschutzerklaerung / Privacy Policy** (GDPR Art. 13/14)
- [ ] **Widerrufsbelehrung** (BGB §§355-356) — withdrawal notice with model form
- [ ] **AI Transparency Notice** (EU AI Act Art. 50) — "You are interacting with AI"
- [ ] **THIRD_PARTY_LICENSES file** — all OSS licenses bundled
- [ ] **Model License Display** — per-model license shown before download
- [ ] **Docker Desktop Disclaimer** — pricing/licensing notice
- [ ] **n8n License Disclaimer** — Sustainable Use License notice
- [ ] **GitHub API Disclosure** — data processing notice in privacy policy
- [ ] **EU ODR Link** — https://ec.europa.eu/consumers/odr

### Pre-Release (Strongly Recommended)

- [ ] **DPIA** — Data Protection Impact Assessment
- [ ] **cargo-deny** — license audit in CI/CD pipeline
- [ ] **cargo-audit** — vulnerability scanning in CI/CD
- [ ] **SBOM** — Software Bill of Materials (CycloneDX or SPDX)
- [ ] **Product liability insurance** — software product coverage
- [ ] **Cookie/tracking consent** — if website uses analytics
- [ ] **Technical documentation** — AI system doc per Annex IV template
- [ ] **Data deletion feature** — one-click AI data erasure
- [ ] **Update policy document** — 2-year minimum commitment

### Ongoing Compliance

- [ ] **Dependency license monitoring** — check on every dependency update
- [ ] **Security advisory tracking** — RustSec, GitHub Advisories
- [ ] **EU AI Act updates** — monitor EU AI Office publications
- [ ] **n8n license changes** — monitor for license updates
- [ ] **Docker pricing changes** — monitor Docker Desktop licensing
- [ ] **GDPR enforcement** — track relevant supervisory authority decisions

---

*This document is legal research, not legal advice. Consult qualified legal counsel before commercial launch.*
*Last updated: 2026-03-08*
