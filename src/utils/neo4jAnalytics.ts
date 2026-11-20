import type { SerializedResultValue } from '../contracts/resultSet';
import type { CypherValue } from './cypherValue';

export const NEO4J_SAMPLE_LIMIT_OPTIONS = [500, 1_000, 5_000] as const;
export const DEFAULT_NEO4J_SAMPLE_LIMIT = 1_000;
export const MAX_NEO4J_OVERVIEW_ITEMS = 40;

export interface Neo4jCountItem {
  readonly name: string;
  readonly count: number;
}

export interface Neo4jSupernodeEntry {
  readonly id: string;
  readonly labels: readonly string[];
  readonly degree: number;
}

export interface Neo4jPropertyCoverage {
  readonly key: string;
  readonly count: number;
  readonly ratio: number;
}

export interface Neo4jDegreeSampleSummary {
  readonly sampledNodes: number;
  readonly isolatedCount: number;
  readonly isolatedRatio: number;
  readonly minDegree: number;
  readonly p50Degree: number;
  readonly p95Degree: number;
  readonly maxDegree: number;
  readonly supernodes: readonly Neo4jSupernodeEntry[];
  readonly propertyCoverage: readonly Neo4jPropertyCoverage[];
}

export function quoteCypherIdent(name: string): string {
  return `\`${name.replace(/`/g, '``')}\``;
}

/** O(1) count-store 标签节点数查询 */
export function buildLabelCountCypher(label: string): string {
  return `MATCH (n:${quoteCypherIdent(label)}) RETURN count(n) AS count`;
}

/** O(1) count-store 关系类型边数查询 */
export function buildRelationshipCountCypher(relType: string): string {
  return `MATCH ()-[r:${quoteCypherIdent(relType)}]->() RETURN count(r) AS count`;
}

/** 受限抽样节点度与属性覆盖率 Cypher（默认 Neo4j 5 `COUNT { (n)--() }`，兼容 Neo4j 4 回退） */
export function buildNeo4jDegreeSampleCypher(
  label: string | null,
  sampleLimit: number,
  legacySyntax = false
): string {
  const safeLimit = Math.min(5_000, Math.max(1, Math.trunc(sampleLimit)));
  const matchClause = label ? `MATCH (n:${quoteCypherIdent(label)})` : 'MATCH (n)';
  const idExpr = legacySyntax ? 'toString(id(n))' : 'elementId(n)';
  const degreeExpr = legacySyntax ? 'size((n)--())' : 'COUNT { (n)--() }';
  return `${matchClause} WITH n LIMIT ${safeLimit} RETURN ${idExpr} AS id, labels(n) AS labels, ${degreeExpr} AS degree, keys(n) AS propKeys`;
}

/** 生成只读排查超级节点局部邻域的 Cypher（严格不写库） */
export function buildInspectSupernodeCypher(elementId: string, legacyId = false): string {
  const predicate = legacyId
    ? `toString(id(n)) = ${JSON.stringify(elementId)}`
    : `elementId(n) = ${JSON.stringify(elementId)}`;
  return `MATCH (n)\nWHERE ${predicate}\nOPTIONAL MATCH (n)-[r]-(m)\nRETURN n, r, m\nLIMIT 50`;
}

/** 生成只读抽查某 Label 样本节点的 Cypher（严格不写库） */
export function buildInspectLabelCypher(label: string): string {
  return `MATCH (n:${quoteCypherIdent(label)})\nRETURN n\nLIMIT 50`;
}

export function cypherValueAsNumber(value: CypherValue | undefined): number {
  if (!value) return 0;
  if (value.kind === 'integer' || value.kind === 'float') {
    const parsed = Number(value.value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

export function cypherValueAsString(value: CypherValue | undefined): string {
  if (!value) return '';
  if (value.kind === 'string') return value.value;
  if (value.kind === 'integer') return value.value;
  if (value.kind === 'float') return String(value.value);
  return '';
}

function cypherValueAsStringList(value: CypherValue | undefined): string[] {
  if (!value || value.kind !== 'list') {
    return [];
  }
  return value.items.map((item) => cypherValueAsString(item)).filter((item) => item.length > 0);
}

function percentile(sorted: readonly number[], pct: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.round((sorted.length - 1) * pct));
  return sorted[idx] ?? 0;
}

/** 从采样 Cypher 返回行汇总度分位数、孤立节点占比、Top 20 超级节点与属性覆盖率 */
export function summarizeNeo4jDegreeSample(rows: readonly CypherValue[][]): Neo4jDegreeSampleSummary {
  const sampledNodes = rows.length;
  const degrees: number[] = [];
  let isolatedCount = 0;
  const nodes: Neo4jSupernodeEntry[] = [];
  const propCounts = new Map<string, number>();

  for (const row of rows) {
    const id = cypherValueAsString(row[0]);
    const labels = cypherValueAsStringList(row[1]);
    const degree = Math.max(0, Math.trunc(cypherValueAsNumber(row[2])));
    const propKeys = cypherValueAsStringList(row[3]);

    degrees.push(degree);
    if (degree === 0) {
      isolatedCount += 1;
    }
    nodes.push({ id, labels, degree });

    const uniqueKeys = new Set(propKeys);
    for (const key of uniqueKeys) {
      propCounts.set(key, (propCounts.get(key) ?? 0) + 1);
    }
  }

  degrees.sort((a, b) => a - b);
  const supernodes = [...nodes]
    .sort((a, b) => b.degree - a.degree || a.id.localeCompare(b.id))
    .slice(0, 20);

  const propertyCoverage: Neo4jPropertyCoverage[] = Array.from(propCounts.entries())
    .map(([key, count]) => ({
      key,
      count,
      ratio: sampledNodes > 0 ? count / sampledNodes : 0
    }))
    .sort((a, b) => a.ratio - b.ratio || a.key.localeCompare(b.key));

  return {
    sampledNodes,
    isolatedCount,
    isolatedRatio: sampledNodes > 0 ? isolatedCount / sampledNodes : 0,
    minDegree: degrees[0] ?? 0,
    p50Degree: percentile(degrees, 0.5),
    p95Degree: percentile(degrees, 0.95),
    maxDegree: degrees[degrees.length - 1] ?? 0,
    supernodes,
    propertyCoverage
  };
}

/** 将 Label / Relationship Type 体量转为 `ResultChartDialog` 原生 SVG 表格行列 */
export function neo4jCountsToChartTable(
  labelCol: string,
  items: readonly Neo4jCountItem[]
): { columns: string[]; rows: SerializedResultValue[][] } {
  return {
    columns: [labelCol, 'count'],
    rows: items.map((item) => [item.name, item.count])
  };
}
