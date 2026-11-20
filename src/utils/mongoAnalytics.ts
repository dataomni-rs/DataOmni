import type { SerializedResultValue } from '../contracts/resultSet';
import type { MongoDocumentRow } from './mongoDocuments';

export const MONGO_SAMPLE_SIZE_OPTIONS = [500, 1_000, 5_000, 10_000] as const;
export const DEFAULT_MONGO_SAMPLE_SIZE = 1_000;

export interface MongoTypeShare {
  readonly bsonType: string;
  readonly count: number;
  readonly ratio: number;
}

export interface MongoArrayLengthStats {
  readonly min: number;
  readonly p50: number;
  readonly p95: number;
  readonly max: number;
}

export interface MongoSchemaPathProfile {
  readonly path: string;
  readonly depth: number;
  readonly presentCount: number;
  readonly presentRatio: number;
  readonly missingCount: number;
  readonly explicitNullCount: number;
  readonly hasTypeDrift: boolean;
  readonly types: readonly MongoTypeShare[];
  readonly arrayLen?: MongoArrayLengthStats;
  readonly sampleValues: readonly string[];
}

export interface MongoStorageSummary {
  readonly count: number;
  readonly avgObjSize: number;
  readonly storageSize: number;
  readonly totalIndexSize: number;
}

export interface MongoIndexUsageEntry {
  readonly name: string;
  readonly ops: number;
  readonly since?: string;
  readonly unused: boolean;
}

export interface MongoCollectionAnalysis {
  readonly sampledDocs: number;
  readonly truncatedPathsCount: number;
  readonly elapsedMs: number;
  readonly paths: readonly MongoSchemaPathProfile[];
  readonly storage?: MongoStorageSummary;
  readonly indexUsage: readonly MongoIndexUsageEntry[];
  readonly statsUnavailableReason?: string;
}

const BSON_TO_MONGO_TYPE_ALIAS: Readonly<Record<string, string>> = {
  double: 'double',
  string: 'string',
  array: 'array',
  document: 'object',
  bool: 'bool',
  null: 'null',
  int32: 'int',
  int64: 'long',
  date: 'date',
  objectId: 'objectId',
  binary: 'binData',
  decimal128: 'decimal'
};

/** 将嵌套展示路径（如 `items[].price`）转为 MongoDB 点号查询路径（`items.price`） */
export function mongoQueryPath(displayPath: string): string {
  return displayPath.replace(/\[\]/g, '');
}

/** 生成用于回填集合过滤框的 mongosh 筛选文档（按 BSON 类型或字段缺失排查脏数据） */
export function buildMongoTypeFilter(displayPath: string, bsonType: string): string {
  const path = mongoQueryPath(displayPath);
  const key = JSON.stringify(path);
  if (bsonType === 'missing') {
    return `{ ${key}: { $exists: false } }`;
  }
  const alias = BSON_TO_MONGO_TYPE_ALIAS[bsonType] ?? bsonType;
  return `{ ${key}: { $type: ${JSON.stringify(alias)} } }`;
}

/** 按关键字与「仅看类型漂移」过滤路径清单 */
export function filterMongoSchemaPaths(
  paths: readonly MongoSchemaPathProfile[],
  query: string,
  onlyDrift: boolean
): MongoSchemaPathProfile[] {
  const needle = query.trim().toLowerCase();
  return paths.filter((item) => {
    if (onlyDrift && !item.hasTypeDrift) {
      return false;
    }
    if (!needle) {
      return true;
    }
    return (
      item.path.toLowerCase().includes(needle)
      || item.types.some((t) => t.bsonType.toLowerCase().includes(needle))
    );
  });
}

/** 将 MongoCollectionViewer 当前页文档（含聚合管道输出）转换为 ResultChartDialog 所需的二维行列 */
export function mongoRowsToChartTable(
  columns: readonly string[],
  documents: readonly MongoDocumentRow[]
): { columns: string[]; rows: SerializedResultValue[][] } {
  const rows: SerializedResultValue[][] = documents.map((doc) => {
    const fieldMap = new Map(doc.fields);
    return columns.map((col) => {
      const cell = fieldMap.get(col);
      if (!cell || cell.kind === 'null') {
        return null;
      }
      if (cell.kind === 'bool') {
        return cell.text === 'true';
      }
      if (cell.kind === 'int' || cell.kind === 'double') {
        const num = Number(cell.text);
        return Number.isFinite(num) ? num : cell.text;
      }
      return cell.text;
    });
  });
  return { columns: [...columns], rows };
}
