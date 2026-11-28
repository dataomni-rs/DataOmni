//! 内存分析沙盒的列级统计画像、直方图与 Top-K 聚合（H17 / P2-2）。

use super::*;

/// 对已装载的分析会话执行全列画像（GUI / CLI / MCP 共用同一实现）
pub fn profile_session(
  analytics_id: &str,
  session: &AnalyticsSession,
) -> Result<ProfileResult, QueryError> {
  let started = Instant::now();
  session.cancelled.store(false, Ordering::SeqCst);
  session.busy.store(true, Ordering::Release);
  let _busy_guard = ResetBusy(&session.busy);

  let conn = session.connection.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;

  let mut profiles = Vec::with_capacity(session.columns.len());
  for plan in &session.columns {
    if session.cancelled.load(Ordering::SeqCst) {
      return Err(QueryError::with_code(ANALYTICS_CANCELLED_CODE, ANALYTICS_CANCELLED_CODE));
    }
    let profile = profile_single_column(&conn, plan, session.row_count as u64)
      .map_err(|err| map_interrupt_error(err, &session.cancelled))?;
    profiles.push(profile);
  }

  Ok(ProfileResult {
    analytics_id: analytics_id.to_string(),
    row_count: session.row_count as u64,
    column_count: profiles.len(),
    elapsed_ms: started.elapsed().as_millis() as u64,
    columns: profiles,
  })
}

