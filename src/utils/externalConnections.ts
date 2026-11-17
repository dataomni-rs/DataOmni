import {
  DatabaseType,
  type ConnectionEnvironment,
  type ConnectionProfile,
  type SshTunnelConfig,
  type TlsMode,
} from '../contracts/connection';
import { createDefaultConfig } from '../stores/connectionStore';
import { isUsablePort } from './connectionPort';
import { ORACLE_CONNECT_BY_OPTION, ORACLE_SID } from './oracleConnection';
import { SERVER_PRESET_OPTION, serverPresetConfig, type ServerPreset } from './serverPresets';

export type ConnectionImportSource = 'dataomni' | 'navicat' | 'dbeaver';

export interface UnsupportedExternalItem {
  name: string;
  driver: string;
}

export interface ParsedConnectionImport {
  source: ConnectionImportSource;
  profiles: ConnectionProfile[];
  unsupported: UnsupportedExternalItem[];
}

const DATAOMNI_FORMAT = 'dataomni-connections';
const DATAOMNI_MAX_VERSION = 1;

function notConnectionsFile(detail: string): Error {
  return new Error(`DATAOMNI_IMPORT_NOT_CONNECTIONS_FILE: ${detail}`);
}

function parsePortOr(raw: unknown, fallback: number): number {
  const num = typeof raw === 'number' ? raw : Number.parseInt(String(raw ?? '').trim(), 10);
  return isUsablePort(num) ? num : fallback;
}

function sanitizeImportedProfile(
  raw: Omit<ConnectionProfile, 'id' | 'created_at' | 'updated_at'>,
  index: number,
): ConnectionProfile {
  const ssh = raw.ssh_tunnel
    ? {
        host: raw.ssh_tunnel.host.trim(),
        port: parsePortOr(raw.ssh_tunnel.port, 22),
        username: raw.ssh_tunnel.username.trim(),
        private_key_path: raw.ssh_tunnel.private_key_path.trim(),
        auth: raw.ssh_tunnel.auth,
        secret: '',
      }
    : undefined;

  return {
    ...raw,
    id: `import-${index}`,
    name: raw.name.trim() || `${raw.db_type}-${index + 1}`,
    host: raw.host.trim(),
    database: (raw.database ?? '').trim(),
    username: raw.username.trim(),
    password: '',
    credential_ref: undefined,
    save_password: false,
    agent_access: 'off',
    ssh_tunnel: ssh && ssh.host ? ssh : undefined,
    created_at: '',
    updated_at: '',
  };
}

/** 与后端 `ConnectionService::import_profiles` 完全一致的六元组判重 */
export function isSameConnectionTarget(
  left: Pick<ConnectionProfile, 'name' | 'db_type' | 'host' | 'port' | 'database' | 'username'>,
  right: Pick<ConnectionProfile, 'name' | 'db_type' | 'host' | 'port' | 'database' | 'username'>,
): boolean {
  return (
    left.name === right.name &&
    left.db_type === right.db_type &&
    left.host === right.host &&
    left.port === right.port &&
    (left.database ?? '') === (right.database ?? '') &&
    left.username === right.username
  );
}

// ============================================================================
// 1. XML entity & tag attribute decoder for Navicat .ncx
// ============================================================================

function decodeXmlEntities(value: string): string {
  return value.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|quot|apos|amp|lt|gt);/g, (full, entity: string) => {
    switch (entity) {
      case 'quot':
        return '"';
      case 'apos':
        return "'";
      case 'amp':
        return '&';
      case 'lt':
        return '<';
      case 'gt':
        return '>';
      default: {
        const code = entity.startsWith('#x')
          ? Number.parseInt(entity.slice(2), 16)
          : Number.parseInt(entity.slice(1), 10);
        return Number.isFinite(code) && code > 0 ? String.fromCodePoint(code) : full;
      }
    }
  });
}

