---
name: dataomni
description: "Query and inspect databases configured in DataOmni through its headless read-only `dataomni` CLI (and `dataomni mcp`). Use whenever a task needs schema discovery, whole-database Markdown data dictionaries, read-only SQL with named bind parameters, execution plans, built-in DuckDB column profiling, cross-database schema diffs, CSV/JSON/Parquet exports, backups, or MongoDB / Redis / Neo4j / Elasticsearch reads."
---

# DataOmni CLI Skill

`dataomni <command>` runs headless without launching or requiring the desktop GUI, reusing the connections, SSH tunnels, TLS settings, and OS keychain credentials already configured in DataOmni. It is strictly read-only.

- `dataomni cli <command>` is an identical backward-compatible alias for `dataomni <command>`.
- Bare `dataomni` (no arguments) launches the desktop application — never run it without a subcommand from an agent.
- If `dataomni` is not on `PATH`:
  - **macOS**: `/Applications/DataOmni.app/Contents/MacOS/dataomni` (Homebrew Cask links `$(brew --prefix)/bin/dataomni`)
  - **Linux**: `/usr/bin/dataomni`
  - **Windows**: `%LOCALAPPDATA%\DataOmni\dataomni.exe` (NSIS) or `C:\Program Files\DataOmni\dataomni.exe` (MSI); if you chose a custom install directory, locate `dataomni.exe` inside that folder.

## Access Rules & Security Guardrails

1. **Explicit opt-in only**: Only non-production connections set to **"Command line & agent access: Read-only"** in DataOmni are visible.
2. **Production is physically invisible**: Production (`production`) connections never appear in `connections` and always return `not_found`.
3. **Never bypass or ask for passwords**: Credentials are read directly from the OS keychain and grants are verified via HMAC-SHA256 (`<id>#agent`). Never edit `connections.json`, never ask the user for a password, and never invent flags to widen permissions. When a connection returns `not_found`, ask the user to open it as Read-only in DataOmni.

## Recommended Agent Workflow

Follow this order to avoid guessing table/column names or pulling excessive rows:

### 1. Discover open connections
```bash
dataomni connections
```
Use the returned `name` or `id`, and check `type` (relational vs `mongodb` / `redis` / `neo4j` / `elasticsearch`) and `ssh_tunnel`.

### 2. Inspect structure before writing SQL
Never guess column names (e.g. assuming `id` exists). Read the schema first:

```bash
# Whole-database Markdown data dictionary (tables, columns, types, PKs, FKs, and local semantics notes)
dataomni dictionary <connection>

# List all tables, views, and routines
dataomni schema <connection>

# Inspect one table's columns, primary key, indexes, foreign keys, and table semantics
dataomni schema <connection> <table> [--schema S]

# Database engine's native CREATE definition (SHOW CREATE TABLE, sqlite_master, DBMS_METADATA, etc.)
# Note: PostgreSQL provides native DDL for views only; use `schema` for tables.
dataomni ddl <connection> <table> [--schema S]
```

### 3. Profile distributions or aggregate before pulling raw rows
```bash
# Column-level profile via built-in sandboxed DuckDB (type, null ratio, distinct count, min/max/quantiles, top-5)
# Default 10,000 rows (--limit up to 100,000)
dataomni profile <connection> "SELECT * FROM orders" --limit 10000

# Profile a local Parquet, CSV, or JSON file directly (offline, no DB connection needed)
dataomni profile --file ./report.parquet
```

### 4. Run read-only SQL with named bind parameters
```bash
# Basic query (default 200 rows, --limit up to 10000, default 30 s --timeout up to 600 s)
dataomni query <connection> "SELECT status, COUNT(*) AS n FROM orders GROUP BY status" --limit 50

# Safe :name bind parameters (bound natively via prepared statements; values are never logged)
dataomni query <connection> "SELECT * FROM orders WHERE id = :id AND deleted_at IS :del" \
  --param id=42 --param-null del

# Bind parameters from JSON (useful for repeated placeholders or typed values)
dataomni query <connection> "SELECT * FROM orders WHERE tenant_id = :t OR parent_id = :t" \
  --params-json '{"t": 1001}'

# Read long multi-line SQL from stdin using `-`
dataomni query <connection> - < query.sql
```

- On **Windows PowerShell**, `<` stdin redirection is not supported and the GUI-subsystem binary returns immediately unless output is piped; pipe SQL via `Get-Content -Raw query.sql | dataomni query <connection> - | Out-String` (or `Set-Content -Encoding utf8` when saving Markdown/JSON files).
- Check `truncated: true` in the response to know whether more rows existed beyond `--limit`.
- Only **one** read-only statement is allowed per call (`SELECT`, read-only `WITH`, `SHOW`, `DESCRIBE`, `EXPLAIN` without `ANALYZE`, `VALUES`, `TABLE`). Writes, DDL, and multi-statements are refused with exit code `3`.

