import { describe, expect, it } from 'vitest';
import { DatabaseType, type CompletionRelation } from '../contracts';
import { tableStructureKey, type CachedTableStructure } from '../stores/appStore';
import {
  buildCellJump,
  normalizeJumpSchema,
  parseSqlJumpContext,
  resolveQueryColumnJumps,
  resolveTableColumnJumps,
  serializeJumpFilterValue
} from './foreignKeyJump';

const SAMPLE_RELATIONS: CompletionRelation[] = [
  {
    schema: null,
    name: 'users',
    kind: 'table',
    columns: [
      { name: 'id', dataType: 'varchar(64)' },
      { name: 'email', dataType: 'varchar(255)' }
    ]
  },
  {
    schema: null,
    name: 'claw_info',
    kind: 'table',
    columns: [
      { name: 'claw_id', dataType: 'varchar(64)' },
      { name: 'owner_id', dataType: 'varchar(64)' }
    ]
  },
  {
    schema: null,
    name: 'claw_skill_hub',
    kind: 'table',
    columns: [
      { name: 'id', dataType: 'bigint' },
      { name: 'global_skill_id', dataType: 'varchar(128)' },
      { name: 'skill_name', dataType: 'varchar(128)' }
    ]
  },
  {
    schema: null,
    name: 'claw_user_skill',
    kind: 'table',
    columns: [
      { name: 'id', dataType: 'bigint' },
      { name: 'user_id', dataType: 'varchar(64)' },
      { name: 'claw_id', dataType: 'varchar(64)' },
      { name: 'skill_id', dataType: 'bigint' },
      { name: 'updated_at', dataType: 'datetime' }
    ]
  }
];