function parseXmlAttributes(attrText: string): Record<string, string> {
  const attrs: Record<string, string> = {};
  const attrRegex = /([A-Za-z_][A-Za-z0-9_.:-]*)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  for (const match of attrText.matchAll(attrRegex)) {
    const [, key, doubleQuoted, singleQuoted] = match;
    attrs[key] = decodeXmlEntities(doubleQuoted ?? singleQuoted ?? '');
  }
  return attrs;
}

interface MappedEngine {
  dbType: DatabaseType;
  preset?: ServerPreset;
}

function mapNavicatConnType(connType: string): MappedEngine | null {
  switch (connType.trim().toUpperCase()) {
    case 'MYSQL':
      return { dbType: DatabaseType.MySQL };
    case 'MARIADB':
      return { dbType: DatabaseType.MySQL, preset: 'mariadb' };
    case 'PGSQL':
    case 'POSTGRESQL':
      return { dbType: DatabaseType.PostgreSQL };
    case 'ORA':
    case 'ORACLE':
      return { dbType: DatabaseType.Oracle };
    case 'MSSQL':
    case 'SQLSERVER':
      return { dbType: DatabaseType.SqlServer };
    case 'SQLITE':
      return { dbType: DatabaseType.SQLite };
    case 'MONGODB':
    case 'MONGO':
      return { dbType: DatabaseType.MongoDB };
    case 'REDIS':
      return { dbType: DatabaseType.Redis };
    default:
      return null;
  }
}

function isTruthyAttr(value: string | undefined): boolean {
  const normalized = (value ?? '').trim().toLowerCase();
  return normalized === 'true' || normalized === '1' || normalized === 'yes';
}

function parseNavicatNcx(rawXml: string): ParsedConnectionImport {
  const stripped = rawXml.replace(/^\uFEFF/, '').replace(/<!--[\s\S]*?-->/g, '').trim();
  if (!/<Connections\b[^>]*>/i.test(stripped) || !/<\/Connections\s*>/i.test(stripped)) {
    throw notConnectionsFile('invalid Navicat .ncx XML');
  }

  // Catch truncated <Connection ... without closing >
  const withoutValidTags = stripped.replace(/<\/?[A-Za-z_][A-Za-z0-9_.:-]*(?:\s+[^<>]*?)?>/g, '');
  if (withoutValidTags.includes('<Connection')) {
    throw notConnectionsFile('unclosed <Connection> tag in .ncx');
  }

  const profiles: ConnectionProfile[] = [];
  const unsupported: UnsupportedExternalItem[] = [];
  const tagRegex = /<Connection\b([^<>]*?)\/?>/gi;

  for (const match of stripped.matchAll(tagRegex)) {
    const attrs = parseXmlAttributes(match[1] ?? '');
    const name = (attrs.ConnectionName || attrs.Name || '').trim();
    const rawType = (attrs.ConnType || attrs.Type || '').trim();
    if (!rawType) continue;

    const mapped = mapNavicatConnType(rawType);
    if (!mapped) {
      unsupported.push({ name: name || rawType, driver: rawType });
      continue;
    }

    const base = mapped.preset
      ? serverPresetConfig(mapped.preset)
      : createDefaultConfig(mapped.dbType);

    const options: Record<string, string> = { ...base.options };
    if (
      mapped.dbType === DatabaseType.Oracle &&
      ((attrs.OraConnMode ?? '').toUpperCase() === 'SID' ||
        (attrs.OraServiceNameType ?? '').toUpperCase() === 'SID')
    ) {
      options[ORACLE_CONNECT_BY_OPTION] = ORACLE_SID;
    }

    const isFile = mapped.dbType === DatabaseType.SQLite || mapped.dbType === DatabaseType.DuckDB;
    const database = isFile
      ? attrs.DatabaseFileName || attrs.Database || attrs.InitialDatabase || base.database
      : attrs.Database ||
        attrs.InitialDatabase ||
        attrs.DatabaseInitial ||
        attrs.OraServiceName ||
        attrs.ServiceName ||
        base.database;

    let tlsMode: TlsMode = base.tls_mode ?? 'disabled';
    let ssl = base.ssl;
    let caCert = '';
    let clientCert = '';
    let clientKey = '';
    if (isTruthyAttr(attrs.SSL)) {
      ssl = true;
      caCert = (attrs.SSL_CACert ?? '').trim();
      const cCert = (attrs.SSL_ClientCert ?? '').trim();
      const cKey = (attrs.SSL_ClientKey ?? '').trim();
      if (cCert && cKey) {
        clientCert = cCert;
        clientKey = cKey;
      }
      tlsMode = caCert ? 'verify-ca' : 'required';
    }

    let sshTunnel: SshTunnelConfig | undefined;
    if (isTruthyAttr(attrs.SSH) && (attrs.SSH_Host ?? '').trim()) {
      const keyPath = (attrs.SSH_PrivateKey ?? '').trim();
      const methodRaw = (attrs.SSH_AuthenMethod ?? '').trim().toLowerCase();
      const useKey = methodRaw.includes('key') || (methodRaw === '' && keyPath !== '');
      sshTunnel = {
        host: (attrs.SSH_Host ?? '').trim(),
        port: parsePortOr(attrs.SSH_Port, 22),
        username: (attrs.SSH_UserName ?? '').trim(),
        private_key_path: useKey ? keyPath : '',
        auth: useKey ? 'private-key' : 'password',
        secret: '',
      };
    }

    // Navicat SQL Server sometimes stores "host,1433" in Host
    let host = isFile ? '' : (attrs.Host ?? '').trim() || base.host;
    let port = isFile ? 0 : parsePortOr(attrs.Port, base.port);
    if (mapped.dbType === DatabaseType.SqlServer && host.includes(',') && !attrs.Port) {
      const [h, p] = host.split(',', 2);
      host = h.trim();
      port = parsePortOr(p, port);
    }

    profiles.push(
      sanitizeImportedProfile(
        {
          ...base,
          name: name || host || String(database ?? mapped.dbType),
          host,
          port,
          database,
          username: isFile ? '' : (attrs.UserName || attrs.Username || '').trim() || base.username,
          ssl,
          tls_mode: tlsMode,
          ca_certificate_path: caCert,
          client_certificate_path: clientCert,
          client_key_path: clientKey,
          options,
          ssh_tunnel: sshTunnel,
        },
        profiles.length,
      ),
    );
  }

  return { source: 'navicat', profiles, unsupported };
}

