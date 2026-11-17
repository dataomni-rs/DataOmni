# Ecosystem Directory Submission Metadata

Canonical English metadata, security-boundary descriptions, and copy-paste listing snippets for submitting DataOmni to MCP server registries and community `awesome-*` lists. Keep every claim synchronized with `rfcs/mcp-server.md`, `rfcs/agent-cli.md`, and `https://dataomni.dev/docs/mcp/`.

---

## 1. MCP Server Directories (`modelcontextprotocol/servers`, Smithery, Glama)

### Basic Metadata

- **Name**: DataOmni
- **Short Summary (<= 120 chars)**: Local stdio MCP server for read-only access to SQL, MongoDB, Redis, Neo4j, and Elasticsearch with OS keychain auth.
- **Website / Documentation**: `https://dataomni.dev/docs/mcp/`
- **Source Repository**: `https://github.com/dataomni-rs/DataOmni`
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

macOS:

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

One-line registration for `modelcontextprotocol/servers` README (Community Servers):

```markdown
- **[DataOmni](https://github.com/dataomni-rs/DataOmni)** - Native desktop database client with a built-in read-only stdio MCP server across PostgreSQL, MySQL, Oracle, SQLite, DuckDB, ClickHouse, MongoDB, Redis, Neo4j, and Elasticsearch, backed by OS keychain secrets and physical production profile isolation.
```

---

## 2. `awesome-tauri` (`tauri-apps/awesome-tauri` -> Applications -> Developer Tools)

```markdown
- [DataOmni](https://github.com/dataomni-rs/DataOmni) - Native cross-platform client for PostgreSQL, MySQL, SQLite, SQL Server, Oracle, DuckDB, ClickHouse, MongoDB, Redis, Neo4j, and Elasticsearch with OS keychain storage, previewed SQL transactions, schema diff, and a built-in read-only MCP server. ([Website](https://dataomni.dev))
```

---

## 3. `awesome-rust` (`rust-unofficial/awesome-rust` -> Applications -> Database)

```markdown
- [DataOmni](https://github.com/dataomni-rs/DataOmni) - Cross-platform desktop database client (Tauri 2 + Rust) supporting 7 relational drivers and dedicated MongoDB, Redis, Neo4j, and Elasticsearch workspaces, with OS keychain credentials and read-only CLI/MCP interfaces. ([Website](https://dataomni.dev))
```
