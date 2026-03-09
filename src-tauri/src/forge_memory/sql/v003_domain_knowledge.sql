-- ForgeMemory v003 — Domain Knowledge Expansion (91 new tables)
--
-- Transforms ForgeMemory from a pure IDE memory into a comprehensive
-- knowledge engine covering software development, finance, research,
-- user understanding, and 13 web knowledge sources.
--
-- Total after v003: 171 tables (25 v001 + 55 v002 + 91 v003)
--
-- Groups:
--   N. Software Development & Engineering (8 tables)
--   O. Marketing & Business Intelligence (8 tables)
--   P. Finance: Stocks, ETFs, Crypto (8 tables)
--   Q. Web Development & Design (7 tables)
--   R. Platform & OS Intelligence (6 tables)
--   S. People & Contacts Registry (5 tables)
--   T. Scientific Research (6 tables)
--   U. Programming Languages (5 tables)
--   V. GPU & Hardware Intelligence (5 tables)
--   W. User Understanding & Preferences (8 tables)
--   X. Web Knowledge Sources (10 tables)
--   Y. Domain Knowledge & Learning (6 tables)
--   Z. AI & Model Management (5 tables)
--   AA. Collaboration & Review (3 tables)
--
-- Conventions (same as v001/v002):
--   UUID  -> TEXT (hex(randomblob(16)))
--   JSONB -> TEXT (JSON strings)
--   vector(N) -> BLOB (f32 arrays)
--   SERIAL/BIGSERIAL -> INTEGER PRIMARY KEY AUTOINCREMENT
--   TIMESTAMP WITH TIME ZONE -> TEXT (ISO 8601)
--   arrays -> TEXT (JSON arrays)
--   enums  -> TEXT with CHECK constraints
--
-- References:
--   - Personal Knowledge Management (Ahrens 2017, "How to Take Smart Notes")
--   - Spaced Repetition & Knowledge Graphs (Matuschak & Nielsen 2019)
--   - User Modeling (Brusilovsky & Millán 2007, "User Models for Adaptive Hypermedia")
--   - Portfolio Theory (Markowitz 1952, "Portfolio Selection")
--   - Information Foraging Theory (Pirolli & Card 1999)
--   - Bloom's Taxonomy for Skill Levels (Bloom et al. 1956, rev. Anderson & Krathwohl 2001)

-- ============================================================================
-- GROUP N: Software Development & Engineering (8 tables)
-- ============================================================================

