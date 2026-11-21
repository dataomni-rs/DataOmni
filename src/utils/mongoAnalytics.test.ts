import { describe, expect, it } from 'vitest';
import {
  buildMongoTypeFilter,
  filterMongoSchemaPaths,
  mongoQueryPath,
  mongoRowsToChartTable,
  type MongoSchemaPathProfile
} from './mongoAnalytics';

const SAMPLE_PATHS: MongoSchemaPathProfile[] = [
  {
    path: 'user.age',
    depth: 2,
    presentCount: 100,
    presentRatio: 1,
    missingCount: 0,
    explicitNullCount: 2,
    hasTypeDrift: true,
    types: [
      { bsonType: 'int32', count: 80, ratio: 0.8 },
      { bsonType: 'string', count: 18, ratio: 0.18 },
      { bsonType: 'null', count: 2, ratio: 0.02 }
    ],
    sampleValues: ['25', '30']
  },
  {
    path: 'items[].price',
    depth: 2,
    presentCount: 90,
    presentRatio: 0.9,
    missingCount: 10,
    explicitNullCount: 0,
    hasTypeDrift: false,
    types: [{ bsonType: 'double', count: 90, ratio: 1 }],
    sampleValues: ['9.9']
  }
];

describe('mongoAnalytics', () => {
  it('normalizes array display paths and builds mongosh $type / $exists filters', () => {
    expect(mongoQueryPath('orders[].items[].price')).toBe('orders.items.price');
    expect(buildMongoTypeFilter('user.age', 'int32')).toBe('{ "user.age": { $type: "int" } }');
    expect(buildMongoTypeFilter('items[].price', 'string')).toBe(
      '{ "items.price": { $type: "string" } }'
    );
    expect(buildMongoTypeFilter('user.email', 'missing')).toBe(
      '{ "user.email": { $exists: false } }'
    );
  });

  it('filters schema paths by search term and drift-only flag', () => {
    expect(filterMongoSchemaPaths(SAMPLE_PATHS, '', true).map((p) => p.path)).toEqual(['user.age']);
    expect(filterMongoSchemaPaths(SAMPLE_PATHS, 'double', false).map((p) => p.path)).toEqual([
      'items[].price'
    ]);
  });

  it('converts MongoDocumentRow pages into typed chart rows', () => {
    const table = mongoRowsToChartTable(['_id', 'total'], [
      {
        id: '1',
        fields: [
          ['_id', { kind: 'string', text: 'North', truncated: false }],
          ['total', { kind: 'double', text: '125.5', truncated: false }]
        ]
      },
      {
        id: '2',
        fields: [
          ['_id', { kind: 'string', text: 'South', truncated: false }],
          ['total', { kind: 'null', text: 'null', truncated: false }]
        ]
      }
    ]);
    expect(table).toEqual({
      columns: ['_id', 'total'],
      rows: [
        ['North', 125.5],
        ['South', null]
      ]
    });
  });
});
