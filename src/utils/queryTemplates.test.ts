/**
 * @vitest-environment happy-dom
 */
import { beforeEach, describe, expect, it } from 'vitest';
import ON_DISK_FIXTURE from '../../fixtures/on-disk-0.5/query-templates.json?raw';
import { DatabaseType } from '../contracts/connection';
import {
  filterQueryTemplates,
  loadQueryTemplates,
  MAX_QUERY_TEMPLATES,
  MAX_QUERY_TEMPLATE_STORAGE_CHARS,
  QUERY_TEMPLATES_STORAGE_KEY,
  removeQueryTemplate,
  resetQueryTemplatesDiskGuardForTest,
  saveQueryTemplates,
  templateParameterNames,
  upsertQueryTemplate,
  type QueryTemplate,
} from './queryTemplates';

describe('queryTemplates storage & pure logic', () => {
  beforeEach(() => {
    localStorage.clear();
    resetQueryTemplatesDiskGuardForTest();
  });

  it('loads the R1 on-disk fixture losslessly', () => {
    localStorage.setItem(QUERY_TEMPLATES_STORAGE_KEY, ON_DISK_FIXTURE);
    const loaded = loadQueryTemplates();
    expect(loaded).toHaveLength(2);
    expect(loaded[0]).toMatchObject({
      id: 'tpl-orders-by-customer',
      title: '按客户查近期订单',
      dbType: DatabaseType.PostgreSQL,
    });
    expect(loaded[1]).toMatchObject({
      id: 'tpl-table-row-sample',
      title: '按状态统计行数',
    });
    expect(loaded[1].dbType).toBeUndefined();
  });

  it('isolates malformed entries and degrades unknown future dbType to general SQL instead of dropping SQL', () => {
    localStorage.setItem(
      QUERY_TEMPLATES_STORAGE_KEY,
      JSON.stringify({
        version: 1,
        templates: [
          { id: 'ok-1', title: 'Valid PG', sql: 'SELECT :id', dbType: 'postgresql', updatedAt: '2026-11-17T00:00:00Z' },
          { id: 'bad-missing-sql', title: 'Broken' },
          null,
          { id: 'future-db', title: 'Future Engine', sql: 'SELECT 42', dbType: 'snowflake_future', updatedAt: '2026-11-16T00:00:00Z' },
        ],
      }),
    );

    const loaded = loadQueryTemplates();
    expect(loaded.map((item) => ({ id: item.id, dbType: item.dbType }))).toEqual([
      { id: 'ok-1', dbType: DatabaseType.PostgreSQL },
      { id: 'future-db', dbType: undefined },
    ]);
  });

  it('returns [] on corrupt JSON and refuses to overwrite a newer snapshot version on disk', () => {
    localStorage.setItem(QUERY_TEMPLATES_STORAGE_KEY, '{not-json');
    expect(loadQueryTemplates()).toEqual([]);

    const newerPayload = JSON.stringify({
      version: 99,
      templates: [{ id: 'v99', title: 'Newer', sql: 'SELECT 1', updatedAt: '2027-01-01T00:00:00Z' }],
    });
    localStorage.setItem(QUERY_TEMPLATES_STORAGE_KEY, newerPayload);
    expect(loadQueryTemplates()).toEqual([]);

    saveQueryTemplates([
      { id: 'v1', title: 'Should Not Overwrite', sql: 'SELECT 2', updatedAt: '2026-11-17T00:00:00Z' },
    ]);
    expect(localStorage.getItem(QUERY_TEMPLATES_STORAGE_KEY)).toBe(newerPayload);
  });

  it('upserts, updates in-place moving to top, removes, and enforces count & char caps', () => {
    const t1 = new Date('2026-11-17T01:00:00.000Z');
    const t2 = new Date('2026-11-17T02:00:00.000Z');
    const t3 = new Date('2026-11-17T03:00:00.000Z');

    let list: QueryTemplate[] = [];
    list = upsertQueryTemplate(list, { title: '  First  ', sql: 'SELECT :a' }, t1);
    list = upsertQueryTemplate(list, { title: 'Second', sql: 'SELECT :b', dbType: DatabaseType.MySQL }, t2);
    expect(list.map((item) => item.title)).toEqual(['Second', 'First']);

    const firstId = list[1].id;
    list = upsertQueryTemplate(list, { id: firstId, title: 'First Updated', sql: 'SELECT :a, :c' }, t3);
    expect(list).toHaveLength(2);
    expect(list[0]).toMatchObject({ id: firstId, title: 'First Updated', sql: 'SELECT :a, :c' });

    list = removeQueryTemplate(list, firstId);
    expect(list.map((item) => item.title)).toEqual(['Second']);

    // Count cap
    const many: QueryTemplate[] = Array.from({ length: MAX_QUERY_TEMPLATES + 5 }, (_, idx) => ({
      id: `tpl-${idx}`,
      title: `Template ${idx}`,
      sql: 'SELECT 1;',
      updatedAt: new Date(Date.UTC(2026, 0, 1, 0, 0, idx)).toISOString(),
    }));
    saveQueryTemplates(many);
    const reloadedMany = loadQueryTemplates();
    expect(reloadedMany).toHaveLength(MAX_QUERY_TEMPLATES);
    // Newest kept, oldest 5 dropped
    expect(reloadedMany[0].id).toBe(`tpl-${MAX_QUERY_TEMPLATES + 4}`);

    // Character budget cap
    const hugeChunk = 'x'.repeat(120_000);
    const oversized: QueryTemplate[] = Array.from({ length: 6 }, (_, idx) => ({
      id: `big-${idx}`,
      title: `Big ${idx}`,
      sql: `SELECT '${hugeChunk}';`,
      updatedAt: new Date(Date.UTC(2026, 1, 1, 0, 0, idx)).toISOString(),
    }));
    saveQueryTemplates(oversized);
    const rawSaved = localStorage.getItem(QUERY_TEMPLATES_STORAGE_KEY) ?? '';
    expect(rawSaved.length).toBeLessThanOrEqual(MAX_QUERY_TEMPLATE_STORAGE_CHARS);
    expect(loadQueryTemplates().length).toBeGreaterThan(0);
    expect(loadQueryTemplates().length).toBeLessThan(6);
  });

  it('extracts D2 :name bind parameters while ignoring ::cast, literals, and Oracle q-quotes, and filters by scope & query', () => {
    const pgSql = `SELECT id::text, ':not_param' FROM orders WHERE customer_id = :customer_id AND created_at >= :since;`;
    expect(templateParameterNames(pgSql, DatabaseType.PostgreSQL)).toEqual(['customer_id', 'since']);

    const oraSql = `SELECT q'[literal :ignored]', :real_param FROM dual;`;
    expect(templateParameterNames(oraSql, DatabaseType.Oracle)).toEqual(['real_param']);

    // ClickHouse does not support D2 :name binding -> returns []
    expect(templateParameterNames('SELECT :x', DatabaseType.ClickHouse)).toEqual([]);

    const templates: QueryTemplate[] = [
      {
        id: 'pg-locks',
        title: 'Active Locks',
        sql: 'SELECT * FROM pg_locks WHERE pid = :target_pid;',
        dbType: DatabaseType.PostgreSQL,
        updatedAt: '2026-11-17T03:00:00Z',
      },
      {
        id: 'general-orders',
        title: 'Customer Orders',
        sql: 'SELECT * FROM orders WHERE customer_id = :customer_id;',
        updatedAt: '2026-11-17T02:00:00Z',
      },
      {
        id: 'mysql-proc',
        title: 'Process List',
        sql: 'SHOW FULL PROCESSLIST;',
        dbType: DatabaseType.MySQL,
        updatedAt: '2026-11-17T01:00:00Z',
      },
    ];

    // Compatible scope on PostgreSQL includes PostgreSQL + general, excludes MySQL
    expect(
      filterQueryTemplates(templates, {
        query: '',
        currentDbType: DatabaseType.PostgreSQL,
        scope: 'compatible',
      }).map((item) => item.id),
    ).toEqual(['pg-locks', 'general-orders']);

    // Searching by :parameter name finds the template!
    expect(
      filterQueryTemplates(templates, {
        query: 'target_pid',
        currentDbType: DatabaseType.PostgreSQL,
        scope: 'all',
      }).map((item) => item.id),
    ).toEqual(['pg-locks']);
  });
});
