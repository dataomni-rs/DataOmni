import { describe, expect, it } from 'vitest';
import { DatabaseType, type ConnectionProfile } from '../contracts/connection';
import { createDefaultConfig } from '../stores/connectionStore';
import {
  isSameConnectionTarget,
  parseConnectionImportFile,
} from './externalConnections';

const NAVICAT_SAMPLE = `<?xml version="1.0" encoding="UTF-8"?>
<Connections Ver="2.0">
  <Connection
    ConnectionName="Orders &amp; Billing (PG)"
    ConnType="PGSQL"
    Host="db.internal"
    Port="5433"
    Database="orders"
    UserName="readonly_app"
    SavePassword="true"
    Password="9F81B3ENCRYPTEDHEX"
    SSH="true"
    SSH_Host="jump.internal"
    SSH_Port="2222"
    SSH_UserName="deploy"
    SSH_AuthenMethod="PublicKey"
    SSH_PrivateKey="~/.ssh/id_ed25519"
    SSH_SavePassword="true"
    SSH_Password="SSHENCRYPTEDHEX"
    SSL="true"
    SSL_CACert="/etc/ssl/root.pem"
  />
  <Connection
    ConnectionName="Legacy ERP"
    ConnType="ORACLE"
    Host="ora.corp"
    Port="1521"
    InitialDatabase="ORCLSID"
    OraConnMode="SID"
    UserName="system"
    Password="ENCRYPTED"
  />
  <Connection
    ConnectionName="Reporting Maria"
    ConnType="MARIADB"
    Host="maria.local"
    Port="3307"
    Database="analytics"
    UserName="bi"
  />
  <Connection
    ConnectionName="Local Cache"
    ConnType="SQLITE"
    DatabaseFileName="/Users/demo/app.sqlite"
  />
  <Connection
    ConnectionName="Warehouse Snowflake"
    ConnType="SNOWFLAKE"
    Host="acct.snowflakecomputing.com"
  />
</Connections>`;

const DBEAVER_SAMPLE = JSON.stringify({
  folders: {},
  connections: {
    'postgres-jdbc-1': {
      provider: 'postgresql',
      driver: 'postgres-jdbc',
      name: 'Prod Core PG',
      configuration: {
        host: 'pg-prod.internal',
        port: '5432',
        database: 'core',
        user: 'app_ro',
        password: 'should-never-be-imported',
        type: 'prod',
        handlers: {
          ssh_tunnel: {
            type: 'TUNNEL',
            enabled: true,
            properties: {
              host: 'bastion.prod',
              port: 22,
              authType: 'PUBLIC_KEY',
              keyPath: '/Users/ops/.ssh/prod_ed25519',
              user: 'ops',
              password: 'ignored-ssh-password',
            },
          },
        },
      },
    },
    'cockroach-1': {
      provider: 'postgresql',
      driver: 'cockroach-jdbc',
      name: ' Roach Staging ',
      configuration: {
        url: 'jdbc:postgresql://roach.internal:26257/defaultdb?sslmode=require',
        user: 'root',
        type: 'test',
      },
    },
    'oracle-sid-1': {
      provider: 'oracle',
      driver: 'oracle_thin',
      name: 'Finance XE',
      configuration: {
        host: '10.0.0.15',
        port: '1521',
        database: 'XE',
        user: 'fin',
        'provider-properties': {
          '@dbeaver-sid-service@': 'SID',
        },
      },
    },
    'duckdb-file-1': {
      provider: 'generic',
      driver: 'duckdb_jdbc',
      name: 'Events DuckDB',
      configuration: {
        url: 'jdbc:duckdb:/tmp/events.duckdb',
      },
    },
    'db2-legacy': {
      provider: 'db2',
      driver: 'db2_luw',
      name: 'Mainframe DB2',
      configuration: {
        host: 'db2.corp',
        port: '50000',
      },
    },
  },
});