describe('foreignKeyJump', () => {
  it('serializes scalar values for jump filters and rejects null/empty/bool/binary', () => {
    expect(serializeJumpFilterValue('abc')).toBe('abc');
    expect(serializeJumpFilterValue(42)).toBe('42');
    expect(serializeJumpFilterValue({ type: 'bigint', value: '9007199254740993' })).toBe('9007199254740993');
    expect(serializeJumpFilterValue({ type: 'decimal', value: '10.50' })).toBe('10.50');
    expect(serializeJumpFilterValue(null)).toBeNull();
    expect(serializeJumpFilterValue('')).toBeNull();
    expect(serializeJumpFilterValue('   ')).toBeNull();
    expect(serializeJumpFilterValue(true)).toBeNull();
    expect(serializeJumpFilterValue({ type: 'binary', value: '0xdead' })).toBeNull();
  });

  it('normalizes same-database schema to null for MySQL while preserving PostgreSQL schemas', () => {
    expect(normalizeJumpSchema('app_db', DatabaseType.MySQL, 'app_db')).toBeNull();
    expect(normalizeJumpSchema('other_db', DatabaseType.MySQL, 'app_db')).toBe('other_db');
    expect(normalizeJumpSchema('public', DatabaseType.PostgreSQL, 'app_db')).toBe('public');
  });

  it('resolves physical single and composite foreign keys in TableDataViewer', () => {
    const jumps = resolveTableColumnJumps({
      columns: ['id', 'tenant_id', 'user_id', 'note'],
      foreignKeys: [
        {
          name: 'fk_orders_user',
          columns: ['tenant_id', 'user_id'],
          referencedSchema: 'shop',
          referencedTable: 'users',
          referencedColumns: ['tenant_id', 'id'],
          onUpdate: null,
          onDelete: null
        }
      ],
      currentTable: 'orders',
      currentSchema: null,
      dbType: DatabaseType.MySQL,
      currentDatabase: 'shop'
    });

    const userTargets = jumps.get('user_id');
    expect(userTargets).toHaveLength(1);
    expect(userTargets?.[0]).toMatchObject({
      schema: null,
      table: 'users',
      targetColumns: ['tenant_id', 'id'],
      sourceColumns: ['tenant_id', 'user_id'],
      source: 'foreign-key',
      inline: true
    });
    expect(jumps.has('note')).toBe(false);

    const resolved = buildCellJump(userTargets![0], (col) =>
      col === 'tenant_id' ? 7 : col === 'user_id' ? 99 : null
    );
    expect(resolved).toEqual({
      schema: null,
      table: 'users',
      filters: [
        { column: 'tenant_id', value: '7' },
        { column: 'id', value: '99' }
      ],
      source: 'foreign-key',
      targetLabel: 'users (tenant_id, id)'
    });
  });

  it('parses JOIN aliases, ON equi-joins, and projections while ignoring comments and strings', () => {
    const sql = `
      -- SELECT fake FROM ignored
      SELECT us.user_id, us.claw_id, us.skill_id, s.global_skill_id, us.updated_at
      FROM claw_user_skill us
      JOIN claw_skill_hub s ON s.id = us.skill_id
      WHERE s.global_skill_id = 'a.b = c.d'
    `;
    const ctx = parseSqlJumpContext(sql, 'mysql');
    expect(ctx.aliases.get('us')).toEqual({ schema: null, table: 'claw_user_skill' });
    expect(ctx.aliases.get('s')).toEqual({ schema: null, table: 'claw_skill_hub' });
    expect(ctx.joinPairs).toEqual([
      {
        left: { schema: null, table: 'claw_skill_hub' },
        leftColumn: 'id',
        right: { schema: null, table: 'claw_user_skill' },
        rightColumn: 'skill_id'
      }
    ]);
    expect(ctx.projections).toHaveLength(5);
    expect(ctx.projections[3]).toEqual({
      kind: 'column',
      outputName: 'global_skill_id',
      qualifier: 's',
      column: 'global_skill_id'
    });
  });

  it('resolves SQL JOIN, catalog matches, and source-table jumps on queries without physical FKs', () => {
    const sql = `
      SELECT us.user_id, us.claw_id, us.skill_id, s.global_skill_id, us.updated_at
      FROM claw_user_skill us
      JOIN claw_skill_hub s ON s.id = us.skill_id
    `;
    const jumps = resolveQueryColumnJumps({
      sql,
      columns: ['user_id', 'claw_id', 'skill_id', 'global_skill_id', 'updated_at'],
      dialect: 'mysql',
      dbType: DatabaseType.MySQL,
      currentDatabase: 'app',
      connectionId: 'conn-1',
      relations: SAMPLE_RELATIONS
    });

    // user_id -> users.id via catalog match
    expect(jumps.get(0)?.[0]).toMatchObject({
      table: 'users',
      targetColumns: ['id'],
      source: 'catalog-match',
      inline: true
    });
    // claw_id -> claw_info.claw_id via catalog match
    expect(jumps.get(1)?.[0]).toMatchObject({
      table: 'claw_info',
      targetColumns: ['claw_id'],
      source: 'catalog-match',
      inline: true
    });
    // skill_id -> claw_skill_hub.id via explicit JOIN ON
    expect(jumps.get(2)?.[0]).toMatchObject({
      table: 'claw_skill_hub',
      targetColumns: ['id'],
      source: 'sql-join',
      inline: true
    });
    // s.global_skill_id -> claw_skill_hub.global_skill_id via source table
    expect(jumps.get(3)?.[0]).toMatchObject({
      table: 'claw_skill_hub',
      targetColumns: ['global_skill_id'],
      source: 'source-table',
      inline: true
    });
    // updated_at is not an ID column -> no jump clutter
    expect(jumps.has(4)).toBe(false);
  });

  it('uses cached physical foreign keys for single-table SQL queries', () => {
    const structKey = tableStructureKey('conn-pg', 'public', 'orders');
    const tableStructures: Record<string, CachedTableStructure> = {
      [structKey]: {
        connectionId: 'conn-pg',
        columns: [],
        schemaVersion: 1,
        lastUpdated: Date.now(),
        objects: {
          indexes: [],
          foreignKeys: [
            {
              name: 'fk_customer',
              columns: ['customer_id'],
              referencedSchema: 'public',
              referencedTable: 'customers',
              referencedColumns: ['id'],
              onUpdate: null,
              onDelete: null
            }
          ],
          checkConstraints: [],
          ddl: null,
          triggers: [],
          failures: {}
        }
      }
    };

    const jumps = resolveQueryColumnJumps({
      sql: 'SELECT id, customer_id, total FROM public.orders',
      columns: ['id', 'customer_id', 'total'],
      dialect: 'postgresql',
      dbType: DatabaseType.PostgreSQL,
      connectionId: 'conn-pg',
      tableStructures
    });

    expect(jumps.get(0)?.[0]).toMatchObject({
      schema: 'public',
      table: 'orders',
      targetColumns: ['id'],
      source: 'source-table',
      inline: true
    });
    expect(jumps.get(1)?.[0]).toMatchObject({
      schema: 'public',
      table: 'customers',
      targetColumns: ['id'],
      source: 'foreign-key',
      inline: true
    });
    expect(jumps.has(2)).toBe(false);
  });

  it('keeps ambiguous catalog matches in context menu without inline clutter', () => {
    const ambiguousRelations: CompletionRelation[] = [
      {
        schema: null,
        name: 'order_items',
        kind: 'table',
        columns: [{ name: 'id', dataType: 'int' }, { name: 'item_code', dataType: 'varchar' }]
      },
      {
        schema: null,
        name: 'cart_items',
        kind: 'table',
        columns: [{ name: 'id', dataType: 'int' }, { name: 'item_code', dataType: 'varchar' }]
      }
    ];
    const jumps = resolveTableColumnJumps({
      columns: ['item_id'],
      relations: ambiguousRelations,
      currentTable: 'shipments',
      dbType: DatabaseType.MySQL
    });
    const targets = jumps.get('item_id') ?? [];
    expect(targets.length).toBe(2);
    expect(targets.every((t) => t.inline === false)).toBe(true);
  });
});