pub(super) fn profile_single_column(
  conn: &Connection,
  plan: &ColumnPlan,
  total_rows: u64,
) -> Result<ColumnProfile, QueryError> {
  let col = quote_ident(&plan.name);
  let (present_count, present_ratio, explicit_null_count) = match plan.presence {
    Some(p) => (
      Some(p.present_count),
      Some(safe_ratio(p.present_count, total_rows)),
      Some(p.explicit_null_count),
    ),
    None => (None, None, None),
  };

  if total_rows == 0 {
    return Ok(ColumnProfile {
      name: plan.name.clone(),
      inferred_type: plan.inferred_type,
      duckdb_type: plan.duckdb_type(),
      precision_degraded: plan.precision_degraded,
      total_count: 0,
      null_count: 0,
      null_ratio: 0.0,
      distinct_count: 0,
      distinct_ratio: 0.0,
      present_count,
      present_ratio,
      explicit_null_count,
      min: None,
      max: None,
      mean: None,
      stddev: None,
      p25: None,
      p50: None,
      p75: None,
      p95: None,
      bins: Vec::new(),
      time_bins: Vec::new(),
      min_length: None,
      max_length: None,
      empty_string_count: None,
      padded_whitespace_count: None,
      top_values: Vec::new(),
    });
  }

  match plan.inferred_type {
    ProfileColumnType::Integer | ProfileColumnType::Numeric => profile_numeric_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Binary => profile_binary_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Boolean => profile_boolean_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Date | ProfileColumnType::Time | ProfileColumnType::Datetime => {
      profile_temporal_column(
        conn,
        plan,
        &col,
        total_rows,
        present_count,
        present_ratio,
        explicit_null_count,
      )
    }
    ProfileColumnType::Text | ProfileColumnType::Json | ProfileColumnType::Unknown => {
      profile_text_column(
        conn,
        plan,
        &col,
        total_rows,
        present_count,
        present_ratio,
        explicit_null_count,
      )
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn profile_numeric_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}) AS null_cnt, \
       COUNT(DISTINCT {col}) AS distinct_cnt, \
       MIN({col})::VARCHAR AS min_v, \
       MAX({col})::VARCHAR AS max_v, \
       AVG(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS mean_v, \
       STDDEV_SAMP(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS stddev_v, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.25) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p25, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.50) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p50, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.75) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p75, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.95) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p95, \
       MIN(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS lo, \
       MAX(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS hi, \
       COUNT(*) FILTER (WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE))) AS finite_cnt \
     FROM result_set"
  );

  struct NumericAggregateRow {
    null_count: i64,
    distinct_count: i64,
    min: Option<String>,
    max: Option<String>,
    mean: Option<f64>,
    stddev: Option<f64>,
    p25: Option<f64>,
    p50: Option<f64>,
    p75: Option<f64>,
    p95: Option<f64>,
    lo: Option<f64>,
    hi: Option<f64>,
    finite_cnt: i64,
  }

  let agg = conn
    .query_row(&sql, [], |row| {
      Ok(NumericAggregateRow {
        null_count: row.get(0)?,
        distinct_count: row.get(1)?,
        min: row.get(2)?,
        max: row.get(3)?,
        mean: row.get(4)?,
        stddev: row.get(5)?,
        p25: row.get(6)?,
        p50: row.get(7)?,
        p75: row.get(8)?,
        p95: row.get(9)?,
        lo: row.get(10)?,
        hi: row.get(11)?,
        finite_cnt: row.get(12)?,
      })
    })
    .map_err(duck_error)?;

  let NumericAggregateRow {
    null_count,
    distinct_count,
    min,
    max,
    mean,
    stddev,
    p25,
    p50,
    p75,
    p95,
    lo,
    hi,
    finite_cnt,
  } = agg;

  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);

  let bins = match (lo, hi) {
    (Some(lo), Some(hi)) if lo.is_finite() && hi.is_finite() && finite_cnt > 0 => {
      if (hi - lo).abs() <= f64::EPSILON {
        vec![HistogramBin { lower: lo, upper: hi, count: finite_cnt as u64 }]
      } else {
        query_numeric_histogram(conn, col, lo, hi, HISTOGRAM_BIN_COUNT)?
      }
    }
    _ => Vec::new(),
  };

  // 低基数整数列（如状态码/枚举 ID）一并附带 Top-5 分布，日常排障更有用
  let top_values =
    if plan.inferred_type == ProfileColumnType::Integer && distinct_count <= 20 && non_null > 0 {
      query_top_values(conn, col, non_null)?
    } else {
      Vec::new()
    };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min,
    max,
    mean: finite_opt(mean),
    stddev: finite_opt(stddev),
    p25: finite_opt(p25),
    p50: finite_opt(p50),
    p75: finite_opt(p75),
    p95: finite_opt(p95),
    bins,
    time_bins: Vec::new(),
    min_length: None,
    max_length: None,
    empty_string_count: None,
    padded_whitespace_count: None,
    top_values,
  })
}