### 5. Execution plans, schema diff, exports, backups & diagnostics
```bash
# Estimated execution plan tree (plan.roots) + raw text (plan.raw); never runs ANALYZE
dataomni explain <connection> "SELECT * FROM orders WHERE customer_id = 7"

# Read-only schema diff between two open non-production connections (never executes DDL)
dataomni diff <source-connection> <target-connection> [--format json|markdown]

# Stream all rows to a brand-new CSV, JSON, or Apache Parquet file (never overwrites; default 300 s timeout)
dataomni export <connection> "SELECT * FROM orders" --out ./orders.parquet --format parquet
dataomni export <connection> "SELECT * FROM orders" --out ./orders.csv --delimiter "," --bom

# Read-only logical/file backup (pg_dump, mysqldump, mongodump, SQLite VACUUM INTO, DuckDB EXPORT DATABASE)
dataomni backup <connection> --out ./backup.dump [--database D]

# Offline CSV delimiter/encoding sniffer (UTF-8 / GB18030) and ragged-row detector
dataomni csv-preview ./import.csv [--delimiter C] [--no-header] [--rows 50]

# Connectivity probe (attaches DNS/TCP error.diagnosis on network failure)
dataomni test <connection>
```

## Non-Relational Engines (NoSQL)

Run any subcommand without an operation to see its full usage.

- **MongoDB** (`mongosh` filter/pipeline syntax; relaxed Extended JSON output; `$out` and `$merge` refused at parse time):
  ```bash
  dataomni mongo <connection> collections
  dataomni mongo <connection> find <db.collection> '{ status: "open" }' --sort '{ createdAt: -1 }' --limit 20
  dataomni mongo <connection> count <db.collection> '{ status: "open" }'
  dataomni mongo <connection> aggregate <db.collection> '[{ $group: { _id: "$status", n: { $sum: 1 } } }]'
  dataomni mongo <connection> explain <db.collection> '{ status: "open" }'
  dataomni mongo <connection> structure <db.collection>
  ```
- **Redis** (`--db N` goes **before** the operation; `command` verifies the server's `COMMAND INFO` `readonly` flag):
  ```bash
  dataomni redis <connection> keyspaces
  dataomni redis <connection> --db 0 scan "session:*" --limit 50
  dataomni redis <connection> --db 0 get "session:42"
  dataomni redis <connection> --db 0 command HGETALL "user:42"
  ```
- **Neo4j** (`EXPLAIN` verifies query type `r` before executing in a Bolt read-mode transaction):
  ```bash
  dataomni neo4j <connection> labels [--db neo4j]
  dataomni neo4j <connection> run "MATCH (u:User) RETURN u.id, u.name LIMIT 25" [--db neo4j]
  ```
- **Elasticsearch / OpenSearch** (segment-checked whitelist: `GET`/`HEAD` excluding cluster-mutating admin endpoints, and search `POST` such as `_search`, `_count`, `_msearch`; `PUT`/`DELETE` never allowed):
  ```bash
  dataomni es <connection> indices
  dataomni es <connection> request POST /orders/_search '{"size":10,"query":{"match_all":{}}}'
  ```

## SSH Tunnels & High-Frequency Calls

Each standalone `dataomni <command>` invocation opens and gracefully closes its own SSH tunnel. Firing 10+ consecutive CLI commands in seconds against an SSH jump host can trigger server-side SSH rate limits (`PerSourcePenalties` / `MaxStartups`).

- For multi-step agent loops over SSH-tunneled connections, prefer **`dataomni mcp`** (which reuses a single persistent SSH tunnel for the entire stdio session) or batch discovery with `dictionary` / `diff` / SQL aggregations instead of per-table CLI loops.

## Output Format, Display Flags & Exit Codes

- **Piped / Agent subprocesses**: stdout and stderr are always compact single-line plain JSON with top-level `"schema": 1` and zero ANSI escape codes (`dictionary` and `diff --format markdown` emit plain Markdown).
- **Interactive terminals (TTY)**: stdout and stderr automatically pretty-print JSON (2-space indent) with ANSI syntax highlighting, and lightly highlight Markdown.
- **Global display overrides**: `--pretty`, `--compact` (`-c`), `--color auto|always|never`, `--no-color` (also respects `NO_COLOR`, `CLICOLOR=0`, `CLICOLOR_FORCE=1`, `FORCE_COLOR=1`, `TERM=dumb`).

Errors are written to stderr as `{"schema": 1, "error": {"kind": "...", "message": "..."}}`.

| Exit | Meaning | Agent action |
| --- | --- | --- |
| `0` | Success | Parse stdout JSON (or Markdown for `dictionary` / `diff --format markdown`). |
| `1` | Database error or timeout | Read `error.message`; fix SQL syntax, column/table names, or tighten filters. |
| `2` | Usage error or `not_found` | Fix flags, or ask the user to open the connection as Read-only in DataOmni. |
| `3` | Refused by read-only gate | Stop immediately — write/DDL/multi-statements are never allowed and no flag bypasses this. |
| `4` | Unavailable / keychain waiting | Inspect `error.diagnosis` for network failures, or ask the user to click **"Always Allow"** in the macOS Keychain prompt (10 s deadline). |

Every command invocation is audit-logged in DataOmni's log directory with timestamp, connection name, command, statement keyword, length, SHA-256 hash, row count, and duration; raw SQL text and bind parameter values are never logged.
