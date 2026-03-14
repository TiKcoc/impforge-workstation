# Contributing to ImpForge

Thank you for your interest in contributing to ImpForge!

## Development Setup

### Prerequisites
- **Rust** (stable, 1.77+)
- **Node.js** (LTS, 20+)
- **pnpm** (9+)
- **Linux**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

### Quick Start
```bash
git clone https://github.com/TiKcoc/impforge-workstation.git
cd impforge-workstation
pnpm install
pnpm tauri dev
```

### Checks Before Submitting
```bash
pnpm check          # Svelte type checking
cd src-tauri
cargo check          # Rust compilation
cargo test           # Rust tests
cargo clippy         # Lint
```

## Commit Convention

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `refactor:` Code restructuring
- `perf:` Performance improvement
- `test:` Adding tests
- `chore:` Build, CI, deps

## Pull Requests

1. Fork the repo and create your branch from `main`
2. Make your changes
3. Ensure all checks pass
4. Submit a PR using the template

## Architecture

See [docs/dev/DEVELOPER.md](docs/dev/DEVELOPER.md) for architecture details.

## Code of Conduct

Be respectful. We're building something cool together.