pub(super) fn query_numeric_histogram(
  conn: &Connection,
  col: &str,
  lo: f64,
  hi: f64,
  bin_count: usize,
) -> Result<Vec<HistogramBin>, QueryError> {
  let max_bucket = (bin_count.saturating_sub(1)) as i64;
  let span = hi - lo;
  let width = span / bin_count as f64;
  let sql = format!(
    "SELECT \
       LEAST({max_bucket}, GREATEST(0, CAST(FLOOR((CAST({col} AS DOUBLE) - $1) / $2 * {bin_count}) AS BIGINT))) AS bucket, \
       COUNT(*) AS cnt \
     FROM result_set \
     WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE)) \
     GROUP BY bucket \
     ORDER BY bucket"
  );
  let mut counts = vec![0u64; bin_count];
  let mut stmt = conn.prepare(&sql).map_err(duck_error)?;
  let mut rows = stmt.query([lo, span]).map_err(duck_error)?;
  while let Some(row) = rows.next().map_err(duck_error)? {
    let bucket: i64 = row.get(0).map_err(duck_error)?;
    let cnt: i64 = row.get(1).map_err(duck_error)?;
    if let Ok(idx) = usize::try_from(bucket) {
      if idx < bin_count {
        counts[idx] = cnt.max(0) as u64;
      }
    }
  }
  Ok(
    counts
      .into_iter()
      .enumerate()
      .map(|(i, count)| HistogramBin {
        lower: lo + width * i as f64,
        upper: if i + 1 == bin_count { hi } else { lo + width * (i + 1) as f64 },
        count,
      })
      .collect(),
  )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn profile_binary_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}) AS null_cnt, \
       COUNT(DISTINCT {col}) AS distinct_cnt, \
       MIN(octet_length({col})) AS min_len, \
       MAX(octet_length({col})) AS max_len \
     FROM result_set"
  );
  let (null_count, distinct_count, min_len, max_len): (i64, i64, Option<i64>, Option<i64>) = conn
    .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min: None,
    max: None,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    time_bins: Vec::new(),
    min_length: min_len.map(|v| v.max(0) as u64),
    max_length: max_len.map(|v| v.max(0) as u64),
    empty_string_count: None,
    padded_whitespace_count: None,
    top_values: Vec::new(),
  })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn profile_boolean_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!("SELECT COUNT(*) - COUNT({col}), COUNT(DISTINCT {col}) FROM result_set");
  let (null_count, distinct_count): (i64, i64) =
    conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min: None,
    max: None,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    time_bins: Vec::new(),
    min_length: None,
    max_length: None,
    empty_string_count: None,
    padded_whitespace_count: None,
    top_values,
  })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn profile_temporal_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}), \
       COUNT(DISTINCT {col}), \
       MIN({col})::VARCHAR, \
       MAX({col})::VARCHAR \
     FROM result_set"
  );
  let (null_count, distinct_count, min, max): (i64, i64, Option<String>, Option<String>) = conn
    .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };
  let time_bins =
    if matches!(plan.inferred_type, ProfileColumnType::Date | ProfileColumnType::Datetime)
      && distinct_count >= 2
    {
      query_temporal_histogram(conn, col, 12)?
    } else {
      Vec::new()
    };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min,
    max,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    time_bins,
    min_length: None,
    max_length: None,
    empty_string_count: None,
    padded_whitespace_count: None,
    top_values,
  })
}