// ============================================================================
// 2. DBeaver data-sources.json & JDBC URL fallback
// ============================================================================

function mapDbeaverEngine(provider: string, driver: string): MappedEngine | null {
  const combined = `${provider} ${driver}`.toLowerCase();
  if (combined.includes('cockroach')) {
    return { dbType: DatabaseType.PostgreSQL, preset: 'cockroachdb' };
  }
  if (combined.includes('tidb')) {
    return { dbType: DatabaseType.MySQL, preset: 'tidb' };
  }
  if (combined.includes('maria')) {
    return { dbType: DatabaseType.MySQL, preset: 'mariadb' };
  }
  if (combined.includes('postgres') || combined.includes('opengauss')) {
    return { dbType: DatabaseType.PostgreSQL };
  }
  if (combined.includes('mysql') || combined.includes('oceanbase')) {
    return { dbType: DatabaseType.MySQL };
  }
  if (combined.includes('oracle')) {
    return { dbType: DatabaseType.Oracle };
  }
  if (combined.includes('sqlserver') || combined.includes('mssql') || combined.includes('jtds')) {
    return { dbType: DatabaseType.SqlServer };
  }
  if (combined.includes('duckdb')) {
    return { dbType: DatabaseType.DuckDB };
  }
  if (combined.includes('sqlite')) {
    return { dbType: DatabaseType.SQLite };
  }
  if (combined.includes('clickhouse')) {
    return { dbType: DatabaseType.ClickHouse };
  }
  if (combined.includes('mongo')) {
    return { dbType: DatabaseType.MongoDB };
  }
  if (combined.includes('redis') || combined.includes('valkey')) {
    return { dbType: DatabaseType.Redis };
  }
  if (combined.includes('neo4j')) {
    return { dbType: DatabaseType.Neo4j };
  }
  if (combined.includes('elastic') || combined.includes('opensearch')) {
    return { dbType: DatabaseType.Elasticsearch };
  }
  return null;
}

