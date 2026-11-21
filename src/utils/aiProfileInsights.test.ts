import { describe, expect, it } from 'vitest';
import type { ColumnProfile } from '../contracts/analytics';
import {
  MAX_AI_PROFILE_COLUMNS,
  buildProfileInsightsPrompt,
  parseProfileInsightsReply
} from './aiProfileInsights';

function makeCol(overrides: Partial<ColumnProfile> & { name: string }): ColumnProfile {
  return {
    inferredType: 'text',
    duckdbType: 'VARCHAR',
    precisionDegraded: false,
    totalCount: 1000,
    nullCount: 0,
    nullRatio: 0,
    distinctCount: 10,
    distinctRatio: 0.01,
    bins: [],
    topValues: [],
    ...overrides
  };
}

describe('aiProfileInsights', () => {
  it('redacts text sample values, min/max strings, and topValues from prompts', () => {
    const prompt = buildProfileInsightsPrompt({
      title: 'orders',
      dialect: 'sqlite',
      scopeLabel: 'Sampled · first 5,000 rows',
      language: 'zh',
      profile: {
        rowCount: 5000,
        columnCount: 2,
        columns: [
          makeCol({
            name: 'customer_email',
            inferredType: 'text',
            min: 'aaa.secret@corp.example',
            max: 'zzz.private@corp.example',
            minLength: 8,
            maxLength: 42,
            emptyStringCount: 3,
            paddedWhitespaceCount: 2,
            topValues: [{ value: 'ceo.confidential@corp.example', count: 5, ratio: 0.001 }]
          }),
          makeCol({
            name: 'amount',
            inferredType: 'numeric',
            duckdbType: 'DOUBLE',
            min: '-120.5',
            max: '28000',
            p25: 35,
            p50: 110,
            p75: 240,
            p95: 420,
            mean: 390,
            stddev: 2100
          })
        ]
      }
    });

    // 必须包含脱敏后的聚合指标
    expect(prompt.user).toContain('customer_email (VARCHAR)');
    expect(prompt.user).toContain('len=8..42');
    expect(prompt.user).toContain('empty_strings=3');
    expect(prompt.user).toContain('padded_whitespace=2');
    expect(prompt.user).toContain('min=-120.5');
    expect(prompt.user).toContain('[high] amount: negative');

    // 隐私红线：绝不外发任何文本列的 min/max 或 topValues 真实样本值
    expect(prompt.user).not.toContain('aaa.secret@corp.example');
    expect(prompt.user).not.toContain('zzz.private@corp.example');
    expect(prompt.user).not.toContain('ceo.confidential@corp.example');
    expect(prompt.system).not.toContain('ceo.confidential@corp.example');
  });

  it('caps wide tables at MAX_AI_PROFILE_COLUMNS while prioritizing flagged columns', () => {
    const columns: ColumnProfile[] = Array.from({ length: 55 }, (_, idx) =>
      makeCol({ name: `col_${idx}`, nullCount: 0, nullRatio: 0 })
    );
    columns[52] = makeCol({
      name: 'late_dirty_col',
      nullCount: 400,
      nullRatio: 0.4,
      emptyStringCount: 12
    });
    const prompt = buildProfileInsightsPrompt({
      profile: { rowCount: 1000, columnCount: 55, columns },
      language: 'en'
    });
    expect(prompt.user).toContain(`showing ${MAX_AI_PROFILE_COLUMNS} columns`);
    expect(prompt.user).toContain('late_dirty_col');
  });

  it('parses fenced JSON replies and falls back gracefully on non-JSON text', () => {
    const ok = parseProfileInsightsReply(
      '```json\n{"summary":"发现 2 处异常","findings":[{"column":"amount","severity":"high","title":"存在负值","detail":"156 条退款金额误记为负","suggestedSql":"SELECT * FROM result_set WHERE amount < 0"}]}\n```'
    );
    expect(ok.ok).toBe(true);
    if (ok.ok) {
      expect(ok.value.summary).toBe('发现 2 处异常');
      expect(ok.value.findings).toHaveLength(1);
      expect(ok.value.findings[0].severity).toBe('high');
      expect(ok.value.findings[0].suggestedSql).toContain('amount < 0');
    }

    const fallback = parseProfileInsightsReply('Plain text response without JSON');
    expect(fallback.ok).toBe(false);
  });
});