pub(super) fn query_temporal_histogram(
  conn: &Connection,
  col: &str,
  max_bins: usize,
) -> Result<Vec<TimeHistogramBin>, QueryError> {
  let bounds_sql = format!(
    "SELECT MIN(epoch(CAST({col} AS TIMESTAMP))), MAX(epoch(CAST({col} AS TIMESTAMP))) \
     FROM result_set WHERE {col} IS NOT NULL"
  );
  let (lo, hi): (Option<f64>, Option<f64>) =
    conn.query_row(&bounds_sql, [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(duck_error)?;
  let (Some(lo), Some(hi)) = (lo, hi) else {
    return Ok(Vec::new());
  };
  let span = hi - lo;
  if !span.is_finite() || span <= 0.0 || max_bins < 2 {
    return Ok(Vec::new());
  }

  let bin_count = max_bins;
  let max_bucket = (bin_count - 1) as i64;
  let width = span / bin_count as f64;
  let label_fmt = if span > 365.0 * 86_400.0 {
    "%Y-%m"
  } else if span > 3.0 * 86_400.0 {
    "%Y-%m-%d"
  } else if span > 2.0 * 3_600.0 {
    "%m-%d %H:00"
  } else {
    "%H:%M:%S"
  };

  let bucket_sql = format!(
    "SELECT \
       LEAST({max_bucket}, GREATEST(0, CAST(FLOOR((epoch(CAST({col} AS TIMESTAMP)) - $1) / $2 * {bin_count}) AS BIGINT))) AS bucket, \
       COUNT(*) AS cnt \
     FROM result_set \
     WHERE {col} IS NOT NULL \
     GROUP BY bucket \
     ORDER BY bucket"
  );
  let mut counts = vec![0u64; bin_count];
  let mut stmt = conn.prepare(&bucket_sql).map_err(duck_error)?;
  let mut rows = stmt.query([lo, span]).map_err(duck_error)?;
  while let Some(row) = rows.next().map_err(duck_error)? {
    let bucket: i64 = row.get(0).map_err(duck_error)?;
    let cnt: i64 = row.get(1).map_err(duck_error)?;
    if let Ok(idx) = usize::try_from(bucket) {
      if idx < bin_count {
        counts[idx] = cnt.max(0) as u64;
      }
    }
  }

  let label_sql = format!("SELECT strftime(to_timestamp($1), '{label_fmt}')");
  let mut label_stmt = conn.prepare(&label_sql).map_err(duck_error)?;
  let mut out = Vec::with_capacity(bin_count);
  for (idx, count) in counts.into_iter().enumerate() {
    let ts = lo + width * idx as f64;
    let label: String =
      label_stmt.query_row([ts], |r| r.get(0)).unwrap_or_else(|_| format!("#{}", idx + 1));
    out.push(TimeHistogramBin { label, count });
  }
  Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn profile_text_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}), \
       COUNT(DISTINCT {col}), \
       MIN({col}), \
       MAX({col}), \
       MIN(length({col})), \
       MAX(length({col})), \
       CAST(COALESCE(SUM(CASE WHEN {col} = '' THEN 1 ELSE 0 END), 0) AS BIGINT), \
       CAST(COALESCE(SUM(CASE WHEN {col} IS NOT NULL AND {col} <> '' AND trim({col}) <> {col} THEN 1 ELSE 0 END), 0) AS BIGINT) \
     FROM result_set"
  );
  struct TextColumnAgg {
    null_count: i64,
    distinct_count: i64,
    min: Option<String>,
    max: Option<String>,
    min_len: Option<i64>,
    max_len: Option<i64>,
    empty_strings: i64,
    padded_ws: i64,
  }
  let agg = conn
    .query_row(&sql, [], |r| {
      Ok(TextColumnAgg {
        null_count: r.get(0)?,
        distinct_count: r.get(1)?,
        min: r.get(2)?,
        max: r.get(3)?,
        min_len: r.get(4)?,
        max_len: r.get(5)?,
        empty_strings: r.get(6)?,
        padded_ws: r.get(7)?,
      })
    })
    .map_err(duck_error)?;
  let null_count = agg.null_count.max(0) as u64;
  let distinct_count = agg.distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min: agg.min,
    max: agg.max,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    time_bins: Vec::new(),
    min_length: agg.min_len.map(|v| v.max(0) as u64),
    max_length: agg.max_len.map(|v| v.max(0) as u64),
    empty_string_count: Some(agg.empty_strings.max(0) as u64),
    padded_whitespace_count: Some(agg.padded_ws.max(0) as u64),
    top_values,
  })
}

pub(super) fn query_top_values(
  conn: &Connection,
  col: &str,
  non_null_total: u64,
) -> Result<Vec<TopValueEntry>, QueryError> {
  let sql = format!(
    "SELECT CAST({col} AS VARCHAR) AS val, COUNT(*) AS cnt \
     FROM result_set \
     WHERE {col} IS NOT NULL \
     GROUP BY {col} \
     ORDER BY cnt DESC, val ASC \
     LIMIT {TOP_VALUES_LIMIT}"
  );
  let mut stmt = conn.prepare(&sql).map_err(duck_error)?;
  let mut rows = stmt.query([]).map_err(duck_error)?;
  let mut out = Vec::with_capacity(TOP_VALUES_LIMIT);
  while let Some(row) = rows.next().map_err(duck_error)? {
    let value: String = row.get(0).map_err(duck_error)?;
    let count: i64 = row.get(1).map_err(duck_error)?;
    let count_u = count.max(0) as u64;
    out.push(TopValueEntry { value, count: count_u, ratio: safe_ratio(count_u, non_null_total) });
  }
  Ok(out)
}