describe('externalConnections parser', () => {
  it('parses Navicat .ncx XML with SSH tunnel, TLS CA, Oracle SID, presets, and skips unsupported types while stripping all secrets', () => {
    const parsed = parseConnectionImportFile(NAVICAT_SAMPLE);
    expect(parsed.source).toBe('navicat');
    expect(parsed.profiles).toHaveLength(4);
    expect(parsed.unsupported).toEqual([
      { name: 'Warehouse Snowflake', driver: 'SNOWFLAKE' },
    ]);

    const [pg, ora, maria, sqlite] = parsed.profiles;
    expect(pg.name).toBe('Orders & Billing (PG)');
    expect(pg.db_type).toBe(DatabaseType.PostgreSQL);
    expect(pg.host).toBe('db.internal');
    expect(pg.port).toBe(5433);
    expect(pg.database).toBe('orders');
    expect(pg.username).toBe('readonly_app');
    expect(pg.password).toBe('');
    expect(pg.save_password).toBe(false);
    expect(pg.agent_access).toBe('off');
    expect(pg.tls_mode).toBe('verify-ca');
    expect(pg.ca_certificate_path).toBe('/etc/ssl/root.pem');
    expect(pg.ssh_tunnel).toEqual({
      host: 'jump.internal',
      port: 2222,
      username: 'deploy',
      private_key_path: '~/.ssh/id_ed25519',
      auth: 'private-key',
      secret: '',
    });

    expect(ora.db_type).toBe(DatabaseType.Oracle);
    expect(ora.database).toBe('ORCLSID');
    expect(ora.options).toEqual({ connectBy: 'sid' });
    expect(ora.password).toBe('');

    expect(maria.db_type).toBe(DatabaseType.MySQL);
    expect(maria.port).toBe(3307);
    expect(maria.options).toEqual({ server: 'mariadb' });

    expect(sqlite.db_type).toBe(DatabaseType.SQLite);
    expect(sqlite.database).toBe('/Users/demo/app.sqlite');
  });

  it('parses DBeaver data-sources.json with prod environment, SSH handler, JDBC URL fallback, and unsupported notice', () => {
    const parsed = parseConnectionImportFile(DBEAVER_SAMPLE);
    expect(parsed.source).toBe('dbeaver');
    expect(parsed.profiles).toHaveLength(4);
    expect(parsed.unsupported).toEqual([
      { name: 'Mainframe DB2', driver: 'db2 / db2_luw' },
    ]);

    const [prodPg, roach, oraSid, duckdb] = parsed.profiles;
    expect(prodPg.name).toBe('Prod Core PG');
    expect(prodPg.environment).toBe('production');
    expect(prodPg.password).toBe('');
    expect(prodPg.save_password).toBe(false);
    expect(prodPg.agent_access).toBe('off');
    expect(prodPg.ssh_tunnel).toEqual({
      host: 'bastion.prod',
      port: 22,
      username: 'ops',
      private_key_path: '/Users/ops/.ssh/prod_ed25519',
      auth: 'private-key',
      secret: '',
    });

    expect(roach.name).toBe('Roach Staging');
    expect(roach.db_type).toBe(DatabaseType.PostgreSQL);
    expect(roach.host).toBe('roach.internal');
    expect(roach.port).toBe(26257);
    expect(roach.database).toBe('defaultdb');
    expect(roach.options).toEqual({ server: 'cockroachdb' });
    expect(roach.environment).toBe('testing');

    expect(oraSid.db_type).toBe(DatabaseType.Oracle);
    expect(oraSid.database).toBe('XE');
    expect(oraSid.options).toEqual({ connectBy: 'sid' });

    expect(duckdb.db_type).toBe(DatabaseType.DuckDB);
    expect(duckdb.database).toBe('/tmp/events.duckdb');
  });

  it('parses native DataOmni export JSON, strips injected secrets/agent_access, and rejects newer format versions', () => {
    const doc = JSON.stringify({
      format: 'dataomni-connections',
      version: 1,
      connections: [
        {
          ...createDefaultConfig(DatabaseType.Redis),
          name: 'Local Redis',
          host: '127.0.0.1',
          port: 6379,
          password: 'leaked',
          agent_access: 'read',
        },
      ],
    });
    const parsed = parseConnectionImportFile(doc);
    expect(parsed.source).toBe('dataomni');
    expect(parsed.profiles).toHaveLength(1);
    expect(parsed.profiles[0].password).toBe('');
    expect(parsed.profiles[0].agent_access).toBe('off');

    const newer = JSON.stringify({
      format: 'dataomni-connections',
      version: 99,
      connections: [],
    });
    expect(() => parseConnectionImportFile(newer)).toThrow(
      'DATAOMNI_IMPORT_NEWER_VERSION: 99',
    );
  });

  it('rejects unrecognized JSON or malformed XML with DATAOMNI_IMPORT_NOT_CONNECTIONS_FILE', () => {
    expect(() => parseConnectionImportFile('not json or xml')).toThrow(
      /DATAOMNI_IMPORT_NOT_CONNECTIONS_FILE/,
    );
    expect(() => parseConnectionImportFile('{"hello": "world"}')).toThrow(
      /DATAOMNI_IMPORT_NOT_CONNECTIONS_FILE/,
    );
    expect(() => parseConnectionImportFile('<Connections><Connection Host="unclosed"')).toThrow(
      /DATAOMNI_IMPORT_NOT_CONNECTIONS_FILE/,
    );
  });

  it('matches duplicate targets using the same 6-tuple as ConnectionService::import_profiles', () => {
    const base: ConnectionProfile = {
      ...createDefaultConfig(DatabaseType.PostgreSQL),
      id: 'existing-1',
      name: 'Shop Dev',
      host: '127.0.0.1',
      port: 5432,
      database: 'shop',
      username: 'app',
      created_at: '',
      updated_at: '',
    };
    const cloneWithNewId: ConnectionProfile = { ...base, id: 'new-id' };
    expect(isSameConnectionTarget(base, cloneWithNewId)).toBe(true);
    expect(isSameConnectionTarget(base, { ...base, database: 'other' })).toBe(false);
  });

  it('caps out-of-range numeric XML entities instead of throwing RangeError (H18 / P2-5)', () => {
    const malformed = `<?xml version="1.0" encoding="UTF-8"?>
<Connections Ver="2.0">
  <Connection ConnectionName="Test &#99999999; &#x110000; &#65;" ConnType="PGSQL" Host="127.0.0.1" Port="5432" Database="app" UserName="u" />
</Connections>`;
    const parsed = parseConnectionImportFile(malformed);
    expect(parsed.profiles).toHaveLength(1);
    expect(parsed.profiles[0].name).toBe('Test &#99999999; &#x110000; A');
  });
});