-- Projects the user is working on
CREATE TABLE IF NOT EXISTS dev_projects (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    root_path   TEXT,
    language    TEXT,
    framework   TEXT,
    build_tool  TEXT,
    vcs_url     TEXT,
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','archived','paused','template')),
    tags        TEXT,
    metadata    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_projects_status ON dev_projects(status);
CREATE INDEX IF NOT EXISTS idx_dev_projects_language ON dev_projects(language);

-- Programming language knowledge base
CREATE TABLE IF NOT EXISTS dev_languages (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL UNIQUE,
    version     TEXT,
    paradigms   TEXT,
    typing      TEXT CHECK (typing IN ('static','dynamic','gradual','dependent')),
    compiled    INTEGER NOT NULL DEFAULT 0,
    package_manager TEXT,
    file_extensions TEXT,
    lsp_command TEXT,
    formatter   TEXT,
    linter      TEXT,
    test_framework TEXT,
    popularity_rank INTEGER,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Framework & library proficiency tracking
CREATE TABLE IF NOT EXISTS dev_frameworks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    language    TEXT NOT NULL,
    category    TEXT NOT NULL CHECK (category IN (
                    'web_frontend','web_backend','mobile','desktop','game',
                    'ml_ai','data','devops','testing','embedded','other')),
    version     TEXT,
    docs_url    TEXT,
    proficiency TEXT DEFAULT 'beginner'
                CHECK (proficiency IN ('beginner','intermediate','advanced','expert')),
    last_used   TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_frameworks_language ON dev_frameworks(language);
CREATE INDEX IF NOT EXISTS idx_dev_frameworks_category ON dev_frameworks(category);

-- Dependency tracking across projects
CREATE TABLE IF NOT EXISTS dev_dependencies (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES dev_projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    version     TEXT,
    registry    TEXT,
    dep_type    TEXT NOT NULL DEFAULT 'runtime'
                CHECK (dep_type IN ('runtime','dev','build','optional','peer')),
    license     TEXT,
    security_advisory TEXT,
    last_audit  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_deps_project ON dev_dependencies(project_id);

-- Build configurations per platform
CREATE TABLE IF NOT EXISTS dev_build_configs (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES dev_projects(id) ON DELETE CASCADE,
    platform    TEXT NOT NULL CHECK (platform IN (
                    'linux','windows','macos','android','ios','wasm','cross')),
    arch        TEXT,
    build_command TEXT NOT NULL,
    env_vars    TEXT,
    flags       TEXT,
    output_path TEXT,
    working     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_builds_project ON dev_build_configs(project_id);

-- Recognized code patterns and anti-patterns
CREATE TABLE IF NOT EXISTS dev_code_patterns (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    language    TEXT,
    category    TEXT NOT NULL CHECK (category IN (
                    'design_pattern','anti_pattern','idiom','architecture',
                    'performance','security','testing','refactoring')),
    description TEXT NOT NULL,
    example_code TEXT,
    when_to_use TEXT,
    when_to_avoid TEXT,
    refs        TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_patterns_category ON dev_code_patterns(category);
CREATE INDEX IF NOT EXISTS idx_dev_patterns_language ON dev_code_patterns(language);

-- Error solutions database (learn from past debugging)
CREATE TABLE IF NOT EXISTS dev_error_solutions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    error_message TEXT NOT NULL,
    error_type  TEXT,
    language    TEXT,
    framework   TEXT,
    root_cause  TEXT NOT NULL,
    solution    TEXT NOT NULL,
    prevention  TEXT,
    occurrences INTEGER NOT NULL DEFAULT 1,
    confidence  REAL NOT NULL DEFAULT 0.5 CHECK (confidence BETWEEN 0.0 AND 1.0),
    embedding   BLOB,
    last_seen   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_errors_language ON dev_error_solutions(language);

-- Reusable code snippets
CREATE TABLE IF NOT EXISTS dev_snippets (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    language    TEXT NOT NULL,
    code        TEXT NOT NULL,
    description TEXT,
    tags        TEXT,
    use_count   INTEGER NOT NULL DEFAULT 0,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_dev_snippets_language ON dev_snippets(language);

-- ============================================================================
-- GROUP O: Marketing & Business Intelligence (8 tables)
-- ============================================================================

-- Marketing campaigns
CREATE TABLE IF NOT EXISTS marketing_campaigns (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    platform    TEXT NOT NULL,
    campaign_type TEXT NOT NULL CHECK (campaign_type IN (
                    'content','email','social','paid','seo','event',
                    'referral','influencer','product_launch','other')),
    status      TEXT NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft','scheduled','active','paused','completed','cancelled')),
    budget      REAL,
    currency    TEXT DEFAULT 'USD',
    start_date  TEXT,
    end_date    TEXT,
    target_audience TEXT,
    goals       TEXT,
    results     TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_marketing_campaigns_status ON marketing_campaigns(status);

-- Marketing channels performance
CREATE TABLE IF NOT EXISTS marketing_channels (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL UNIQUE,
    channel_type TEXT NOT NULL CHECK (channel_type IN (
                    'organic_social','paid_social','email','seo','ppc',
                    'content','referral','direct','affiliate','other')),
    monthly_reach INTEGER,
    conversion_rate REAL,
    cost_per_acquisition REAL,
    roi         REAL,
    active      INTEGER NOT NULL DEFAULT 1,
    notes       TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Content pieces (blog posts, videos, social media)
CREATE TABLE IF NOT EXISTS marketing_content (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    campaign_id TEXT REFERENCES marketing_campaigns(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN (
                    'blog_post','video','social_post','email','landing_page',
                    'whitepaper','case_study','infographic','podcast','other')),
    platform    TEXT,
    url         TEXT,
    status      TEXT NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft','review','scheduled','published','archived')),
    publish_date TEXT,
    engagement  TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_marketing_content_type ON marketing_content(content_type);

-- Analytics data points
CREATE TABLE IF NOT EXISTS marketing_analytics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source      TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    dimensions  TEXT,
    period_start TEXT NOT NULL,
    period_end  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_marketing_analytics_source ON marketing_analytics(source, period_start);

-- Target audiences / personas
CREATE TABLE IF NOT EXISTS marketing_audiences (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    demographics TEXT,
    psychographics TEXT,
    pain_points TEXT,
    goals       TEXT,
    channels    TEXT,
    size_estimate INTEGER,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Business contacts / leads
CREATE TABLE IF NOT EXISTS business_contacts (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    company     TEXT,
    role        TEXT,
    email       TEXT,
    phone       TEXT,
    linkedin    TEXT,
    contact_type TEXT NOT NULL DEFAULT 'lead'
                CHECK (contact_type IN ('lead','prospect','customer','partner','vendor','other')),
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','inactive','converted','lost')),
    notes       TEXT,
    last_contact TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_business_contacts_type ON business_contacts(contact_type);

-- Business plans and strategies
CREATE TABLE IF NOT EXISTS business_plans (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    plan_type   TEXT NOT NULL CHECK (plan_type IN (
                    'business_plan','marketing_plan','product_roadmap',
                    'financial_plan','growth_strategy','exit_strategy','other')),
    content     TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft','review','approved','active','archived')),
    timeline    TEXT,
    kpis        TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Business KPIs and metrics
CREATE TABLE IF NOT EXISTS business_metrics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    unit        TEXT,
    category    TEXT NOT NULL DEFAULT 'general'
                CHECK (category IN ('revenue','growth','engagement','retention',
                                    'efficiency','quality','customer','general')),
    period      TEXT NOT NULL,
    target      REAL,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_business_metrics_category ON business_metrics(category);

-- ============================================================================
-- GROUP P: Finance — Stocks, ETFs, Crypto (8 tables)
-- ============================================================================

-- Investment portfolios
-- Reference: Modern Portfolio Theory (Markowitz 1952)
CREATE TABLE IF NOT EXISTS fin_portfolios (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    portfolio_type TEXT NOT NULL DEFAULT 'personal'
                CHECK (portfolio_type IN ('personal','retirement','trading',
                                          'savings','education','other')),
    currency    TEXT NOT NULL DEFAULT 'USD',
    total_value REAL,
    risk_level  TEXT CHECK (risk_level IN ('conservative','moderate','aggressive','speculative')),
    strategy    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Individual financial assets
CREATE TABLE IF NOT EXISTS fin_assets (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    portfolio_id TEXT REFERENCES fin_portfolios(id) ON DELETE SET NULL,
    symbol      TEXT NOT NULL,
    name        TEXT NOT NULL,
    asset_type  TEXT NOT NULL CHECK (asset_type IN (
                    'stock','etf','crypto','bond','commodity',
                    'option','futures','forex','reit','other')),
    exchange    TEXT,
    sector      TEXT,
    quantity    REAL NOT NULL DEFAULT 0,
    avg_cost    REAL,
    current_price REAL,
    currency    TEXT NOT NULL DEFAULT 'USD',
    notes       TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_fin_assets_type ON fin_assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_fin_assets_symbol ON fin_assets(symbol);
CREATE INDEX IF NOT EXISTS idx_fin_assets_portfolio ON fin_assets(portfolio_id);

-- Transaction history (buy/sell/transfer/dividend)
CREATE TABLE IF NOT EXISTS fin_transactions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    asset_id    TEXT REFERENCES fin_assets(id) ON DELETE SET NULL,
    portfolio_id TEXT REFERENCES fin_portfolios(id) ON DELETE SET NULL,
    tx_type     TEXT NOT NULL CHECK (tx_type IN (
                    'buy','sell','transfer_in','transfer_out',
                    'dividend','interest','fee','split','other')),
    quantity    REAL NOT NULL,
    price       REAL NOT NULL,
    total_amount REAL NOT NULL,
    fee         REAL DEFAULT 0,
    currency    TEXT NOT NULL DEFAULT 'USD',
    exchange    TEXT,
    notes       TEXT,
    executed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_fin_tx_asset ON fin_transactions(asset_id);
CREATE INDEX IF NOT EXISTS idx_fin_tx_date ON fin_transactions(executed_at DESC);

-- Watchlists
CREATE TABLE IF NOT EXISTS fin_watchlists (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS fin_watchlist_items (
    watchlist_id TEXT NOT NULL REFERENCES fin_watchlists(id) ON DELETE CASCADE,
    symbol      TEXT NOT NULL,
    asset_type  TEXT NOT NULL,
    target_price REAL,
    notes       TEXT,
    added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (watchlist_id, symbol)
);

-- Price alerts
CREATE TABLE IF NOT EXISTS fin_price_alerts (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    symbol      TEXT NOT NULL,
    alert_type  TEXT NOT NULL CHECK (alert_type IN (
                    'price_above','price_below','pct_change','volume_spike')),
    threshold   REAL NOT NULL,
    triggered   INTEGER NOT NULL DEFAULT 0,
    triggered_at TEXT,
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_fin_alerts_active ON fin_price_alerts(active, symbol);

-- Cached market data (avoid repeated API calls)
CREATE TABLE IF NOT EXISTS fin_market_data (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol      TEXT NOT NULL,
    data_type   TEXT NOT NULL CHECK (data_type IN (
                    'price','volume','market_cap','pe_ratio','dividend_yield',
                    'earnings','news','fundamentals','technicals')),
    value       TEXT NOT NULL,
    source      TEXT,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_fin_market_symbol ON fin_market_data(symbol, data_type);

-- Investment analysis notes
CREATE TABLE IF NOT EXISTS fin_analysis_notes (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    symbol      TEXT,
    asset_type  TEXT,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    sentiment   TEXT CHECK (sentiment IN ('bullish','bearish','neutral')),
    timeframe   TEXT CHECK (timeframe IN ('day','week','month','quarter','year','long_term')),
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP Q: Web Development & Design (7 tables)
-- ============================================================================

-- Web projects
CREATE TABLE IF NOT EXISTS web_projects (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    project_type TEXT NOT NULL CHECK (project_type IN (
                    'spa','ssr','ssg','pwa','api','fullstack',
                    'landing_page','ecommerce','blog','other')),
    frontend_framework TEXT,
    backend_framework TEXT,
    css_framework TEXT,
    hosting     TEXT,
    domain      TEXT,
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','deployed','maintenance','archived')),
    repo_url    TEXT,
    live_url    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- UI component library (reusable across projects)
CREATE TABLE IF NOT EXISTS web_components (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    category    TEXT NOT NULL CHECK (category IN (
                    'layout','navigation','form','data_display','feedback',
                    'overlay','media','typography','animation','utility')),
    framework   TEXT,
    html        TEXT,
    css         TEXT,
    js          TEXT,
    props       TEXT,
    variants    TEXT,
    preview_url TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_components_category ON web_components(category);

-- Design tokens (colors, spacing, typography)
CREATE TABLE IF NOT EXISTS web_design_tokens (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES web_projects(id) ON DELETE SET NULL,
    token_type  TEXT NOT NULL CHECK (token_type IN (
                    'color','spacing','typography','shadow','border',
                    'breakpoint','animation','opacity','z_index')),
    name        TEXT NOT NULL,
    value       TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_tokens_type ON web_design_tokens(token_type);

-- API endpoint documentation
CREATE TABLE IF NOT EXISTS web_api_endpoints (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES web_projects(id) ON DELETE SET NULL,
    method      TEXT NOT NULL CHECK (method IN ('GET','POST','PUT','PATCH','DELETE','HEAD','OPTIONS')),
    path        TEXT NOT NULL,
    description TEXT,
    request_schema TEXT,
    response_schema TEXT,
    auth_required INTEGER NOT NULL DEFAULT 0,
    rate_limit  TEXT,
    examples    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Deployment history
CREATE TABLE IF NOT EXISTS web_deployments (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES web_projects(id) ON DELETE CASCADE,
    environment TEXT NOT NULL CHECK (environment IN ('development','staging','production','preview')),
    version     TEXT,
    commit_sha  TEXT,
    deployed_by TEXT,
    status      TEXT NOT NULL CHECK (status IN ('pending','deploying','success','failed','rolled_back')),
    url         TEXT,
    logs        TEXT,
    deployed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_deploy_project ON web_deployments(project_id, deployed_at DESC);

-- Performance metrics (Lighthouse, Core Web Vitals)
CREATE TABLE IF NOT EXISTS web_perf_metrics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  TEXT REFERENCES web_projects(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    lcp_ms      REAL,
    fid_ms      REAL,
    cls         REAL,
    ttfb_ms     REAL,
    fcp_ms      REAL,
    lighthouse_perf INTEGER,
    lighthouse_a11y INTEGER,
    lighthouse_seo INTEGER,
    lighthouse_bp INTEGER,
    measured_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Accessibility audit results
CREATE TABLE IF NOT EXISTS web_a11y_audits (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    project_id  TEXT REFERENCES web_projects(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    standard    TEXT NOT NULL DEFAULT 'WCAG21AA'
                CHECK (standard IN ('WCAG20A','WCAG20AA','WCAG21AA','WCAG22AA','Section508')),
    violations  TEXT NOT NULL,
    passes      INTEGER NOT NULL DEFAULT 0,
    fails       INTEGER NOT NULL DEFAULT 0,
    notes       TEXT,
    audited_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP R: Platform & OS Intelligence (6 tables)
-- ============================================================================

-- OS-specific configurations
CREATE TABLE IF NOT EXISTS platform_configs (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    platform    TEXT NOT NULL CHECK (platform IN (
                    'linux','windows','macos','android','ios','freebsd')),
    distro      TEXT,
    version     TEXT,
    arch        TEXT CHECK (arch IN ('x86_64','aarch64','armv7','riscv64','wasm32')),
    kernel      TEXT,
    desktop_env TEXT,
    shell       TEXT,
    config_data TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Package manager tracking
CREATE TABLE IF NOT EXISTS platform_packages (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    config_id   TEXT REFERENCES platform_configs(id) ON DELETE CASCADE,
    manager     TEXT NOT NULL CHECK (manager IN (
                    'apt','pacman','dnf','zypper','brew','winget','scoop',
                    'choco','snap','flatpak','nix','cargo','pip','npm')),
    package_name TEXT NOT NULL,
    version     TEXT,
    purpose     TEXT,
    installed   INTEGER NOT NULL DEFAULT 1,
    installed_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_platform_pkgs_manager ON platform_packages(manager);

-- System services tracking
CREATE TABLE IF NOT EXISTS platform_services (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    config_id   TEXT REFERENCES platform_configs(id) ON DELETE CASCADE,
    service_name TEXT NOT NULL,
    service_type TEXT CHECK (service_type IN ('systemd','launchd','windows_service','cron','other')),
    enabled     INTEGER NOT NULL DEFAULT 1,
    running     INTEGER NOT NULL DEFAULT 0,
    port        INTEGER,
    description TEXT,
    config_path TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Driver configurations (GPU, audio, network)
CREATE TABLE IF NOT EXISTS platform_drivers (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    config_id   TEXT REFERENCES platform_configs(id) ON DELETE CASCADE,
    driver_type TEXT NOT NULL CHECK (driver_type IN (
                    'gpu','audio','network','storage','input','display','other')),
    driver_name TEXT NOT NULL,
    version     TEXT,
    vendor      TEXT,
    device_id   TEXT,
    env_vars    TEXT,
    working     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Keyboard shortcuts & shell aliases
CREATE TABLE IF NOT EXISTS platform_shortcuts (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    config_id   TEXT REFERENCES platform_configs(id) ON DELETE CASCADE,
    shortcut_type TEXT NOT NULL CHECK (shortcut_type IN (
                    'keyboard','shell_alias','shell_function','snippet','other')),
    trigger     TEXT NOT NULL,
    action      TEXT NOT NULL,
    application TEXT,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Environment variables per platform
CREATE TABLE IF NOT EXISTS platform_environments (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    config_id   TEXT REFERENCES platform_configs(id) ON DELETE CASCADE,
    var_name    TEXT NOT NULL,
    var_value   TEXT NOT NULL,
    scope       TEXT NOT NULL DEFAULT 'user'
                CHECK (scope IN ('system','user','project','session')),
    purpose     TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP S: People & Contacts Registry (5 tables)
-- ============================================================================

-- Contact registry (real people)
CREATE TABLE IF NOT EXISTS contacts (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    first_name  TEXT NOT NULL,
    last_name   TEXT,
    display_name TEXT,
    email       TEXT,
    phone       TEXT,
    company     TEXT,
    role        TEXT,
    location    TEXT,
    website     TEXT,
    github      TEXT,
    linkedin    TEXT,
    twitter     TEXT,
    avatar_url  TEXT,
    relationship TEXT NOT NULL DEFAULT 'acquaintance'
                CHECK (relationship IN ('family','friend','colleague','client',
                                        'mentor','mentee','acquaintance','other')),
    notes       TEXT,
    tags        TEXT,
    birthday    TEXT,
    last_contact TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts(first_name, last_name);
CREATE INDEX IF NOT EXISTS idx_contacts_email ON contacts(email);

-- Interaction history with contacts
CREATE TABLE IF NOT EXISTS contact_interactions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    contact_id  TEXT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    interaction_type TEXT NOT NULL CHECK (interaction_type IN (
                    'meeting','call','email','message','social','event','other')),
    subject     TEXT,
    content     TEXT,
    sentiment   TEXT CHECK (sentiment IN ('positive','neutral','negative')),
    follow_up   TEXT,
    occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_contact_interactions ON contact_interactions(contact_id, occurred_at DESC);

-- Contact tags (many-to-many)
CREATE TABLE IF NOT EXISTS contact_tags (
    contact_id  TEXT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    PRIMARY KEY (contact_id, tag)
);

-- Organizations
CREATE TABLE IF NOT EXISTS contact_organizations (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    org_type    TEXT CHECK (org_type IN (
                    'company','startup','nonprofit','government',
                    'university','community','open_source','other')),
    website     TEXT,
    industry    TEXT,
    size        TEXT CHECK (size IN ('solo','small','medium','large','enterprise')),
    location    TEXT,
    description TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Notes per contact
CREATE TABLE IF NOT EXISTS contact_notes (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    contact_id  TEXT REFERENCES contacts(id) ON DELETE CASCADE,
    org_id      TEXT REFERENCES contact_organizations(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP T: Scientific Research (6 tables)
-- ============================================================================

-- Scientific papers
-- Reference: Semantic Scholar API, arXiv, PubMed
CREATE TABLE IF NOT EXISTS papers (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    authors     TEXT NOT NULL,
    abstract    TEXT,
    doi         TEXT UNIQUE,
    arxiv_id    TEXT,
    venue       TEXT,
    year        INTEGER,
    pdf_url     TEXT,
    local_path  TEXT,
    read_status TEXT NOT NULL DEFAULT 'unread'
                CHECK (read_status IN ('unread','skimmed','reading','read','reference')),
    rating      INTEGER CHECK (rating BETWEEN 1 AND 5),
    tags        TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_papers_year ON papers(year DESC);
CREATE INDEX IF NOT EXISTS idx_papers_doi ON papers(doi);
CREATE INDEX IF NOT EXISTS idx_papers_status ON papers(read_status);

-- Paper authors (for collaboration network)
CREATE TABLE IF NOT EXISTS paper_authors (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    affiliation TEXT,
    h_index     INTEGER,
    scholar_id  TEXT,
    semantic_scholar_id TEXT,
    orcid       TEXT,
    homepage    TEXT,
    paper_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Citation graph (paper → paper)
CREATE TABLE IF NOT EXISTS paper_citations (
    citing_paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    cited_paper_id  TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    context     TEXT,
    citation_type TEXT CHECK (citation_type IN (
                    'foundational','methodological','comparison','extension','critique')),
    PRIMARY KEY (citing_paper_id, cited_paper_id)
);

-- Reading notes per paper
CREATE TABLE IF NOT EXISTS paper_notes (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    paper_id    TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    section     TEXT,
    content     TEXT NOT NULL,
    note_type   TEXT NOT NULL DEFAULT 'note'
                CHECK (note_type IN ('note','question','insight','critique','todo')),
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_paper_notes_paper ON paper_notes(paper_id);

-- Paper collections (reading lists, topic groups)
CREATE TABLE IF NOT EXISTS paper_collections (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS paper_collection_items (
    collection_id TEXT NOT NULL REFERENCES paper_collections(id) ON DELETE CASCADE,
    paper_id    TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (collection_id, paper_id)
);

-- Research topics of interest
CREATE TABLE IF NOT EXISTS research_topics (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    field       TEXT,
    description TEXT,
    interest_level TEXT NOT NULL DEFAULT 'medium'
                CHECK (interest_level IN ('high','medium','low','watching')),
    key_papers  TEXT,
    key_people  TEXT,
    notes       TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP U: Programming Languages (5 tables)
-- ============================================================================

-- Language profiles (deep knowledge per language)
CREATE TABLE IF NOT EXISTS lang_profiles (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    language    TEXT NOT NULL UNIQUE,
    paradigm    TEXT,
    type_system TEXT,
    memory_model TEXT CHECK (memory_model IN (
                    'gc','arc','ownership','manual','stack_only','other')),
    concurrency_model TEXT,
    ecosystem_size TEXT CHECK (ecosystem_size IN ('tiny','small','medium','large','massive')),
    maturity    TEXT CHECK (maturity IN ('experimental','growing','mature','legacy')),
    strengths   TEXT,
    weaknesses  TEXT,
    ideal_for   TEXT,
    avoid_for   TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Toolchains per language
CREATE TABLE IF NOT EXISTS lang_toolchains (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    language    TEXT NOT NULL,
    tool_type   TEXT NOT NULL CHECK (tool_type IN (
                    'compiler','runtime','package_manager','build_tool',
                    'linter','formatter','debugger','profiler','lsp','repl')),
    name        TEXT NOT NULL,
    install_cmd TEXT,
    config_file TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_lang_tools_lang ON lang_toolchains(language);

-- Best practices per language
CREATE TABLE IF NOT EXISTS lang_best_practices (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    language    TEXT NOT NULL,
    category    TEXT NOT NULL CHECK (category IN (
                    'naming','error_handling','testing','performance',
                    'security','concurrency','api_design','documentation')),
    title       TEXT NOT NULL,
    description TEXT NOT NULL,
    example     TEXT,
    anti_pattern TEXT,
    refs        TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_lang_bp_lang ON lang_best_practices(language);

-- Language migration guides (e.g. Python → Rust)
CREATE TABLE IF NOT EXISTS lang_migrations (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    from_lang   TEXT NOT NULL,
    to_lang     TEXT NOT NULL,
    concept     TEXT NOT NULL,
    from_syntax TEXT,
    to_syntax   TEXT,
    gotchas     TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_lang_mig ON lang_migrations(from_lang, to_lang);

-- Performance benchmarks across languages
CREATE TABLE IF NOT EXISTS lang_benchmarks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    benchmark_name TEXT NOT NULL,
    language    TEXT NOT NULL,
    version     TEXT,
    runtime_ms  REAL,
    memory_mb   REAL,
    throughput  REAL,
    hardware    TEXT,
    notes       TEXT,
    measured_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP V: GPU & Hardware Intelligence (5 tables)
-- ============================================================================

-- GPU profile configurations
CREATE TABLE IF NOT EXISTS gpu_profiles (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    vendor      TEXT NOT NULL CHECK (vendor IN ('amd','nvidia','intel','apple','other')),
    model       TEXT NOT NULL,
    codename    TEXT,
    vram_gb     REAL,
    compute_units INTEGER,
    architecture TEXT,
    driver_stack TEXT,
    driver_version TEXT,
    env_vars    TEXT,
    power_limit_w INTEGER,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- GPU compute configurations (ROCm, CUDA, Metal, Vulkan)
CREATE TABLE IF NOT EXISTS gpu_compute_configs (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    gpu_id      TEXT REFERENCES gpu_profiles(id) ON DELETE CASCADE,
    framework   TEXT NOT NULL CHECK (framework IN (
                    'cuda','rocm','metal','vulkan','opencl','sycl',
                    'directml','webgpu','onnxruntime','other')),
    version     TEXT,
    install_path TEXT,
    env_vars    TEXT,
    working     INTEGER NOT NULL DEFAULT 1,
    workarounds TEXT,
    notes       TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- GPU benchmarks
CREATE TABLE IF NOT EXISTS gpu_benchmarks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    gpu_id      TEXT REFERENCES gpu_profiles(id) ON DELETE CASCADE,
    benchmark_type TEXT NOT NULL CHECK (benchmark_type IN (
                    'ml_training','ml_inference','rendering','compute',
                    'gaming','memory_bandwidth','power_efficiency')),
    workload    TEXT NOT NULL,
    score       REAL,
    throughput  TEXT,
    latency_ms  REAL,
    vram_used_gb REAL,
    power_watts REAL,
    notes       TEXT,
    measured_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Model ↔ GPU compatibility matrix
CREATE TABLE IF NOT EXISTS gpu_model_compat (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    gpu_id      TEXT REFERENCES gpu_profiles(id) ON DELETE CASCADE,
    model_name  TEXT NOT NULL,
    model_size  TEXT,
    quantization TEXT,
    vram_required_gb REAL,
    tokens_per_sec REAL,
    works       INTEGER NOT NULL DEFAULT 1,
    workarounds TEXT,
    notes       TEXT,
    tested_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_gpu_compat_model ON gpu_model_compat(model_name);

-- Full hardware inventory
CREATE TABLE IF NOT EXISTS hardware_inventory (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    hw_type     TEXT NOT NULL CHECK (hw_type IN (
                    'cpu','gpu','ram','storage','motherboard','psu',
                    'monitor','keyboard','mouse','audio','network','other')),
    vendor      TEXT,
    model       TEXT NOT NULL,
    specs       TEXT,
    serial_number TEXT,
    purchase_date TEXT,
    warranty_until TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP W: User Understanding & Preferences (8 tables)
-- ============================================================================
-- Reference: Brusilovsky & Millán 2007, "User Models for Adaptive Hypermedia"
-- Reference: Bloom's Taxonomy (Anderson & Krathwohl 2001) for skill levels

-- User preferences (key-value store with categories)
CREATE TABLE IF NOT EXISTS user_preferences (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    category    TEXT NOT NULL CHECK (category IN (
                    'appearance','editor','workflow','communication',
                    'ai','notifications','privacy','accessibility','other')),
    pref_key    TEXT NOT NULL,
    pref_value  TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.5 CHECK (confidence BETWEEN 0.0 AND 1.0),
    source      TEXT NOT NULL DEFAULT 'inferred'
                CHECK (source IN ('explicit','inferred','observed','default')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(category, pref_key)
);

-- Skill levels per technology (Bloom's Taxonomy inspired)
CREATE TABLE IF NOT EXISTS user_skills (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    skill_name  TEXT NOT NULL,
    skill_type  TEXT NOT NULL CHECK (skill_type IN (
                    'language','framework','tool','concept','platform','domain')),
    level       TEXT NOT NULL DEFAULT 'beginner'
                CHECK (level IN ('novice','beginner','intermediate','advanced','expert','master')),
    confidence  REAL NOT NULL DEFAULT 0.5,
    evidence    TEXT,
    last_demonstrated TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(skill_name, skill_type)
);
CREATE INDEX IF NOT EXISTS idx_user_skills_type ON user_skills(skill_type);

-- Work patterns & productivity tracking
CREATE TABLE IF NOT EXISTS user_work_patterns (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    pattern_type TEXT NOT NULL CHECK (pattern_type IN (
                    'active_hours','break_pattern','focus_duration',
                    'peak_productivity','meeting_preference','deadline_behavior')),
    description TEXT NOT NULL,
    data        TEXT,
    confidence  REAL NOT NULL DEFAULT 0.5,
    observed_count INTEGER NOT NULL DEFAULT 1,
    last_observed TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Communication style preferences
CREATE TABLE IF NOT EXISTS user_communication_style (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    dimension   TEXT NOT NULL UNIQUE CHECK (dimension IN (
                    'verbosity','formality','language','emoji_usage',
                    'code_comments','explanation_depth','humor',
                    'response_format','proactivity')),
    preference  TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.5,
    examples    TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Learning style preferences
CREATE TABLE IF NOT EXISTS user_learning_style (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    dimension   TEXT NOT NULL UNIQUE CHECK (dimension IN (
                    'visual_vs_textual','theoretical_vs_practical',
                    'top_down_vs_bottom_up','example_driven',
                    'documentation_preference','video_preference',
                    'interactive_preference','pace')),
    preference  TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.5,
    evidence    TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Goals and aspirations (short-term + long-term)
CREATE TABLE IF NOT EXISTS user_goals (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    description TEXT,
    goal_type   TEXT NOT NULL CHECK (goal_type IN (
                    'career','project','skill','financial','health',
                    'learning','creative','social','other')),
    timeframe   TEXT NOT NULL CHECK (timeframe IN (
                    'daily','weekly','monthly','quarterly','yearly','long_term')),
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','achieved','paused','abandoned')),
    progress    REAL DEFAULT 0 CHECK (progress BETWEEN 0.0 AND 1.0),
    deadline    TEXT,
    milestones  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Explicit user feedback on system behavior
CREATE TABLE IF NOT EXISTS user_feedback (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    feedback_type TEXT NOT NULL CHECK (feedback_type IN (
                    'like','dislike','correction','suggestion',
                    'bug_report','feature_request','praise','complaint')),
    context     TEXT NOT NULL,
    content     TEXT NOT NULL,
    action_taken TEXT,
    resolved    INTEGER NOT NULL DEFAULT 0,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Personality traits & working style
CREATE TABLE IF NOT EXISTS user_personality (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    trait_name  TEXT NOT NULL UNIQUE,
    trait_type  TEXT NOT NULL CHECK (trait_type IN (
                    'work_style','decision_making','collaboration',
                    'risk_tolerance','creativity','organization',
                    'attention_to_detail','adaptability')),
    value       TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.5,
    evidence    TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP X: Web Knowledge Sources (10 tables)
-- ============================================================================
-- Reference: Information Foraging Theory (Pirolli & Card 1999)
-- These tables cache knowledge from web searches for persistent learning.

-- Wikipedia article cache
CREATE TABLE IF NOT EXISTS web_wikipedia (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    url         TEXT NOT NULL,
    summary     TEXT NOT NULL,
    full_content TEXT,
    categories  TEXT,
    language    TEXT NOT NULL DEFAULT 'en',
    revision_id TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5 CHECK (relevance BETWEEN 0.0 AND 1.0),
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_wiki_title ON web_wikipedia(title);

-- Reddit threads and insights
CREATE TABLE IF NOT EXISTS web_reddit (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    subreddit   TEXT NOT NULL,
    post_id     TEXT,
    title       TEXT NOT NULL,
    url         TEXT,
    content     TEXT,
    score       INTEGER,
    comment_count INTEGER,
    top_comments TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_reddit_sub ON web_reddit(subreddit);

-- Stack Overflow Q&A cache
CREATE TABLE IF NOT EXISTS web_stackoverflow (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    question_id INTEGER,
    title       TEXT NOT NULL,
    url         TEXT,
    question    TEXT NOT NULL,
    accepted_answer TEXT,
    top_answer  TEXT,
    score       INTEGER,
    tags        TEXT,
    language    TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_so_tags ON web_stackoverflow(tags);

-- GitHub repository metadata
CREATE TABLE IF NOT EXISTS web_github_repos (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    full_name   TEXT NOT NULL,
    description TEXT,
    url         TEXT NOT NULL,
    language    TEXT,
    stars       INTEGER,
    forks       INTEGER,
    topics      TEXT,
    license     TEXT,
    last_commit TEXT,
    readme_summary TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_gh_language ON web_github_repos(language);
CREATE INDEX IF NOT EXISTS idx_web_gh_stars ON web_github_repos(stars DESC);

-- Hacker News stories and discussions
CREATE TABLE IF NOT EXISTS web_hackernews (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    hn_id       INTEGER,
    title       TEXT NOT NULL,
    url         TEXT,
    score       INTEGER,
    author      TEXT,
    comment_count INTEGER,
    top_comments TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- arXiv paper metadata (lighter than full papers table)
CREATE TABLE IF NOT EXISTS web_arxiv (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    arxiv_id    TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    authors     TEXT NOT NULL,
    abstract    TEXT NOT NULL,
    categories  TEXT,
    primary_category TEXT,
    pdf_url     TEXT,
    published   TEXT,
    updated     TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_arxiv_cat ON web_arxiv(primary_category);

-- YouTube video and tutorial metadata
CREATE TABLE IF NOT EXISTS web_youtube (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    video_id    TEXT,
    title       TEXT NOT NULL,
    channel     TEXT,
    url         TEXT,
    description TEXT,
    duration_sec INTEGER,
    view_count  INTEGER,
    like_count  INTEGER,
    tags        TEXT,
    transcript  TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Dev blog posts (Dev.to, Medium, personal blogs)
CREATE TABLE IF NOT EXISTS web_devblogs (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    platform    TEXT NOT NULL CHECK (platform IN (
                    'devto','medium','hashnode','substack','personal','other')),
    title       TEXT NOT NULL,
    author      TEXT,
    url         TEXT NOT NULL,
    content     TEXT,
    tags        TEXT,
    reading_time_min INTEGER,
    reactions   INTEGER,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_devblogs_platform ON web_devblogs(platform);

-- Package registries (npm, PyPI, crates.io, etc.)
CREATE TABLE IF NOT EXISTS web_package_registries (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    registry    TEXT NOT NULL CHECK (registry IN (
                    'npm','pypi','crates_io','maven','nuget','rubygems',
                    'packagist','pub','hex','go_modules','other')),
    package_name TEXT NOT NULL,
    version     TEXT,
    description TEXT,
    downloads   INTEGER,
    license     TEXT,
    homepage    TEXT,
    repository  TEXT,
    dependencies TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_pkgs_registry ON web_package_registries(registry);
CREATE INDEX IF NOT EXISTS idx_web_pkgs_name ON web_package_registries(package_name);

-- Social trends (Twitter/X, LinkedIn, Product Hunt)
CREATE TABLE IF NOT EXISTS web_social_trends (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    platform    TEXT NOT NULL CHECK (platform IN (
                    'twitter','linkedin','product_hunt','mastodon',
                    'bluesky','threads','other')),
    content_type TEXT NOT NULL CHECK (content_type IN (
                    'post','trend','announcement','launch','discussion')),
    title       TEXT,
    content     TEXT NOT NULL,
    author      TEXT,
    url         TEXT,
    engagement  TEXT,
    relevance   REAL NOT NULL DEFAULT 0.5,
    embedding   BLOB,
    fetched_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_web_social_platform ON web_social_trends(platform);

-- ============================================================================
-- GROUP Y: Domain Knowledge & Learning (6 tables)
-- ============================================================================

-- Domain-specific glossaries
CREATE TABLE IF NOT EXISTS domain_glossaries (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    domain      TEXT NOT NULL,
    term        TEXT NOT NULL,
    definition  TEXT NOT NULL,
    examples    TEXT,
    related_terms TEXT,
    source      TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(domain, term)
);
CREATE INDEX IF NOT EXISTS idx_domain_gloss_domain ON domain_glossaries(domain);

-- Step-by-step tutorials
CREATE TABLE IF NOT EXISTS domain_tutorials (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    domain      TEXT NOT NULL,
    difficulty  TEXT NOT NULL DEFAULT 'intermediate'
                CHECK (difficulty IN ('beginner','intermediate','advanced','expert')),
    steps       TEXT NOT NULL,
    prerequisites TEXT,
    estimated_time_min INTEGER,
    tags        TEXT,
    source_url  TEXT,
    rating      INTEGER CHECK (rating BETWEEN 1 AND 5),
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_domain_tutorials_domain ON domain_tutorials(domain);

-- Checklists for common tasks
CREATE TABLE IF NOT EXISTS domain_checklists (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    domain      TEXT NOT NULL,
    items       TEXT NOT NULL,
    use_count   INTEGER NOT NULL DEFAULT 0,
    tags        TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Project templates
CREATE TABLE IF NOT EXISTS domain_templates (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    domain      TEXT NOT NULL,
    template_type TEXT NOT NULL CHECK (template_type IN (
                    'project','file','config','workflow','email',
                    'document','presentation','other')),
    content     TEXT NOT NULL,
    variables   TEXT,
    description TEXT,
    use_count   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Standards and specifications
CREATE TABLE IF NOT EXISTS domain_standards (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    domain      TEXT NOT NULL,
    version     TEXT,
    organization TEXT,
    url         TEXT,
    summary     TEXT,
    key_requirements TEXT,
    compliance_status TEXT CHECK (compliance_status IN (
                    'compliant','partial','non_compliant','not_applicable','unknown')),
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Certification tracks and progress
CREATE TABLE IF NOT EXISTS domain_certifications (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    provider    TEXT NOT NULL,
    domain      TEXT NOT NULL,
    level       TEXT,
    status      TEXT NOT NULL DEFAULT 'interested'
                CHECK (status IN ('interested','studying','scheduled','passed','expired')),
    exam_date   TEXT,
    expiry_date TEXT,
    score       REAL,
    study_resources TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP Z: AI & Model Management (5 tables)
-- ============================================================================

-- Available AI models registry
CREATE TABLE IF NOT EXISTS ai_model_registry (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    provider    TEXT NOT NULL CHECK (provider IN (
                    'ollama','huggingface','openai','anthropic','google',
                    'mistral','cohere','local','other')),
    model_type  TEXT NOT NULL CHECK (model_type IN (
                    'llm','embedding','vision','audio','multimodal',
                    'code','image_gen','video_gen','other')),
    model_id    TEXT NOT NULL,
    parameters  TEXT,
    context_length INTEGER,
    quantization TEXT,
    size_gb     REAL,
    license     TEXT,
    local_path  TEXT,
    api_endpoint TEXT,
    capabilities TEXT,
    performance_notes TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_ai_models_type ON ai_model_registry(model_type);
CREATE INDEX IF NOT EXISTS idx_ai_models_provider ON ai_model_registry(provider);

-- Prompt templates library
CREATE TABLE IF NOT EXISTS ai_prompt_templates (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    category    TEXT NOT NULL CHECK (category IN (
                    'coding','writing','analysis','creative','debugging',
                    'refactoring','testing','documentation','translation',
                    'summarization','extraction','conversation','other')),
    template    TEXT NOT NULL,
    variables   TEXT,
    model_hint  TEXT,
    use_count   INTEGER NOT NULL DEFAULT 0,
    avg_quality REAL,
    tags        TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_ai_prompts_category ON ai_prompt_templates(category);

-- Fine-tuning datasets
CREATE TABLE IF NOT EXISTS ai_fine_tune_data (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    dataset_name TEXT NOT NULL,
    purpose     TEXT NOT NULL,
    format      TEXT NOT NULL CHECK (format IN (
                    'chat','completion','instruction','preference',
                    'classification','qa','other')),
    sample_count INTEGER NOT NULL DEFAULT 0,
    data        TEXT,
    file_path   TEXT,
    quality_score REAL,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Evaluation benchmark sets
CREATE TABLE IF NOT EXISTS ai_evaluation_sets (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    eval_type   TEXT NOT NULL CHECK (eval_type IN (
                    'accuracy','latency','quality','safety',
                    'consistency','creativity','factuality','other')),
    model_id    TEXT,
    test_cases  TEXT NOT NULL,
    results     TEXT,
    score       REAL,
    notes       TEXT,
    evaluated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Model routing rules (which model for which task)
CREATE TABLE IF NOT EXISTS ai_model_routing (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    task_type   TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    priority    INTEGER NOT NULL DEFAULT 1,
    conditions  TEXT,
    fallback_model TEXT,
    active      INTEGER NOT NULL DEFAULT 1,
    performance TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_ai_routing_task ON ai_model_routing(task_type, priority);

-- ============================================================================
-- GROUP AA: Collaboration & Review (3 tables)
-- ============================================================================

-- Collaborative workspaces
CREATE TABLE IF NOT EXISTS collab_workspaces (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    workspace_type TEXT NOT NULL CHECK (workspace_type IN (
                    'project','team','personal','shared','template')),
    owner       TEXT,
    members     TEXT,
    settings    TEXT,
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Code annotations (inline comments, bookmarks)
CREATE TABLE IF NOT EXISTS collab_annotations (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    file_path   TEXT NOT NULL,
    line_start  INTEGER NOT NULL,
    line_end    INTEGER,
    annotation_type TEXT NOT NULL CHECK (annotation_type IN (
                    'comment','bookmark','todo','question','issue',
                    'suggestion','highlight','warning')),
    content     TEXT NOT NULL,
    author      TEXT,
    resolved    INTEGER NOT NULL DEFAULT 0,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_collab_ann_file ON collab_annotations(file_path);

-- Review history
CREATE TABLE IF NOT EXISTS collab_reviews (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    target_type TEXT NOT NULL CHECK (target_type IN (
                    'pull_request','commit','file','design','document','other')),
    target_ref  TEXT NOT NULL,
    reviewer    TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','in_progress','approved',
                                  'changes_requested','rejected')),
    comments    TEXT,
    score       INTEGER CHECK (score BETWEEN 1 AND 5),
    summary     TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
