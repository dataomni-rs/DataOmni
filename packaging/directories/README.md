# Ecosystem Directory Submission Guidelines & Metadata

Canonical English metadata, audited upstream submission rules (2026-11), hard eligibility gates, and copy-paste snippets for DataOmni. Keep every capability claim synchronized with `rfcs/mcp-server.md`, `rfcs/agent-cli.md`, `https://dataomni.dev/docs/mcp/`, and `https://dataomni.dev/privacy/`.

---

## 0. Pre-Submission Checklist (Before Opening Directory PRs)

1. **Glama Organization Ownership File (`glama.json`)**: Because the repository lives under the `dataomni-rs` GitHub organization rather than a personal account, add a root `glama.json` listing the maintainer's personal GitHub username (`{"$schema": "https://glama.ai/mcp/schemas/server.json", "maintainers": ["<github-username>"]}`) and exempt `glama.json` in `.github/workflows/ci.yml` before claiming the server on Glama.
2. **Submission Sequence & Star Gates**:
   - **Zero-star channels (ready anytime)**: Search Console / IndexNow -> Glama.ai -> `punkpeye/awesome-mcp-servers` -> Official MCP Registry (`registry.modelcontextprotocol.io`) / Smithery -> `chrysler-stone/awesome-tauri-apps` & Tauri Discord `#showcase`.
   - **Gated channels (wait until threshold is met to avoid automated closure)**:
     - `microsoft/winget-pkgs`: Wait until initial package PR `#481999` merges before opening a `1.2.2+` update PR via `scripts/winget-manifest.sh`.
     - `rust-unofficial/awesome-rust`: Requires **`> 50` GitHub stars** (or `> 2,000` crates.io downloads).
     - `Homebrew/homebrew-cask`: Requires `dataomni.dev` domain age **`>= 30` days** AND GitHub stars **`>= 75`** (`>= 225` when self-submitted by a maintainer).

---

## 1. MCP Server Directories & Registries

> **Upstream Rule Note**: `modelcontextprotocol/servers` only houses official reference implementations and **no longer accepts community PRs to its `README.md`**. Community discovery now goes through **Glama -> `punkpeye/awesome-mcp-servers`** and the **Official MCP Registry (`registry.modelcontextprotocol.io`)**.

### Basic Metadata

- **Name**: DataOmni
- **Reverse-DNS Identifier (`server.json`)**: `io.github.dataomni-rs/dataomni`
- **Short Summary (<= 120 chars)**: Local stdio MCP server for read-only access to SQL, MongoDB, Redis, Neo4j, and Elasticsearch with OS keychain auth.
- **Website / Documentation**: `https://dataomni.dev/docs/mcp/`
- **Privacy & Security Policy**: `https://dataomni.dev/privacy/`
- **Source Repository**: `https://github.com/dataomni-rs/DataOmni`
- **Homebrew Tap (macOS Notarized)**: `brew install --cask dataomni-rs/tap/dataomni`
- **License**: `MIT`
- **Transport**: `stdio` (`JSON-RPC 2.0`, protocol versions `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`)
- **Runtime**: Native Rust binary (`dataomni mcp`), zero Node.js / Python / Docker wrapper required

### Long Description

DataOmni is an MIT-licensed native desktop database client (Tauri 2 + Rust) with a built-in `dataomni mcp` server over `stdio`. Instead of pasting database passwords into agent config files or spinning up one MCP server per database engine, developers configure connections, SSH jump-host tunnels, and TLS certificates once in DataOmni (secrets stored exclusively in macOS Keychain, Windows Credential Manager, or Linux Secret Service) and opt specific non-production profiles into read-only agent access.

### Security Model (4 Enforced Invariants)

1. **Opt-in per connection with HMAC keychain binding**: Every connection defaults to agent access `off`. Enabling `read-only` in the DataOmni GUI signs an `HMAC-SHA256` grant (`<id>#agent`) into the OS keychain bound to the database secret and endpoint — hand-editing `connections.json` on disk cannot unlock a password-protected profile.
2. **Physical Production invisibility**: Profiles marked `Production` lock agent access off, omit any keychain grant, never appear in `connections`, and return `not_found` if queried by name.
3. **Two-barrier read-only execution**: Every SQL call accepts at most one read statement (`SELECT`, `WITH` without nested writes, `SHOW`, `DESCRIBE`, `EXPLAIN` without `ANALYZE`, `VALUES`, `TABLE`) AND runs inside a database-level read-only transaction (`BEGIN READ ONLY`, `START TRANSACTION READ ONLY`, `SET TRANSACTION READ ONLY`, `readonly=1`, or read-only file handle with DuckDB external file access locked off) that always ends in `ROLLBACK`.
4. **Zero host filesystem tools**: File-reading and file-writing CLI commands (`export`, `backup`, `csv-preview`) are excluded from `tools/list` so non-shell MCP clients cannot read or write host files.

