# MCP Server Research for NEXUS AI Workstation Builder

**Date**: 2026-03-07
**Purpose**: Identify high-value open-source MCP servers to integrate into NEXUS beyond the inherited ORK-Station servers.

---

## Current NEXUS MCP Server Inventory (from ORK-Station)

| Server | Port | Purpose |
|--------|------|---------|
| ork-harmony | 8001 | Memory System |
| ork-semantic | 8002 | RAG, Hybrid Search, Tasks |
| ork-blender | 8003 | 3D Automation |
| ork-offline-coding | 8004 | Local LLMs (Ollama) |
| ork-creative-apps | 8005 | Krita/GIMP/Cascadeur |
| unity-mcp | 8006 | Unity Editor |
| ork-unlimited-context | 8010 | UCF/RLM |
| playwright | stdio | Browser Automation |
| context7 | stdio | Library Documentation |
| pinecone | stdio | Vector Database |
| linear | stdio | Project Management |
| slack | stdio | Team Communication |
| hunyuan3d | 8007 | 3D Model Generation |

---

## Recommended New MCP Servers for NEXUS

### 1. GitHub Official MCP Server

**Repository**: [github/github-mcp-server](https://github.com/github/github-mcp-server)
**Stars**: 27,600+
**License**: MIT

**What it does**: GitHub's official MCP server provides full GitHub platform integration for AI agents. It enables repository management, issue/PR automation, CI/CD workflow intelligence, code security analysis (Dependabot), and team collaboration -- all through natural language.

**Key Tools**:
- Repository operations (browse, search, create, fork)
- Issue management (create, update, search, label)
- Pull request handling (create, review, merge, diff)
- GitHub Actions workflow monitoring and triggering
- Code security scanning and Dependabot alert management
- Discussions and team collaboration

**Installation**:
```bash
# Remote (easiest -- hosted by GitHub)
# URL: https://api.githubcopilot.com/mcp/

# Docker
docker pull ghcr.io/github/github-mcp-server

# Local binary (Go)
go install github.com/github/github-mcp-server@latest
```

**NEXUS Integration**:
- Replaces manual `gh` CLI usage in automation workflows
- Enables AI agents to autonomously manage repositories, review PRs, and monitor CI/CD
- Pairs with Linear MCP for end-to-end project management (Linear issues -> GitHub PRs -> CI/CD)
- Can use the remote hosted endpoint (no local server needed) or Docker for offline scenarios

**Priority**: HIGH -- this is the most popular MCP server (27K+ stars) and directly relevant to any development workstation.

---

### 2. Postgres MCP Pro (crystaldba)

**Repository**: [crystaldba/postgres-mcp](https://github.com/crystaldba/postgres-mcp)
**Stars**: 2,300+
**License**: MIT

**What it does**: Production-grade PostgreSQL MCP server with index tuning, EXPLAIN plans, health checks, and safe SQL execution. Designed to support AI agents throughout the entire database lifecycle -- from development through production tuning and maintenance.

**Key Tools**:
- `list_schemas` / `list_objects` -- Schema and table discovery
- `get_object_details` -- Columns, constraints, indexes for any table
- `execute_sql` -- Safe SQL execution with configurable access control (read-only mode)
- `explain_query` -- Execution plans with hypothetical index simulation
- `get_top_queries` -- Identify slowest queries via pg_stat_statements
- `analyze_workload_indexes` -- Industrial-strength index recommendations
- `analyze_db_health` -- Comprehensive health checks (connections, buffer cache, vacuum, replication)

**Installation**:
```bash
# Docker
docker pull crystaldba/postgres-mcp

# pipx (recommended)
pipx install postgres-mcp

# uv
uv pip install postgres-mcp
```

**Configuration** (Claude Desktop / MCP client):
```json
{
  "mcpServers": {
    "postgres-mcp": {
      "command": "postgres-mcp",
      "args": ["postgresql://user:pass@localhost:5433/orkstation_archive"]
    }
  }
}
```

**NEXUS Integration**:
- Direct integration with the existing PostgreSQL 16 (port 5433) orkstation_archive database
- Complements ork-semantic's task/memory tables with DBA-level analysis
- AI agents can autonomously optimize indexes, diagnose slow queries, and monitor database health
- Safe read-only mode for production environments
- Health monitoring feeds into NeuralSwarm dashboard metrics

**Priority**: HIGH -- NEXUS already relies heavily on PostgreSQL; this adds DBA intelligence that no existing server provides.

---

### 3. Grafana MCP Server (Official)

**Repository**: [grafana/mcp-grafana](https://github.com/grafana/mcp-grafana)
**Stars**: 2,500+
**License**: Apache 2.0

**What it does**: Official Grafana MCP server that provides AI agents access to the full Grafana observability ecosystem -- dashboards, Prometheus/Loki queries, alerting, incident management, OnCall, and Sift investigations.

**Key Tools**:
- **Dashboards**: Search, retrieve, modify, render as PNG images
- **Data Queries**: Prometheus metrics, Loki logs, ClickHouse, CloudWatch, Elasticsearch
- **Alerting**: Manage alert rules and notification routing
- **Incidents**: Create and manage incidents in Grafana Incident
- **OnCall**: Schedule management and shift tracking
- **Rendering**: Generate dashboard/panel images as base64-encoded PNGs
- **Prometheus-specific**: Metric metadata retrieval, histogram percentile calculation (p50/p90/p95/p99)
- **Loki-specific**: Log pattern analysis, error detection, structured log querying

**Installation**:
```bash
# Binary (recommended)
# Download from GitHub releases

# Docker
docker pull grafana/mcp-grafana

# From source (Go)
go install github.com/grafana/mcp-grafana@latest
```

**Configuration**:
```json
{
  "mcpServers": {
    "grafana": {
      "command": "mcp-grafana",
      "env": {
        "GRAFANA_URL": "http://localhost:3000",
        "GRAFANA_SERVICE_ACCOUNT_TOKEN": "<token>"
      }
    }
  }
}
```

**NEXUS Integration**:
- Provides the monitoring/observability layer that NEXUS currently lacks
- AI agents can query Prometheus metrics about system health, GPU utilization, MCP server performance
- Loki log querying enables AI-driven log analysis and error detection
- Dashboard rendering provides visual system status for ImpUI/ImpOS NeuralSwarm module
- Alert management enables proactive issue detection
- Pairs with existing systemd services monitoring (ork-semantic, ork-harmony, etc.)

**Priority**: HIGH -- critical for enterprise-grade observability. NEXUS needs monitoring beyond basic health checks.

---

### 4. Kubernetes MCP Server (containers org)

**Repository**: [containers/kubernetes-mcp-server](https://github.com/containers/kubernetes-mcp-server)
**Stars**: 1,200+
**License**: Apache 2.0

**What it does**: A native Go implementation (not a kubectl wrapper) that interacts directly with the Kubernetes API server. Supports full CRUD operations on any K8s resource, pod management, Helm operations, and multi-cluster management. Includes OpenTelemetry tracing.

**Key Tools**:
- **Generic Resources**: Create/Update, Get, List, Delete any Kubernetes resource
- **Pod Management**: List, get details, delete, exec commands, view logs, resource usage metrics
- **Helm**: Install charts, list releases, uninstall releases
- **Config**: Kubeconfig management, context switching, multi-cluster support
- **KubeVirt**: Virtual machine management (if KubeVirt is installed)
- **Kiali**: Service mesh observability (if Istio/Kiali is installed)
- **Observability**: OpenTelemetry distributed tracing with custom sampling rates

**Installation**:
```bash
# NPX (easiest)
npx kubernetes-mcp-server@latest

# Binary download from GitHub releases
# Available for Linux, macOS, Windows

# Python
pip install kubernetes-mcp-server
```

**NEXUS Integration**:
- Essential for NEXUS deployments that use Kubernetes (NeuralSwarm distributed agents)
- Enables AI-driven container orchestration -- scaling agent pools, managing deployments
- Pod log inspection and resource monitoring complement Grafana MCP
- Helm chart management for deploying NEXUS components
- Multi-cluster support for distributed NEXUS installations
- Direct API interaction (no kubectl dependency) makes it lightweight

**Priority**: MEDIUM-HIGH -- essential if NEXUS targets Kubernetes deployments. Lower priority for single-machine setups.

---

### 5. Docker MCP Server

**Repository**: [QuantGeekDev/docker-mcp](https://github.com/QuantGeekDev/docker-mcp)
**Stars**: 453
**License**: MIT

**What it does**: MCP server for Docker operations enabling container and Docker Compose stack management through AI agents. Covers container lifecycle, compose deployments, and log retrieval.

**Key Tools**:
- `create-container` -- Build standalone Docker containers with configurable ports and env vars
- `deploy-compose` -- Deploy multi-service stacks using Docker Compose YAML
- `get-logs` -- Retrieve container output for debugging
- `list-containers` -- Display all containers and their status

**Installation**:
```bash
# Via uvx (recommended)
# Add to MCP config: "command": "uvx", "args": ["docker-mcp"]

# Via Smithery
npx @smithery/cli install docker-mcp --client claude
```

**Also consider**: [docker/mcp-gateway](https://github.com/docker/mcp-gateway) -- Docker's official MCP gateway that provides a unified interface for running MCP servers in isolated containers with secrets management.

**NEXUS Integration**:
- Manage Docker containers for MCP servers, Ollama, ComfyUI, and other NEXUS services
- Deploy/redeploy service stacks via natural language
- Container log inspection for debugging service issues
- Compose stack management for NEXUS multi-service deployments
- Pairs with Kubernetes MCP for hybrid Docker+K8s environments

**Priority**: MEDIUM -- useful for container management but less critical than GitHub/Postgres/Grafana.

---

### 6. Code Pathfinder MCP Server

**Repository**: [shivasurya/code-pathfinder](https://github.com/shivasurya/code-pathfinder)
**License**: AGPL-3.0

**What it does**: AI-native static code analysis using AST (Abstract Syntax Trees), CFG (Control Flow Graphs), and DFG (Data Flow Graphs). Enables AI agents to perform deep semantic code analysis -- call graphs, symbol search, dependency tracing, and dataflow analysis. Runs locally; code never leaves the machine.

**Key Tools**:
- Project statistics and structure overview
- Symbol search across codebase
- Forward and reverse call graph analysis
- Call site detail inspection
- Import resolution and dependency tracing
- Dataflow tracking through application

**Analysis Pipeline**: 5-pass AST-based indexing that parses code into ASTs, builds CFGs for execution path tracking, and constructs DFGs for data flow tracing.

**Current Language Support**: Python (JavaScript, TypeScript, Go, Java planned)

**Installation**:
```bash
# Follow setup instructions at codepathfinder.dev/mcp
# Indexes codebase automatically using AST-based analysis
```

**NEXUS Integration**:
- Enables AI agents to deeply understand NEXUS Python codebases (MCP servers, Archivar, services)
- Call graph analysis helps with refactoring and dependency management
- Security-focused static analysis for vulnerability detection
- Runs entirely offline -- fits NEXUS offline-first architecture
- Complements context7 (library docs) with deep code structure analysis

**Priority**: MEDIUM -- valuable for code intelligence but limited to Python currently.

---

### 7. mcp-agent (LastMile AI)

**Repository**: [lastmile-ai/mcp-agent](https://github.com/lastmile-ai/mcp-agent)
**Stars**: 8,100+
**License**: Apache 2.0

**What it does**: A framework for building effective AI agents using composable MCP patterns. Supports map-reduce, orchestrator, evaluator-optimizer, router, and multi-agent handoff patterns. Includes Temporal-backed durable execution for pause/resume/recovery.

**Key Features**:
- Full MCP support (Tools, Resources, Prompts, Notifications, OAuth, Sampling, Elicitation)
- Composable agent patterns: map-reduce, orchestrator, evaluator-optimizer, router
- Durable execution via Temporal (pause, resume, recover workflows)
- Multi-provider LLM support (OpenAI, Anthropic, Google, Azure, Bedrock)
- Agents can be exposed as MCP servers themselves
- YAML-based agent configuration

**Installation**:
```bash
# Via uv (recommended)
uv add "mcp-agent"

# Via pip
pip install mcp-agent

# With LLM providers
uv add "mcp-agent[openai,anthropic,google]"
```

**NEXUS Integration**:
- Powers the NeuralSwarm agent pool with production-grade orchestration patterns
- Map-reduce pattern for parallel code analysis across large codebases
- Orchestrator pattern for multi-step workflows (build -> test -> deploy)
- Durable execution ensures long-running agent tasks survive failures
- Agents can expose themselves as MCP servers, enabling recursive agent composition
- Replaces custom agent orchestration code with a battle-tested framework

**Priority**: HIGH -- directly addresses the Neural Swarm / Agent Pool requirement and has massive community adoption (8K+ stars).

---

### 8. Ultimate MCP Server

**Repository**: [Dicklesworthstone/ultimate_mcp_server](https://github.com/Dicklesworthstone/ultimate_mcp_server)
**Stars**: 142
**License**: MIT

**What it does**: A comprehensive "kitchen sink" MCP server exposing 60+ tools: multi-provider LLM delegation, browser automation, document processing, vector operations, cognitive memory systems, and database interactions.

**Key Tools** (60+):
- Multi-provider LLM routing (OpenAI, Anthropic, Google, DeepSeek, xAI)
- Browser automation (Playwright-based)
- Database interactions via SQLAlchemy
- Vector operations and semantic search
- RAG (Retrieval-Augmented Generation)
- OCR for images and PDFs
- Excel automation with VBA generation
- Structured data extraction (JSON, tables, key-value)
- Audio transcription
- Filesystem operations with security controls
- Command-line utilities (ripgrep, awk, sed, jq)
- REST API dynamic integration
- Cognitive memory hierarchy (working, episodic, semantic, procedural)

**Installation**:
```bash
# Via uv
uv pip install ultimate-mcp-server

# From source with OCR
uv pip install -e ".[ocr]"

# Docker
docker-compose up
```

**NEXUS Integration**:
- Fills multiple gaps with a single server (document processing, OCR, audio transcription)
- Multi-provider LLM delegation complements ork-offline-coding's local model routing
- Cognitive memory hierarchy adds another dimension to ork-harmony's memory system
- Could serve as a "utility belt" server for miscellaneous AI agent needs
- Some overlap with existing servers -- evaluate which tools to enable

**Priority**: LOW-MEDIUM -- many overlapping capabilities with existing NEXUS servers. Best used selectively for document processing, OCR, and audio transcription features not covered elsewhere.

---

## Recommended Integration Order

| Priority | Server | Reason |
|----------|--------|--------|
| 1 | **GitHub MCP** | 27K+ stars, official, critical for dev workflow |
| 2 | **mcp-agent** | 8K+ stars, powers NeuralSwarm agent orchestration |
| 3 | **Postgres MCP Pro** | 2.3K stars, DBA intelligence for existing PostgreSQL |
| 4 | **Grafana MCP** | 2.5K stars, enterprise observability layer |
| 5 | **Kubernetes MCP** | 1.2K stars, container orchestration for scaling |
| 6 | **Docker MCP** | 453 stars, container lifecycle management |
| 7 | **Code Pathfinder** | AST-based code intelligence, offline-first |
| 8 | **Ultimate MCP** | Utility belt for document/OCR/audio gaps |

---

## Port Allocation Plan

| Server | Proposed Port | Transport |
|--------|---------------|-----------|
| GitHub MCP | Remote or stdio | HTTPS / stdio |
| mcp-agent | Library (no port) | Python framework |
| Postgres MCP Pro | 8020 | stdio |
| Grafana MCP | 8021 | SSE / stdio |
| Kubernetes MCP | 8022 | stdio |
| Docker MCP | 8023 | stdio |
| Code Pathfinder | 8024 | stdio |
| Ultimate MCP | 8025 | SSE / HTTP |

---

## Configuration Snippets

### GitHub MCP (Remote -- no local server needed)
```json
{
  "mcpServers": {
    "github": {
      "url": "https://api.githubcopilot.com/mcp/",
      "headers": {
        "Authorization": "Bearer <GITHUB_TOKEN>"
      }
    }
  }
}
```

### GitHub MCP (Docker -- offline)
```json
{
  "mcpServers": {
    "github": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        "-e", "GITHUB_PERSONAL_ACCESS_TOKEN",
        "ghcr.io/github/github-mcp-server"
      ],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "<token>"
      }
    }
  }
}
```

### Postgres MCP Pro
```json
{
  "mcpServers": {
    "postgres-mcp": {
      "command": "postgres-mcp",
      "args": ["postgresql://orkel@localhost:5433/orkstation_archive"]
    }
  }
}
```

### Grafana MCP
```json
{
  "mcpServers": {
    "grafana": {
      "command": "mcp-grafana",
      "env": {
        "GRAFANA_URL": "http://localhost:3000",
        "GRAFANA_SERVICE_ACCOUNT_TOKEN": "<token>"
      }
    }
  }
}
```

### Kubernetes MCP
```json
{
  "mcpServers": {
    "kubernetes": {
      "command": "npx",
      "args": ["kubernetes-mcp-server@latest"]
    }
  }
}
```

---

## Ecosystem Notes

### Awesome MCP Server Directories
- [punkpeye/awesome-mcp-servers](https://github.com/punkpeye/awesome-mcp-servers) -- largest curated list
- [wong2/awesome-mcp-servers](https://github.com/wong2/awesome-mcp-servers) -- well-maintained alternative
- [modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers) -- official reference implementations
- [TensorBlock/awesome-mcp-servers](https://github.com/TensorBlock/awesome-mcp-servers) -- categorized by domain

### Additional Servers Worth Monitoring
- **[docker/mcp-gateway](https://github.com/docker/mcp-gateway)** -- Docker's official MCP gateway for unified server management
- **[awslabs MCP servers](https://awslabs.github.io/mcp/)** -- AWS-specific MCP servers (S3, Lambda, DynamoDB, etc.)
- **[qdrant/mcp-server-qdrant](https://github.com/qdrant/mcp-server-qdrant)** -- Qdrant vector DB MCP server (alternative to Pinecone)
- **[rinadelph/Agent-MCP](https://github.com/rinadelph/Agent-MCP)** -- Multi-agent coordination framework
- **[angrysky56/ast-mcp-server](https://github.com/angrysky56/ast-mcp-server)** -- AST/ASG multi-language code analysis
- **[ForLoopCodes/contextplus](https://github.com/ForLoopCodes/contextplus)** -- RAG + Tree-sitter AST + Spectral Clustering for large codebases

---

## Sources

- [GitHub MCP Server](https://github.com/github/github-mcp-server)
- [Postgres MCP Pro](https://github.com/crystaldba/postgres-mcp)
- [Grafana MCP Server](https://github.com/grafana/mcp-grafana)
- [Kubernetes MCP Server](https://github.com/containers/kubernetes-mcp-server)
- [Docker MCP Server](https://github.com/QuantGeekDev/docker-mcp)
- [Code Pathfinder](https://github.com/shivasurya/code-pathfinder)
- [mcp-agent Framework](https://github.com/lastmile-ai/mcp-agent)
- [Ultimate MCP Server](https://github.com/Dicklesworthstone/ultimate_mcp_server)
- [Official MCP Servers](https://github.com/modelcontextprotocol/servers)
- [awesome-mcp-servers](https://github.com/punkpeye/awesome-mcp-servers)
- [Top MCP Servers 2026 (Obot AI)](https://obot.ai/blog/top-15-mcp-servers/)
- [MCP Server Stack Essentials 2026](https://dev.to/techlatest-ai/the-mcp-server-stack-10-open-source-essentials-for-2026-44k8)
- [Code Pathfinder MCP](https://codepathfinder.dev/mcp)
- [Docker MCP Gateway](https://github.com/docker/mcp-gateway)
