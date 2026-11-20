//! 只读跨库结构差异报告（`rfcs/cli-mcp-usability.md` CM3）。
//!
//! 方向与界面 `src/utils/schemaDiff.ts` 一致：「源 → 目标」，只输出差异报告（JSON 或 Markdown），
//! 绝不执行任何同步 DDL。

use crate::models::DatabaseType;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiffColumn {
  pub name: String,
  pub data_type: String,
  pub nullable: bool,
  pub primary_key: bool,
  pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiffIndex {
  pub name: String,
  pub columns: Vec<String>,
  pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiffForeignKey {
  pub name: String,
  pub columns: Vec<String>,
  pub referenced_table: String,
  pub referenced_columns: Vec<String>,
  pub on_update: Option<String>,
  pub on_delete: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiffTable {
  pub schema: Option<String>,
  pub name: String,
  pub columns: Vec<DiffColumn>,
  pub primary_key: Vec<String>,
  pub indexes: Vec<DiffIndex>,
  pub foreign_keys: Vec<DiffForeignKey>,
}

impl DiffTable {
  pub(super) fn display_name(&self) -> String {
    match self.schema.as_deref().filter(|s| !s.is_empty()) {
      Some(schema) => format!("{schema}.{}", self.name),
      None => self.name.clone(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ColumnDiff {
  pub name: String,
  pub changes: Vec<&'static str>,
  pub source: DiffColumn,
  pub target: DiffColumn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrimaryKeyDiff {
  pub source: Vec<String>,
  pub target: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TableDiff {
  pub table: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub columns_added: Vec<DiffColumn>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub columns_removed: Vec<DiffColumn>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub columns_changed: Vec<ColumnDiff>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub primary_key_changed: Option<PrimaryKeyDiff>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub indexes_added: Vec<DiffIndex>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub indexes_removed: Vec<DiffIndex>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub foreign_keys_added: Vec<DiffForeignKey>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub foreign_keys_removed: Vec<DiffForeignKey>,
}

impl TableDiff {
  fn change_count(&self) -> usize {
    self.columns_added.len()
      + self.columns_removed.len()
      + self.columns_changed.len()
      + usize::from(self.primary_key_changed.is_some())
      + self.indexes_added.len()
      + self.indexes_removed.len()
      + self.foreign_keys_added.len()
      + self.foreign_keys_removed.len()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiffSummary {
  pub tables_added: usize,
  pub tables_removed: usize,
  pub tables_changed: usize,
  pub total_differences: usize,
  pub identical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SchemaDiffReport {
  pub same_dialect: bool,
  pub ignore_schema: bool,
  pub summary: DiffSummary,
  pub tables_added: Vec<String>,
  pub tables_removed: Vec<String>,
  pub tables_changed: Vec<TableDiff>,
}

pub(super) fn supports_diff(db_type: &DatabaseType) -> bool {
  db_type.has_driver()
    && matches!(
      db_type,
      DatabaseType::PostgreSQL
        | DatabaseType::MySQL
        | DatabaseType::SQLite
        | DatabaseType::DuckDB
        | DatabaseType::Oracle
    )
}

fn fold(text: &str) -> String {
  text.trim().to_ascii_lowercase()
}

fn comparable_type(data_type: &str, dialect: &DatabaseType) -> String {
  let collapsed = data_type.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase();
  let compact = collapsed
    .replace(" (", "(")
    .replace("( ", "(")
    .replace(" )", ")")
    .replace(" ,", ",")
    .replace(", ", ",");
  if *dialect == DatabaseType::MySQL {
    strip_mysql_int_display_width(&compact)
  } else {
    compact
  }
}

fn strip_mysql_int_display_width(text: &str) -> String {
  for prefix in ["tinyint(", "smallint(", "mediumint(", "integer(", "bigint(", "int("] {
    if let Some(rest) = text.strip_prefix(prefix) {
      if let Some((digits, suffix)) = rest.split_once(')') {
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
          let base = prefix.trim_end_matches('(');
          return format!("{base}{suffix}");
        }
      }
    }
  }
  text.to_string()
}

fn normalized_default(value: Option<&str>) -> Option<String> {
  value.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn table_key(table: &DiffTable, ignore_schema: bool) -> String {
  if ignore_schema {
    fold(&table.name)
  } else {
    match table.schema.as_deref().filter(|s| !s.is_empty()) {
      Some(schema) => format!("{}.{}", fold(schema), fold(&table.name)),
      None => fold(&table.name),
    }
  }
}

fn index_signature(index: &DiffIndex) -> String {
  let cols: Vec<String> = index.columns.iter().map(|c| fold(c)).collect();
  format!("{}|{}|{}", fold(&index.name), if index.unique { "u" } else { "n" }, cols.join(","))
}

fn fk_signature(fk: &DiffForeignKey, same_dialect: bool, ignore_schema: bool) -> String {
  let cols: Vec<String> = fk.columns.iter().map(|c| fold(c)).collect();
  let ref_cols: Vec<String> = fk.referenced_columns.iter().map(|c| fold(c)).collect();
  let ref_table = if ignore_schema {
    fk.referenced_table
      .rsplit_once('.')
      .map_or_else(|| fold(&fk.referenced_table), |(_, name)| fold(name))
  } else {
    fold(&fk.referenced_table)
  };
  if same_dialect {
    format!(
      "{}->{}:{}|{}|{}",
      cols.join(","),
      ref_table,
      ref_cols.join(","),
      fk.on_update.as_deref().map(fold).unwrap_or_default(),
      fk.on_delete.as_deref().map(fold).unwrap_or_default(),
    )
  } else {
    format!("{}->{}:{}", cols.join(","), ref_table, ref_cols.join(","))
  }
}

/// 从 `schema_metadata::indexes` 原始目录行归并出逻辑索引（过滤主键索引）。
pub(super) fn indexes_from_rows(rows: &[Value], truthy: fn(&Value) -> bool) -> Vec<DiffIndex> {
  let mut grouped: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
  for row in rows {
    if truthy(&row["is_primary"]) {
      continue;
    }
    let name = row["index_name"].as_str().unwrap_or_default().trim();
    let col = row["column_name"].as_str().unwrap_or_default().trim();
    if name.is_empty() || col.is_empty() {
      continue;
    }
    let unique = truthy(&row["is_unique"]);
    let entry = grouped.entry(name.to_string()).or_insert_with(|| (unique, Vec::new()));
    entry.1.push(col.to_string());
  }
  grouped.into_iter().map(|(name, (unique, columns))| DiffIndex { name, columns, unique }).collect()
}

/// 从 `schema_metadata::foreign_keys` 原始目录行归并出外键定义。
pub(super) fn foreign_keys_from_rows(rows: &[Value]) -> Vec<DiffForeignKey> {
  let mut grouped: BTreeMap<String, DiffForeignKey> = BTreeMap::new();
  for row in rows {
    let name = row["constraint_name"].as_str().unwrap_or_default().trim();
    let col = row["column_name"].as_str().unwrap_or_default().trim();
    let ref_table_raw = row["referenced_table"].as_str().unwrap_or_default().trim();
    let ref_col = row["referenced_column"].as_str().unwrap_or_default().trim();
    if name.is_empty() || col.is_empty() || ref_table_raw.is_empty() || ref_col.is_empty() {
      continue;
    }
    let ref_schema = row["referenced_schema"].as_str().unwrap_or_default().trim();
    let referenced_table = if ref_schema.is_empty() {
      ref_table_raw.to_string()
    } else {
      format!("{ref_schema}.{ref_table_raw}")
    };
    let on_update =
      row["on_update"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let on_delete =
      row["on_delete"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let entry = grouped.entry(name.to_string()).or_insert_with(|| DiffForeignKey {
      name: name.to_string(),
      columns: Vec::new(),
      referenced_table,
      referenced_columns: Vec::new(),
      on_update,
      on_delete,
    });
    entry.columns.push(col.to_string());
    entry.referenced_columns.push(ref_col.to_string());
  }
  grouped.into_values().collect()
}

/// 比对源库与目标库的结构快照，返回纯只读差异报告。
pub(super) fn compare_snapshots(
  source_dialect: &DatabaseType,
  source_tables: &[DiffTable],
  target_dialect: &DatabaseType,
  target_tables: &[DiffTable],
) -> SchemaDiffReport {
  let same_dialect = source_dialect == target_dialect;
  let source_schemas: HashSet<&str> =
    source_tables.iter().map(|t| t.schema.as_deref().unwrap_or("")).collect();
  let target_schemas: HashSet<&str> =
    target_tables.iter().map(|t| t.schema.as_deref().unwrap_or("")).collect();
  let ignore_schema = source_schemas.len() <= 1 && target_schemas.len() <= 1;

  let source_map: BTreeMap<String, &DiffTable> =
    source_tables.iter().map(|t| (table_key(t, ignore_schema), t)).collect();
  let target_map: BTreeMap<String, &DiffTable> =
    target_tables.iter().map(|t| (table_key(t, ignore_schema), t)).collect();

  let mut tables_added = Vec::new();
  let mut tables_removed = Vec::new();
  let mut tables_changed = Vec::new();

  for (key, src) in &source_map {
    let Some(dst) = target_map.get(key) else {
      tables_added.push(src.display_name());
      continue;
    };
    let diff = compare_table(src, dst, source_dialect, same_dialect, ignore_schema);
    if diff.change_count() > 0 {
      tables_changed.push(diff);
    }
  }
  for (key, dst) in &target_map {
    if !source_map.contains_key(key) {
      tables_removed.push(dst.display_name());
    }
  }

  let detail_changes: usize = tables_changed.iter().map(TableDiff::change_count).sum();
  let total_differences = tables_added.len() + tables_removed.len() + detail_changes;
  let summary = DiffSummary {
    tables_added: tables_added.len(),
    tables_removed: tables_removed.len(),
    tables_changed: tables_changed.len(),
    total_differences,
    identical: total_differences == 0,
  };

  SchemaDiffReport {
    same_dialect,
    ignore_schema,
    summary,
    tables_added,
    tables_removed,
    tables_changed,
  }
}

fn compare_table(
  src: &DiffTable,
  dst: &DiffTable,
  dialect: &DatabaseType,
  same_dialect: bool,
  ignore_schema: bool,
) -> TableDiff {
  let src_cols: BTreeMap<String, &DiffColumn> =
    src.columns.iter().map(|c| (fold(&c.name), c)).collect();
  let dst_cols: BTreeMap<String, &DiffColumn> =
    dst.columns.iter().map(|c| (fold(&c.name), c)).collect();

  let mut columns_added = Vec::new();
  let mut columns_removed = Vec::new();
  let mut columns_changed = Vec::new();

  for col in &src.columns {
    let key = fold(&col.name);
    let Some(target_col) = dst_cols.get(&key) else {
      columns_added.push(col.clone());
      continue;
    };
    let mut changes = Vec::new();
    if same_dialect
      && comparable_type(&col.data_type, dialect) != comparable_type(&target_col.data_type, dialect)
    {
      changes.push("type");
    }
    if col.nullable != target_col.nullable {
      changes.push("nullable");
    }
    if same_dialect
      && normalized_default(col.default.as_deref())
        != normalized_default(target_col.default.as_deref())
    {
      changes.push("default");
    }
    if !changes.is_empty() {
      columns_changed.push(ColumnDiff {
        name: col.name.clone(),
        changes,
        source: col.clone(),
        target: (*target_col).clone(),
      });
    }
  }
  for col in &dst.columns {
    if !src_cols.contains_key(&fold(&col.name)) {
      columns_removed.push(col.clone());
    }
  }

  let src_pk: Vec<String> = src.primary_key.iter().map(|c| fold(c)).collect();
  let dst_pk: Vec<String> = dst.primary_key.iter().map(|c| fold(c)).collect();
  let primary_key_changed = if src_pk != dst_pk {
    Some(PrimaryKeyDiff { source: src.primary_key.clone(), target: dst.primary_key.clone() })
  } else {
    None
  };

  let src_idx: BTreeMap<String, &DiffIndex> =
    src.indexes.iter().map(|i| (index_signature(i), i)).collect();
  let dst_idx: BTreeMap<String, &DiffIndex> =
    dst.indexes.iter().map(|i| (index_signature(i), i)).collect();
  let indexes_added: Vec<DiffIndex> =
    src_idx.iter().filter(|(k, _)| !dst_idx.contains_key(*k)).map(|(_, v)| (*v).clone()).collect();
  let indexes_removed: Vec<DiffIndex> =
    dst_idx.iter().filter(|(k, _)| !src_idx.contains_key(*k)).map(|(_, v)| (*v).clone()).collect();

  let src_fks: BTreeMap<String, &DiffForeignKey> =
    src.foreign_keys.iter().map(|f| (fk_signature(f, same_dialect, ignore_schema), f)).collect();
  let dst_fks: BTreeMap<String, &DiffForeignKey> =
    dst.foreign_keys.iter().map(|f| (fk_signature(f, same_dialect, ignore_schema), f)).collect();
  let foreign_keys_added: Vec<DiffForeignKey> =
    src_fks.iter().filter(|(k, _)| !dst_fks.contains_key(*k)).map(|(_, v)| (*v).clone()).collect();
  let foreign_keys_removed: Vec<DiffForeignKey> =
    dst_fks.iter().filter(|(k, _)| !src_fks.contains_key(*k)).map(|(_, v)| (*v).clone()).collect();

  TableDiff {
    table: src.display_name(),
    columns_added,
    columns_removed,
    columns_changed,
    primary_key_changed,
    indexes_added,
    indexes_removed,
    foreign_keys_added,
    foreign_keys_removed,
  }
}

/// 将差异报告渲染为只读 Markdown 摘要（不包含同步 DDL）。
pub(super) fn render_markdown(
  source_name: &str,
  target_name: &str,
  report: &SchemaDiffReport,
) -> String {
  let mut lines = vec![
    format!("# Schema Diff: {source_name} → {target_name}"),
    String::new(),
    format!(
      "Tables added: {} · Tables removed: {} · Tables changed: {} · Total differences: {}",
      report.summary.tables_added,
      report.summary.tables_removed,
      report.summary.tables_changed,
      report.summary.total_differences,
    ),
  ];
  if report.summary.identical {
    lines.push(String::new());
    lines.push("Schemas are identical.".to_string());
    return lines.join("\n") + "\n";
  }
  if !report.tables_added.is_empty() {
    lines.push(String::new());
    lines.push("## Tables Only in Source".to_string());
    for table in &report.tables_added {
      lines.push(format!("- `{table}`"));
    }
  }
  if !report.tables_removed.is_empty() {
    lines.push(String::new());
    lines.push("## Tables Only in Target".to_string());
    for table in &report.tables_removed {
      lines.push(format!("- `{table}`"));
    }
  }
  if !report.tables_changed.is_empty() {
    lines.push(String::new());
    lines.push("## Changed Tables".to_string());
    for table in &report.tables_changed {
      lines.push(String::new());
      lines.push(format!("### `{}`", table.table));
      for col in &table.columns_added {
        lines.push(format!("- + Column `{}` ({})", col.name, col.data_type));
      }
      for col in &table.columns_removed {
        lines.push(format!("- - Column `{}` ({})", col.name, col.data_type));
      }
      for col in &table.columns_changed {
        lines.push(format!(
          "- ~ Column `{}` [{}]: `{}` → `{}`",
          col.name,
          col.changes.join(", "),
          col.source.data_type,
          col.target.data_type
        ));
      }
      if let Some(pk) = &table.primary_key_changed {
        lines.push(format!(
          "- ~ Primary key: ({}) → ({})",
          pk.source.join(", "),
          pk.target.join(", ")
        ));
      }
      for idx in &table.indexes_added {
        lines.push(format!("- + Index `{}` ({})", idx.name, idx.columns.join(", ")));
      }
      for idx in &table.indexes_removed {
        lines.push(format!("- - Index `{}` ({})", idx.name, idx.columns.join(", ")));
      }
      for fk in &table.foreign_keys_added {
        lines.push(format!(
          "- + Foreign key `{}` ({}) → {}({})",
          fk.name,
          fk.columns.join(", "),
          fk.referenced_table,
          fk.referenced_columns.join(", ")
        ));
      }
      for fk in &table.foreign_keys_removed {
        lines.push(format!(
          "- - Foreign key `{}` ({}) → {}({})",
          fk.name,
          fk.columns.join(", "),
          fk.referenced_table,
          fk.referenced_columns.join(", ")
        ));
      }
    }
  }
  lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
  use super::*;

  fn col(
    name: &str,
    data_type: &str,
    nullable: bool,
    pk: bool,
    default: Option<&str>,
  ) -> DiffColumn {
    DiffColumn {
      name: name.to_string(),
      data_type: data_type.to_string(),
      nullable,
      primary_key: pk,
      default: default.map(str::to_string),
    }
  }

  #[test]
  fn detects_added_removed_changed_tables_columns_indexes_and_fks() {
    let source = vec![
      DiffTable {
        schema: Some("dev".into()),
        name: "users".into(),
        columns: vec![
          col("id", "int(11)", false, true, None),
          col("email", "varchar(255)", false, false, None),
          col("status", "varchar(20)", false, false, Some("'active'")),
        ],
        primary_key: vec!["id".into()],
        indexes: vec![DiffIndex {
          name: "idx_email".into(),
          columns: vec!["email".into()],
          unique: true,
        }],
        foreign_keys: vec![],
      },
      DiffTable {
        schema: Some("dev".into()),
        name: "audit_logs".into(),
        columns: vec![col("id", "bigint", false, true, None)],
        primary_key: vec!["id".into()],
        indexes: vec![],
        foreign_keys: vec![],
      },
    ];
    let target = vec![
      DiffTable {
        schema: Some("staging".into()),
        name: "users".into(),
        columns: vec![
          col("id", "int", false, true, None), // MySQL int(11) == int
          col("email", "varchar(120)", true, false, None),
          col("legacy_flag", "tinyint", true, false, None),
        ],
        primary_key: vec!["id".into()],
        indexes: vec![],
        foreign_keys: vec![],
      },
      DiffTable {
        schema: Some("staging".into()),
        name: "deprecated_tmp".into(),
        columns: vec![col("id", "int", false, true, None)],
        primary_key: vec!["id".into()],
        indexes: vec![],
        foreign_keys: vec![],
      },
    ];

    let report = compare_snapshots(&DatabaseType::MySQL, &source, &DatabaseType::MySQL, &target);
    assert!(report.same_dialect);
    assert!(report.ignore_schema);
    assert!(!report.summary.identical);
    assert_eq!(report.tables_added, vec!["dev.audit_logs"]);
    assert_eq!(report.tables_removed, vec!["staging.deprecated_tmp"]);
    assert_eq!(report.tables_changed.len(), 1);
    let users_diff = &report.tables_changed[0];
    assert_eq!(users_diff.columns_added.len(), 1);
    assert_eq!(users_diff.columns_added[0].name, "status");
    assert_eq!(users_diff.columns_removed.len(), 1);
    assert_eq!(users_diff.columns_removed[0].name, "legacy_flag");
    assert_eq!(users_diff.columns_changed.len(), 1);
    assert_eq!(users_diff.columns_changed[0].name, "email");
    assert_eq!(users_diff.columns_changed[0].changes, vec!["type", "nullable"]);
    assert_eq!(users_diff.indexes_added.len(), 1);

    let md = render_markdown("dev-db", "staging-db", &report);
    assert!(md.contains("# Schema Diff: dev-db → staging-db"));
    assert!(md.contains("audit_logs"));
    assert!(md.contains("deprecated_tmp"));
    assert!(md.contains("+ Column `status`"));
  }
}