interface UrlEndpoint {
  host?: string;
  port?: number;
  database?: string;
  oracleSid?: boolean;
}

function parseJdbcOrEndpointUrl(rawUrl: string, dbType: DatabaseType): UrlEndpoint {
  const trimmed = rawUrl.trim();
  if (!trimmed) return {};
  const withoutJdbc = trimmed.replace(/^jdbc:/i, '');

  if (dbType === DatabaseType.SQLite || dbType === DatabaseType.DuckDB) {
    const fileMatch = withoutJdbc.match(/^(?:sqlite|duckdb):(.*)$/i);
    if (fileMatch) {
      const filePath = fileMatch[1].split('?')[0].trim();
      return filePath ? { database: filePath } : {};
    }
  }

  if (dbType === DatabaseType.Oracle) {
    // jdbc:oracle:thin:@//host:1521/service or jdbc:oracle:thin:@host:1521:SID
    const serviceMatch = withoutJdbc.match(/@\/\/([^:/?]+)(?::(\d+))?\/([^?;]+)/i);
    if (serviceMatch) {
      return {
        host: serviceMatch[1],
        port: serviceMatch[2] ? Number.parseInt(serviceMatch[2], 10) : undefined,
        database: serviceMatch[3],
      };
    }
    const sidMatch = withoutJdbc.match(/@([^:/?]+):(\d+):([^?;]+)/i);
    if (sidMatch) {
      return {
        host: sidMatch[1],
        port: Number.parseInt(sidMatch[2], 10),
        database: sidMatch[3],
        oracleSid: true,
      };
    }
  }

  if (dbType === DatabaseType.SqlServer) {
    // jdbc:sqlserver://host:1433;databaseName=mydb
    const msMatch = withoutJdbc.match(/^sqlserver:\/\/([^:/;]+)(?::(\d+))?(?:;(.*))?$/i);
    if (msMatch) {
      const params = msMatch[3] ?? '';
      const dbParam = params.match(/(?:databaseName|database)=([^;]+)/i);
      return {
        host: msMatch[1],
        port: msMatch[2] ? Number.parseInt(msMatch[2], 10) : undefined,
        database: dbParam?.[1],
      };
    }
  }

  const genericMatch = withoutJdbc.match(/^[a-z0-9+-]+:\/\/([^/?#@]+)(?:\/([^?;#]*))?/i);
  if (genericMatch) {
    const hostPort = genericMatch[1];
    const colonIdx = hostPort.lastIndexOf(':');
    const host = colonIdx > 0 ? hostPort.slice(0, colonIdx) : hostPort;
    const maybePort = colonIdx > 0 ? Number.parseInt(hostPort.slice(colonIdx + 1), 10) : undefined;
    return {
      host,
      port: isUsablePort(maybePort ?? 0) ? maybePort : undefined,
      database: genericMatch[2] ? decodeURIComponent(genericMatch[2]) : undefined,
    };
  }

  return {};
}

function mapDbeaverEnvironment(rawType: unknown): ConnectionEnvironment {
  switch (String(rawType ?? '').trim().toLowerCase()) {
    case 'prod':
    case 'production':
      return 'production';
    case 'stage':
    case 'staging':
      return 'staging';
    case 'test':
    case 'qa':
      return 'testing';
    default:
      return 'development';
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function parseDbeaverJson(root: Record<string, unknown>): ParsedConnectionImport {
  const connections = asRecord(root.connections);
  if (!connections) {
    throw notConnectionsFile('missing DBeaver connections object');
  }

  const profiles: ConnectionProfile[] = [];
  const unsupported: UnsupportedExternalItem[] = [];

  for (const [entryId, rawEntry] of Object.entries(connections)) {
    const entry = asRecord(rawEntry);
    if (!entry) continue;

    const provider = String(entry.provider ?? '').trim();
    const driver = String(entry.driver ?? '').trim();
    const name = String(entry.name ?? entryId).trim();
    const mapped = mapDbeaverEngine(provider, driver);
    if (!mapped) {
      const driverDesc = [provider, driver].filter(Boolean).join(' / ') || 'unknown';
      unsupported.push({ name: name || entryId, driver: driverDesc });
      continue;
    }

    const base = mapped.preset
      ? serverPresetConfig(mapped.preset)
      : createDefaultConfig(mapped.dbType);
    const cfg = asRecord(entry.configuration) ?? {};
    const urlInfo = parseJdbcOrEndpointUrl(String(cfg.url ?? ''), mapped.dbType);
    const isFile = mapped.dbType === DatabaseType.SQLite || mapped.dbType === DatabaseType.DuckDB;

    const options: Record<string, string> = { ...base.options };
    if (mapped.preset) {
      options[SERVER_PRESET_OPTION] = mapped.preset;
    }
    const providerProps = asRecord(cfg['provider-properties']);
    if (
      mapped.dbType === DatabaseType.Oracle &&
      (urlInfo.oracleSid ||
        String(providerProps?.['@dbeaver-sid-service@'] ?? '').toUpperCase() === 'SID')
    ) {
      options[ORACLE_CONNECT_BY_OPTION] = ORACLE_SID;
    }

    const host = isFile ? '' : String(cfg.host ?? urlInfo.host ?? base.host).trim();
    const port = isFile ? 0 : parsePortOr(cfg.port ?? urlInfo.port, base.port);
    const database = String(cfg.database ?? urlInfo.database ?? base.database ?? '').trim();
    const username = isFile ? '' : String(cfg.user ?? cfg.username ?? base.username).trim();

    let sshTunnel: SshTunnelConfig | undefined;
    const handlers = asRecord(cfg.handlers);
    const sshHandler = asRecord(handlers?.ssh_tunnel);
    if (sshHandler && sshHandler.enabled === true) {
      const props = asRecord(sshHandler.properties) ?? {};
      const sshHost = String(props.host ?? '').trim();
      if (sshHost) {
        const authType = String(props.authType ?? '').toUpperCase();
        const keyPath = String(props.keyPath ?? props.privateKey ?? '').trim();
        const useKey = authType.includes('KEY') || (authType === '' && keyPath !== '');
        sshTunnel = {
          host: sshHost,
          port: parsePortOr(props.port, 22),
          username: String(props.user ?? props.userName ?? sshHandler.user ?? '').trim(),
          private_key_path: useKey ? keyPath : '',
          auth: useKey ? 'private-key' : 'password',
          secret: '',
        };
      }
    }

    profiles.push(
      sanitizeImportedProfile(
        {
          ...base,
          name: name || host || database || entryId,
          host,
          port,
          database,
          username,
          environment: mapDbeaverEnvironment(cfg.type ?? entry['connection-type']),
          options,
          ssh_tunnel: sshTunnel,
        },
        profiles.length,
      ),
    );
  }

  return { source: 'dbeaver', profiles, unsupported };
}

// ============================================================================
// 3. Native DataOmni export JSON & unified entry point
// ============================================================================

function parseDataOmniJson(root: Record<string, unknown>): ParsedConnectionImport {
  if (root.format !== DATAOMNI_FORMAT) {
    throw notConnectionsFile(String(root.format ?? 'unknown'));
  }
  const version = typeof root.version === 'number' ? root.version : Number.NaN;
  if (!Number.isInteger(version) || version < 1) {
    throw notConnectionsFile('invalid version');
  }
  if (version > DATAOMNI_MAX_VERSION) {
    throw new Error(`DATAOMNI_IMPORT_NEWER_VERSION: ${version}`);
  }
  if (!Array.isArray(root.connections)) {
    throw notConnectionsFile('connections must be an array');
  }

  const validTypes = new Set<string>(Object.values(DatabaseType));
  const profiles = root.connections.map((item, idx) => {
    const record = asRecord(item);
    if (!record || !validTypes.has(String(record.db_type ?? ''))) {
      throw notConnectionsFile(`invalid connection at index ${idx}`);
    }
    const dbType = record.db_type as DatabaseType;
    const base = createDefaultConfig(dbType);
    const rawSsh = asRecord(record.ssh_tunnel);
    const sshTunnel: SshTunnelConfig | undefined =
      rawSsh && String(rawSsh.host ?? '').trim()
        ? {
            host: String(rawSsh.host ?? '').trim(),
            port: parsePortOr(rawSsh.port, 22),
            username: String(rawSsh.username ?? '').trim(),
            private_key_path: String(rawSsh.private_key_path ?? '').trim(),
            auth: rawSsh.auth === 'private-key' ? 'private-key' : 'password',
            secret: '',
            remote_host: rawSsh.remote_host ? String(rawSsh.remote_host) : undefined,
            remote_port:
              typeof rawSsh.remote_port === 'number' ? rawSsh.remote_port : undefined,
          }
        : undefined;

    const envRaw = String(record.environment ?? base.environment);
    const environment: ConnectionEnvironment =
      envRaw === 'production' ||
      envRaw === 'staging' ||
      envRaw === 'testing' ||
      envRaw === 'development'
        ? envRaw
        : 'development';

    const tlsRaw = String(record.tls_mode ?? base.tls_mode ?? 'disabled');
    const tlsMode: TlsMode =
      tlsRaw === 'disabled' ||
      tlsRaw === 'preferred' ||
      tlsRaw === 'required' ||
      tlsRaw === 'verify-ca' ||
      tlsRaw === 'verify-full'
        ? tlsRaw
        : base.tls_mode ?? 'disabled';

    const rawOptions = asRecord(record.options);
    const options: Record<string, string> = {};
    if (rawOptions) {
      for (const [k, v] of Object.entries(rawOptions)) {
        if (typeof v === 'string') options[k] = v;
      }
    }

    return sanitizeImportedProfile(
      {
        ...base,
        name: String(record.name ?? '').trim(),
        db_type: dbType,
        host: String(record.host ?? base.host),
        port:
          dbType === DatabaseType.SQLite || dbType === DatabaseType.DuckDB
            ? 0
            : parsePortOr(record.port, base.port),
        database: String(record.database ?? base.database ?? ''),
        username: String(record.username ?? base.username),
        ssl: typeof record.ssl === 'boolean' ? record.ssl : tlsMode !== 'disabled',
        tls_mode: tlsMode,
        ca_certificate_path: String(record.ca_certificate_path ?? ''),
        client_certificate_path: String(record.client_certificate_path ?? ''),
        client_key_path: String(record.client_key_path ?? ''),
        options,
        tags: Array.isArray(record.tags)
          ? record.tags.filter((tag): tag is string => typeof tag === 'string')
          : [],
        environment,
        ssh_tunnel: sshTunnel,
      },
      idx,
    );
  });

  return { source: 'dataomni', profiles, unsupported: [] };
}

export function parseConnectionImportFile(rawText: string): ParsedConnectionImport {
  const trimmed = rawText.replace(/^\uFEFF/, '').trim();
  if (!trimmed) {
    throw notConnectionsFile('empty file');
  }

  if (trimmed.startsWith('<')) {
    return parseNavicatNcx(trimmed);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    throw notConnectionsFile(error instanceof Error ? error.message : String(error));
  }

  const root = asRecord(parsed);
  if (!root) {
    throw notConnectionsFile('JSON root must be an object');
  }

  if ('format' in root) {
    return parseDataOmniJson(root);
  }

  if ('connections' in root && asRecord(root.connections)) {
    return parseDbeaverJson(root);
  }

  throw notConnectionsFile('unrecognized JSON schema');
}