### Exposed Tools (11, all `readOnlyHint: true`)

| Tool | Purpose |
| --- | --- |
| `connections` | List non-production connections explicitly opened to agents |
| `dictionary` | Export a whole-database Markdown data dictionary (tables, columns, types, PKs, FKs) |
| `schema` | List tables/views or inspect columns, indexes, and foreign keys of a table |
| `ddl` | Return the database engine's canonical `CREATE` definition |
| `query` | Run one read-only SQL statement (default 200 rows, max 10,000) |
| `explain` | Return estimated execution plan without running `ANALYZE` |
| `test` | Test connectivity and report DNS / TCP port diagnosis on failure |
| `mongo` | Read-only MongoDB (`collections`, `find`, `count`, `aggregate` blocking `$out`/`$merge`, `explain`, `structure`) |
| `redis` | Read-only Redis (`keyspaces`, `scan`, `get`, and server-verified `readonly` `command`) |
| `neo4j` | Read-only Neo4j Cypher (`labels`, `run` verified via `EXPLAIN` query type `r` + Bolt read routing) |
| `es` | Read-only Elasticsearch / OpenSearch (`indices`, segment-checked `GET`/`HEAD` and search `POST`) |

### Standard `mcpServers` Configuration

macOS (Homebrew Cask `brew install --cask dataomni-rs/tap/dataomni`, Apple Silicon):

```json
{
  "mcpServers": {
    "dataomni": {
      "command": "/opt/homebrew/bin/dataomni",
      "args": ["mcp"]
    }
  }
}
```

macOS (Standalone `.dmg` in `/Applications`):

```json
{
  "mcpServers": {
    "dataomni": {
      "command": "/Applications/DataOmni.app/Contents/MacOS/dataomni",
      "args": ["mcp"]
    }
  }
}
```

Linux (`.deb` / `.rpm`):

```json
{
  "mcpServers": {
    "dataomni": {
      "command": "/usr/bin/dataomni",
      "args": ["mcp"]
    }
  }
}
```

Windows (NSIS per-user install):

```json
{
  "mcpServers": {
    "dataomni": {
      "command": "C:\\Users\\<USER>\\AppData\\Local\\DataOmni\\dataomni.exe",
      "args": ["mcp"]
    }
  }
}
```

### `punkpeye/awesome-mcp-servers` Entry (`🗄️ Databases`, Alphabetical Order)

> **Gate**: Submit and claim `https://github.com/dataomni-rs/DataOmni` on `https://glama.ai/mcp/servers` first so `check-glama.yml` passes. Legend icons: `🦀` Rust, `🏠` Local service, `🍎` macOS, `🪟` Windows, `🐧` Linux.

```markdown
- [dataomni-rs/DataOmni](https://github.com/dataomni-rs/DataOmni) 🦀 🏠 🍎 🪟 🐧 - Native desktop database client with a built-in read-only stdio MCP server across PostgreSQL, MySQL, Oracle, SQLite, DuckDB, ClickHouse, MongoDB, Redis, Neo4j, and Elasticsearch, backed by OS keychain secrets and physical production isolation.
```

---

## 2. Tauri Showcase (`chrysler-stone/awesome-tauri-apps` & Tauri Discord `#showcase`)

> **Upstream Rule Note**: Official `tauri-apps/awesome-tauri` removed its `Applications` showcase section (now restricted to plugins, templates, integrations, and guides). Strict Tauri awesome-list linter rules still worth keeping: alphabetical order, **under 24 words**, **no leading `A`/`An`**, and **no parentheses or nested links** in the description.

```markdown
- [DataOmni](https://github.com/dataomni-rs/DataOmni) - Native client for SQL, MongoDB, Redis, Neo4j, and Elasticsearch with keychain credentials, previewed transactions, and read-only MCP.
```

---

## 3. `rust-unofficial/awesome-rust` (`Applications -> Database`)

> **Hard Gate**: Requires **`> 50` GitHub stars** (or `> 2,000` crates.io downloads). Entry title **must** use `[ACCOUNT/REPO](https://github.com/ACCOUNT/REPO)` in strict alphabetical order (`dataomni-rs/DataOmni`).

```markdown
- [dataomni-rs/DataOmni](https://github.com/dataomni-rs/DataOmni) - Cross-platform desktop database client supporting 7 relational drivers and dedicated MongoDB, Redis, Neo4j, and Elasticsearch workspaces, with OS keychain credentials and read-only CLI/MCP interfaces.
```
