import type { DriverQueryColumn } from './query';
import type { SerializedResultValue } from './resultSet';

export type AnalyticsSourceKind = 'resultSet' | 'jsonDocuments' | 'localFile';

export type LocalFileKind = 'parquet' | 'csv' | 'tsv' | 'json' | 'ndjson';

export type ProfileColumnType =
  | 'integer'
  | 'numeric'
  | 'text'
  | 'boolean'
  | 'date'
  | 'time'
  | 'datetime'
  | 'binary'
  | 'json'
  | 'unknown';

export interface AnalyticsColumnInput {
  readonly name: string;
  readonly logicalType?: string | null;
  readonly databaseType?: string | null;
}

export interface AnalyticsColumnMeta {
  readonly name: string;
  readonly duckdbType: string;
  readonly inferredType: ProfileColumnType;
  readonly precisionDegraded: boolean;
}

export interface AnalyticsSessionSummary {
  readonly analyticsId: string;
  readonly sourceKind: AnalyticsSourceKind;
  readonly fileKind?: LocalFileKind;
  readonly rowCount: number;
  readonly columnCount: number;
  readonly truncated: boolean;
  readonly columns: readonly AnalyticsColumnMeta[];
}

export interface AnalyticsQueryResult {
  readonly columns: readonly string[];
  readonly columnMetadata: readonly DriverQueryColumn[];
  readonly rows: ReadonlyArray<readonly SerializedResultValue[]>;
  readonly rowCount: number;
  readonly truncated: boolean;
  readonly truncationReason: 'row_limit' | 'byte_limit' | null;
  readonly elapsedMs: number;
}

export interface ParquetExportSummary {
  readonly rowCount: number;
  readonly bytes: number;
}

export interface HistogramBin {
  readonly lower: number;
  readonly upper: number;
  readonly count: number;
}

export interface TimeHistogramBin {
  readonly label: string;
  readonly count: number;
}

export interface TopValueEntry {
  readonly value: string;
  readonly count: number;
  readonly ratio: number;
}

export interface ColumnProfile {
  readonly name: string;
  readonly inferredType: ProfileColumnType;
  readonly duckdbType: string;
  readonly precisionDegraded: boolean;
  readonly totalCount: number;
  readonly nullCount: number;
  readonly nullRatio: number;
  readonly distinctCount: number;
  readonly distinctRatio: number;
  /** 仅 NoSQL JSON 文档模式：该字段实际出现的文档数（区分缺键与显式 null） */
  readonly presentCount?: number;
  readonly presentRatio?: number;
  readonly explicitNullCount?: number;
  readonly min?: string;
  readonly max?: string;
  readonly mean?: number;
  readonly stddev?: number;
  readonly p25?: number;
  readonly p50?: number;
  readonly p75?: number;
  readonly p95?: number;
  readonly bins: readonly HistogramBin[];
  readonly timeBins?: readonly TimeHistogramBin[];
  readonly minLength?: number;
  readonly maxLength?: number;
  readonly topValues: readonly TopValueEntry[];
}

export interface ProfileResult {
  readonly analyticsId: string;
  readonly rowCount: number;
  readonly columnCount: number;
  readonly elapsedMs: number;
  readonly columns: readonly ColumnProfile[];
}
