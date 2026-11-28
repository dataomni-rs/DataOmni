use futures_util::TryStreamExt;
use serde::Serialize;
use serde_json::{Map, Value as JsonValue};
use sqlx::{
  Column, Connection, Executor, MySql, MySqlConnection, PgConnection, Pool, Postgres, Row, Sqlite,
  SqliteConnection, Statement, TypeInfo,
};
use std::future::Future;

use crate::services::{query_parameters, DbPool};
use tokio::time::{timeout, Duration};

/// 查询超时。数据是配置的毫秒数——`code` 字段另有 `QUERY_TIMEOUT_CODE`，
/// 这里管的是**消息**，此前它是一句中文
pub const QUERY_TIMEOUT: &str = "DATAOMNI_QUERY_TIMEOUT";
/// 某一列解码不出来。数据是 `列名 · 数据库给的类型 · 驱动的原因`。
/// 三种库共用一条：对用户来说「这一列读不出来」是同一件事
pub const COLUMN_DECODE_FAILED: &str = "DATAOMNI_COLUMN_DECODE_FAILED";

/// 解码不了的列类型。数据里带的是数据库给的类型名——三种库共用一条：
/// 对用户来说「这一列读不出来」是同一件事，而类型名已经说明了是哪种库
pub const UNSUPPORTED_COLUMN_TYPE: &str = "DATAOMNI_UNSUPPORTED_COLUMN_TYPE";

pub use crate::services::query_error::{QueryError, QUERY_TIMEOUT_CODE};
pub const DEFAULT_QUERY_ROW_LIMIT: usize = 1_000;
pub const DEFAULT_QUERY_BATCH_SIZE: usize = 250;
pub const DEFAULT_QUERY_BYTE_LIMIT: usize = 16 * 1024 * 1024;

pub type QueryRow = Map<String, JsonValue>;

#[derive(Debug, Clone, Serialize)]
pub struct QueryColumnMetadata {
  pub name: String,
  pub ordinal: usize,
  pub database_type: String,
  pub logical_type: String,
  pub nullable: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResultBatch {
  pub index: usize,
  pub offset: usize,
  pub rows: Vec<QueryRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryTruncationReason {
  RowLimit,
  ByteLimit,
}

/// 一条语句返回的第二个及以后的结果集（`EXEC` 过程、`sp_help`、`CALL`）。
///
/// 第一个照常流式交给前端；后面的读进内存随摘要带回。行数上限按每个结果集算，
/// 字节上限所有后续结果集合用一份——一个过程吐出几十万行，不能整份进内存
#[derive(Debug, Serialize)]
pub struct ExtraResultSet {
  pub columns: Vec<String>,
  pub column_metadata: Vec<QueryColumnMetadata>,
  pub rows: Vec<QueryRow>,
  pub truncated: bool,
  pub truncation_reason: Option<QueryTruncationReason>,
}

/// 最多带回几个后续结果集；再往后的只计数（界面上每个是一张表，二十张已经翻不过来）
pub const MAX_EXTRA_RESULT_SETS: usize = 20;

/// 边读边收后续结果集。超过 [`MAX_EXTRA_RESULT_SETS`] 个、或字节预算用完之后开始的，只计数
pub(crate) struct ExtraResultSets {
  sets: Vec<ExtraResultSet>,
  omitted: usize,
  row_limit: usize,
  byte_limit: usize,
  bytes_read: usize,
  out_of_bytes: bool,
  /// 正在读的那个结果集收不收（没收的只计了数，它的行读掉）
  collecting: bool,
}

impl ExtraResultSets {
  pub(crate) fn new(options: &StreamOptions) -> Self {
    Self {
      sets: Vec::new(),
      omitted: 0,
      row_limit: options.row_limit,
      byte_limit: options.byte_limit,
      bytes_read: 0,
      out_of_bytes: false,
      collecting: false,
    }
  }

  /// 一个新的结果集开始了
  pub(crate) fn start(&mut self, column_metadata: Vec<QueryColumnMetadata>) {
    if self.sets.len() >= MAX_EXTRA_RESULT_SETS || self.out_of_bytes {
      self.omitted += 1;
      self.collecting = false;
      return;
    }
    self.sets.push(ExtraResultSet {
      columns: column_metadata.iter().map(|column| column.name.clone()).collect(),
      column_metadata,
      rows: Vec::new(),
      truncated: false,
      truncation_reason: None,
    });
    self.collecting = true;
  }

  /// 正在读的那个结果集的一行
  pub(crate) fn push(&mut self, row: QueryRow) -> Result<(), QueryError> {
    if !self.collecting {
      return Ok(());
    }
    let Some(set) = self.sets.last_mut() else {
      return Ok(());
    };
    if set.truncated {
      return Ok(());
    }
    if set.rows.len() >= self.row_limit {
      set.truncated = true;
      set.truncation_reason = Some(QueryTruncationReason::RowLimit);
      return Ok(());
    }
    if !admit_row_bytes(&row, self.byte_limit, &mut self.bytes_read)? {
      set.truncated = true;
      set.truncation_reason = Some(QueryTruncationReason::ByteLimit);
      self.out_of_bytes = true;
      return Ok(());
    }
    set.rows.push(row);
    Ok(())
  }

  /// 带回来的结果集，与没带回来的个数
  pub(crate) fn finish(self) -> (Vec<ExtraResultSet>, usize) {
    (self.sets, self.omitted)
  }
}

/// 语句不返回结果集时怎么办。
///
/// 谁也无法只看 SQL 文本就断定它返不返回行——`WITH ... SELECT` 返回，
/// `WITH ... DELETE` 不返回，而两者开头一样。唯一权威的回答来自数据库自己：
/// `describe` 报了 0 列就是不返回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonQueryHandling {
  /// 照常执行，回报受影响行数。编辑器里执行一条 UPDATE 走这条。
  Execute,
  /// 拒绝执行。导出走这条：点一下「导出」不该让一条 DELETE 真把数据删掉。
  Refuse,
}

/// 流式读的四个旋钮。
///
/// 合成一个结构不是为了抽象，是因为位置参数已经排到第六个——
/// `(connection, sql, 100000, 16777216, 250, sink)` 里哪个是哪个，读的人得回去数。
/// 这一次执行是不是在取执行计划、取哪一种。只对 SQL Server 有区别：估算的用
/// `SET SHOWPLAN_XML`（只编译不执行），实际的用 `SET STATISTICS XML`（真的执行）。
/// 别的方言的「实际」写在 EXPLAIN 语句里（`EXPLAIN ANALYZE`），这里只当「在取计划」
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplainPlan {
  #[default]
  Off,
  Estimated,
  Actual,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamOptions {
  pub row_limit: usize,
  pub byte_limit: usize,
  pub batch_size: usize,
  pub non_query: NonQueryHandling,
  /// 这是取执行计划的那一次执行。各家要照顾的地方不同：
  ///
  /// - MySQL 在预处理 `EXPLAIN FORMAT=JSON` 时报告 **0 列**，而它执行起来确实
  ///   返回一行 JSON。按 describe 的说法走会拿回一个 `Affected`，执行计划就此
  ///   消失——所以 describe 说「没有列」时仍然去取结果集。
  /// - SQL Server 没有 `EXPLAIN`：语句原样，前后各发一批 `SET SHOWPLAN_XML`，
  ///   服务端只编译不执行，返回一份 XML 计划（[`ExplainPlan::Actual`] 时换成
  ///   `SET STATISTICS XML`，见 `SqlServerConnection::actual_plan`）。
  pub explain_plan: ExplainPlan,
}

impl StreamOptions {
  pub fn limited(row_limit: usize, byte_limit: usize, batch_size: usize) -> Self {
    Self {
      row_limit,
      byte_limit,
      batch_size,
      non_query: NonQueryHandling::Execute,
      explain_plan: ExplainPlan::Off,
    }
  }

  /// 见 [`StreamOptions::explain_plan`]
  pub fn with_explain_plan(mut self, plan: ExplainPlan) -> Self {
    self.explain_plan = plan;
    self
  }

  /// 导出用：行数与字节都不设限，且拒绝执行不返回结果集的语句。
  pub fn unlimited_export(batch_size: usize) -> Self {
    Self {
      row_limit: usize::MAX,
      byte_limit: usize::MAX,
      batch_size,
      non_query: NonQueryHandling::Refuse,
      explain_plan: ExplainPlan::Off,
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryExecutionSummary {
  Rows {
    columns: Vec<String>,
    column_metadata: Vec<QueryColumnMetadata>,
    row_count: usize,
    batch_count: usize,
    truncated: bool,
    truncation_reason: Option<QueryTruncationReason>,
    row_limit: usize,
    byte_limit: usize,
    bytes_read: usize,
    /// 第一个之后还有几个结果集没带回来（超过个数或字节上限，或 MySQL 的空结果集：
    /// sqlx 给不出它的列）。界面上要说少了东西
    omitted_result_sets: usize,
    /// 第一个之后的结果集
    extra_result_sets: Vec<ExtraResultSet>,
  },
  Affected {
    rows_affected: u64,
  },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryExecutionResult {
  Rows {
    columns: Vec<String>,
    column_metadata: Vec<QueryColumnMetadata>,
    rows: Vec<Map<String, JsonValue>>,
    truncated: bool,
    truncation_reason: Option<QueryTruncationReason>,
    row_limit: usize,
    byte_limit: usize,
    bytes_read: usize,
    /// 第一个之后还有几个结果集没带回来（超过个数或字节上限，或 MySQL 的空结果集：
    /// sqlx 给不出它的列）。界面上要说少了东西
    omitted_result_sets: usize,
    /// 第一个之后的结果集
    extra_result_sets: Vec<ExtraResultSet>,
  },
  Affected {
    rows_affected: u64,
  },
}

pub async fn execute_query(pool: &DbPool, sql: &str) -> Result<QueryExecutionResult, QueryError> {
  execute_query_with_limit(pool, sql, DEFAULT_QUERY_ROW_LIMIT).await
}

pub async fn execute_query_with_limit(
  pool: &DbPool,
  sql: &str,
  row_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  execute_query_with_limits(pool, sql, row_limit, DEFAULT_QUERY_BYTE_LIMIT).await
}

pub async fn execute_query_with_limits(
  pool: &DbPool,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  match pool {
    DbPool::Sqlite(pool) => execute_sqlite(pool, sql, row_limit, byte_limit).await,
    DbPool::MySql(pool) => execute_mysql(pool, sql, row_limit, byte_limit).await,
    DbPool::Postgres(pool) => execute_postgres(pool, sql, row_limit, byte_limit).await,
  }
}

/// 一条连接从哪个池子来：插件管的 sqlx 三家，或者后端自己持有的 SQL Server。
///
/// 插件的 `DbPool` 是别人的枚举，加不进第四个变体，所以包一层。只有要
/// **开一条会话连接**的地方需要它（`SessionConnection::acquire`）；只接受
/// `&DbPool` 的调用方经 `From` 照旧编译。
#[derive(Clone, Copy)]
pub enum PoolRef<'a> {
  Sqlx(&'a DbPool),
  SqlServer(&'a std::sync::Arc<crate::services::sql_server::SqlServerPool>),
  Oracle(&'a std::sync::Arc<crate::services::oracle::OraclePool>),
  Dameng(&'a std::sync::Arc<crate::services::dameng::DamengPool>),
  DuckDb(&'a std::sync::Arc<crate::services::duckdb::DuckDbPool>),
  ClickHouse(&'a std::sync::Arc<crate::services::clickhouse::ClickHousePool>),
}

impl<'a> From<&'a DbPool> for PoolRef<'a> {
  fn from(pool: &'a DbPool) -> Self {
    Self::Sqlx(pool)
  }
}

pub enum SessionConnection {
  Sqlite(sqlx::pool::PoolConnection<Sqlite>),
  MySql(sqlx::pool::PoolConnection<MySql>),
  Postgres(sqlx::pool::PoolConnection<Postgres>),
  /// 装箱：tiberius 的客户端比 sqlx 的池连接大得多，不装箱的话每个变体都按它付内存
  SqlServer(Box<crate::services::sql_server::SqlServerConnection>),
  Oracle(Box<crate::services::oracle::OracleConnection>),
  Dameng(Box<crate::services::dameng::DamengConnection>),
  DuckDb(crate::services::duckdb::DuckDbConnection),
  ClickHouse(crate::services::clickhouse::ClickHouseConnection),
}

impl SessionConnection {
  pub async fn acquire<'a>(pool: impl Into<PoolRef<'a>>) -> Result<Self, QueryError> {
    // 走 `QueryError::from`：池已关、socket 断了要带上 CONNECTION_LOST，界面才会说「重连再试」
    match pool.into() {
      PoolRef::Sqlx(DbPool::Sqlite(pool)) => {
        pool.acquire().await.map(Self::Sqlite).map_err(QueryError::from)
      }
      PoolRef::Sqlx(DbPool::MySql(pool)) => {
        pool.acquire().await.map(Self::MySql).map_err(QueryError::from)
      }
      PoolRef::Sqlx(DbPool::Postgres(pool)) => {
        pool.acquire().await.map(Self::Postgres).map_err(QueryError::from)
      }
      PoolRef::SqlServer(pool) => {
        pool.acquire_for_session().await.map(|connection| Self::SqlServer(Box::new(connection)))
      }
      PoolRef::Oracle(pool) => {
        pool.acquire_for_session().await.map(|connection| Self::Oracle(Box::new(connection)))
      }
      PoolRef::Dameng(pool) => {
        pool.acquire_for_session().await.map(|connection| Self::Dameng(Box::new(connection)))
      }
      PoolRef::DuckDb(pool) => pool.acquire_for_session().map(Self::DuckDb),
      PoolRef::ClickHouse(pool) => Ok(Self::ClickHouse(pool.acquire_for_session())),
    }
  }

  pub async fn execute(
    &mut self,
    sql: &str,
    row_limit: usize,
  ) -> Result<QueryExecutionResult, QueryError> {
    match self {
      Self::Sqlite(connection) => execute_sqlite_connection(connection, sql, row_limit).await,
      Self::MySql(connection) => execute_mysql_connection(connection, sql, row_limit).await,
      Self::Postgres(connection) => execute_postgres_connection(connection, sql, row_limit).await,
      Self::SqlServer(connection) => {
        let mut rows = Vec::new();
        let summary = connection
          .execute_streaming(
            sql,
            StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
            &mut |batch| {
              rows.extend(batch.rows);
              Ok(())
            },
          )
          .await?;
        summary_with_rows(summary, rows)
      }
      Self::Oracle(connection) => {
        let mut rows = Vec::new();
        let summary = connection
          .execute_streaming(
            sql,
            StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
            &mut |batch| {
              rows.extend(batch.rows);
              Ok(())
            },
          )
          .await?;
        summary_with_rows(summary, rows)
      }
      Self::Dameng(connection) => {
        let mut rows = Vec::new();
        let summary = connection
          .execute_streaming(
            sql,
            StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
            &mut |batch| {
              rows.extend(batch.rows);
              Ok(())
            },
          )
          .await?;
        summary_with_rows(summary, rows)
      }
      Self::DuckDb(connection) => {
        let mut rows = Vec::new();
        let summary = connection
          .execute_streaming(
            sql,
            StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
            &mut |batch| {
              rows.extend(batch.rows);
              Ok(())
            },
          )
          .await?;
        summary_with_rows(summary, rows)
      }
      Self::ClickHouse(connection) => {
        let mut rows = Vec::new();
        let summary = connection
          .execute_streaming(
            sql,
            StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
            &mut |batch| {
              rows.extend(batch.rows);
              Ok(())
            },
          )
          .await?;
        summary_with_rows(summary, rows)
      }
    }
  }

  /// 带 JSON 绑定参数执行只读查询并返回完整结果集（参数为空时直接走无参 `execute`）
  pub async fn execute_with_json_params(
    &mut self,
    sql: &str,
    params: &[JsonValue],
    row_limit: usize,
  ) -> Result<QueryExecutionResult, QueryError> {
    if params.is_empty() {
      return self.execute(sql, row_limit).await;
    }
    let mut rows = Vec::new();
    let summary = self
      .execute_streaming_with_params(
        sql,
        params,
        StreamOptions::limited(row_limit, DEFAULT_QUERY_BYTE_LIMIT, row_limit.max(1)),
        &mut |batch| {
          rows.extend(batch.rows);
          Ok(())
        },
      )
      .await?;
    summary_with_rows(summary, rows)
  }

  /// 只问「这条语句返回哪些列」，不取任何一行。
  ///
  /// 导出要先写 CSV 表头，而表头必须在第一批数据之前就写出去。三种驱动的
  /// `describe` 都是 prepare 而非执行，所以拿列名不会让语句真的跑起来。
  pub async fn describe_columns(
    &mut self,
    sql: &str,
  ) -> Result<Vec<QueryColumnMetadata>, QueryError> {
    match self {
      Self::Sqlite(connection) => describe_sqlite_columns(connection, sql).await,
      Self::MySql(connection) => describe_mysql_columns(connection, sql).await,
      Self::Postgres(connection) => {
        describe_postgres_columns(connection, sql).await.map(|(columns, _, _)| columns)
      }
      Self::SqlServer(connection) => connection.describe_columns(sql).await,
      Self::Oracle(connection) => connection.describe_columns(sql).await,
      Self::Dameng(connection) => connection.describe_columns(sql).await,
      Self::DuckDb(connection) => connection.describe_columns(sql).await,
      Self::ClickHouse(connection) => connection.describe_columns(sql).await,
    }
  }

  /// 在这条连接上执行一条带参数的语句，返回影响行数。
  ///
  /// 与 `execute` 的区别只在参数，而这个区别是导入的全部安全性所在：CSV 里的
  /// 一个单引号拼进 SQL 就能把语句改写成别的意思。值一律是 `Option<String>`——
  /// CSV 给出来的本来就是文本与「没有值」两种，再包一层 JSON 只是多一次转换。
  pub async fn execute_with_params(
    &mut self,
    sql: &str,
    params: &[Option<String>],
  ) -> Result<u64, QueryError> {
    match self {
      Self::Sqlite(connection) => {
        let mut query = sqlx::query(sql);
        for param in params {
          query = query.bind(param.clone());
        }
        connection.execute(query).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::MySql(connection) => {
        let mut query = sqlx::query(sql);
        for param in params {
          query = query.bind(param.clone());
        }
        connection.execute(query).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::Postgres(connection) => {
        let mut query = sqlx::query(sql);
        for param in params {
          query = query.bind(param.clone());
        }
        connection.execute(query).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::SqlServer(connection) => connection.execute_with_params(sql, params).await,
      // 数组 DML：`sql` 是单行的语句，`params` 是一行接一行的值
      Self::Oracle(connection) => connection.execute_with_params(sql, params).await,
      Self::Dameng(connection) => connection.execute_with_params(sql, params).await,
      Self::DuckDb(connection) => connection.execute_with_params(sql, params).await,
      // 没有事务：导入做不到「中途失败什么都不留」，界面上本来就没有这个入口
      Self::ClickHouse(_) => {
        Err(QueryError::message(crate::services::clickhouse::CLICKHOUSE_WRITE_UNSUPPORTED))
      }
    }
  }

  /// 不经预处理直接发一条语句，返回影响行数。
  ///
  /// 事务与保存点控制必须走这条路：MySQL 的预处理协议**不收** `BEGIN` 与
  /// `SAVEPOINT` 这类命令，报的是 1295「This command is not supported in the
  /// prepared statement protocol yet」——一句完全不指向真正原因的错。
  /// 这些语句本来也没有参数，预处理对它们只有坏处。
  pub async fn execute_unprepared(&mut self, sql: &str) -> Result<u64, QueryError> {
    match self {
      Self::Sqlite(connection) => {
        connection.execute(sql).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::MySql(connection) => {
        connection.execute(sql).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::Postgres(connection) => {
        connection.execute(sql).await.map(|done| done.rows_affected()).map_err(display_error)
      }
      Self::SqlServer(connection) => connection.execute_batch(sql).await,
      Self::Oracle(connection) => connection.execute_batch(sql).await,
      Self::Dameng(connection) => connection.execute_batch(sql).await,
      Self::DuckDb(connection) => connection.execute_batch(sql).await,
      Self::ClickHouse(connection) => connection.execute_batch(sql).await,
    }
  }

  /// 这个方言会不会因为一条语句出错就把整个事务废掉。
  ///
  /// PostgreSQL 是：事务里任何一条语句报错之后，后续语句一律 25P02，
  /// 只有回滚能出去。DuckDB 同样（「Current transaction is aborted (please
  /// ROLLBACK)」，实验过）。MySQL 与 SQLite 不是这样，在那两家标成「事务已失败」
  /// 是在说假话。
  pub fn aborts_transaction_on_error(&self) -> bool {
    matches!(self, Self::Postgres(_) | Self::DuckDb(_))
  }

  /// 这个方言的 DDL 会不会隐式提交当前事务。
  ///
  /// 只有 MySQL 是。PostgreSQL 与 SQLite 的 DDL 在事务里，`DROP TABLE`
  /// 能回滚。见 `transaction_state::commits_implicitly`。
  pub fn commits_implicitly_on_ddl(&self) -> bool {
    matches!(self, Self::MySql(_))
  }

  /// 这条连接还答不答话。
  ///
  /// 只有 sqlx 那两家网络库要问：VPN、NAT、云负载均衡会把空闲了几分钟的 TCP 流
  /// 悄悄丢掉，不发 RST——实测经 Shadowrocket 隧道空闲 3 分钟还在、6 分钟就没了。
  /// 那之后第一条语句会一直等到查询超时。先用一个短超时的 ping 问一句，答不上
  /// 就换一条新的，而不是让用户等满超时再看一句「连接丢了」。
  /// SQLite 在本机，不问。
  pub async fn responds_within(&mut self, limit: Duration) -> bool {
    let ping = match self {
      Self::MySql(connection) => timeout(limit, connection.ping()).await,
      Self::Postgres(connection) => timeout(limit, connection.ping()).await,
      Self::SqlServer(connection) => return connection.ping(limit).await,
      Self::Oracle(connection) => return connection.ping(limit).await,
      Self::Dameng(connection) => return connection.ping(limit).await,
      // 本机的库，没有会被网络悄悄丢掉的连接。ClickHouse 走 HTTP，每次请求自己连
      Self::Sqlite(_) | Self::DuckDb(_) | Self::ClickHouse(_) => return true,
    };
    matches!(ping, Ok(Ok(())))
  }

  /// 丢掉一条判死了的连接。sqlx 的池连接要先 detach：直接 drop 会把它还回池里，
  /// 下一个取连接的人会对着它 ping，又一次等满超时
  pub fn discard(self) {
    match self {
      Self::MySql(connection) => drop(connection.detach()),
      Self::Postgres(connection) => drop(connection.detach()),
      Self::Sqlite(connection) => drop(connection.detach()),
      Self::SqlServer(_)
      | Self::Oracle(_)
      | Self::Dameng(_)
      | Self::DuckDb(_)
      | Self::ClickHouse(_) => {}
    }
  }

  /// 关掉自动提交时补的那一句。T-SQL 里单独一个 `BEGIN` 是语句块的开头，
  /// 不开事务
  ///
  /// Oracle 与达梦 ODBC 没有这一句：事务随第一条 DML 开始，自动提交由连接自己按开关决定
  pub fn begin_statement(&self) -> Option<&'static str> {
    match self {
      Self::SqlServer(_) => Some("BEGIN TRANSACTION"),
      // ClickHouse 没有通用事务，自动提交的开关在界面上不出现
      Self::Oracle(_) | Self::Dameng(_) | Self::ClickHouse(_) => None,
      _ => Some("BEGIN"),
    }
  }

  /// 把这一次执行的自动提交开关交给连接。Oracle 与达梦 ODBC 在连接层控制自动提交
  pub fn set_autocommit(&mut self, autocommit: bool) {
    match self {
      Self::Oracle(connection) => connection.set_autocommit(autocommit),
      Self::Dameng(connection) => connection.set_autocommit(autocommit),
      _ => {}
    }
  }

  /// 这条语句自己就在开、关事务，前面不用补 `BEGIN`
  pub fn controls_transaction(&self, sql: &str) -> bool {
    use crate::services::transaction_state::{transaction_effect, TransactionEffect};
    match self {
      Self::SqlServer(_) => crate::services::sql_server::controls_transaction(sql),
      Self::Oracle(_) => crate::services::oracle::controls_transaction(sql),
      Self::Dameng(_) => crate::services::dameng::controls_transaction(sql),
      _ => transaction_effect(sql) != TransactionEffect::None,
    }
  }

  /// 服务端报的事务状态。只有 SQL Server 有（`@@TRANCOUNT`）；另外三家
  /// 没有可移植的查法，状态由 `TransactionState` 从语句推出来
  pub fn observed_transaction(
    &self,
  ) -> Option<crate::services::transaction_state::TransactionState> {
    match self {
      Self::SqlServer(connection) => Some(connection.transaction()),
      Self::Oracle(connection) => Some(connection.transaction()),
      Self::Dameng(connection) => Some(connection.transaction()),
      _ => None,
    }
  }

  pub async fn execute_streaming(
    &mut self,
    sql: &str,
    options: StreamOptions,
    sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
  ) -> Result<QueryExecutionSummary, QueryError> {
    self.execute_streaming_with_params(sql, &[], options, sink).await
  }

  /// 带编辑器里的绑定参数执行（rfcs/bind-parameters.md）。`params` 为空时与不带参数完全一样
  pub async fn execute_streaming_with_params(
    &mut self,
    sql: &str,
    params: &[JsonValue],
    options: StreamOptions,
    sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
  ) -> Result<QueryExecutionSummary, QueryError> {
    match self {
      Self::Sqlite(connection) => {
        execute_sqlite_connection_streaming(connection, sql, params, options, sink).await
      }
      Self::MySql(connection) => {
        execute_mysql_connection_streaming(connection, sql, params, options, sink).await
      }
      Self::Postgres(connection) => {
        execute_postgres_connection_streaming(connection, sql, params, options, sink).await
      }
      Self::SqlServer(connection) => {
        connection.execute_streaming_with_params(sql, params, options, sink).await
      }
      Self::Oracle(connection) => {
        connection.execute_streaming_with_params(sql, params, options, sink).await
      }
      Self::Dameng(connection) => {
        connection.execute_streaming_with_params(sql, params, options, sink).await
      }
      Self::DuckDb(connection) => {
        connection.execute_streaming_with_params(sql, params, options, sink).await
      }
      Self::ClickHouse(_) if !params.is_empty() => {
        Err(QueryError::message(query_parameters::PARAMETERS_UNSUPPORTED_DIALECT))
      }
      Self::ClickHouse(connection) => connection.execute_streaming(sql, options, sink).await,
    }
  }
}

pub async fn execute_query_with_timeout(
  pool: &DbPool,
  sql: &str,
  timeout_duration: Duration,
) -> Result<QueryExecutionResult, QueryError> {
  with_timeout(execute_query(pool, sql), timeout_duration).await
}

async fn with_timeout<F, T>(future: F, timeout_duration: Duration) -> Result<T, QueryError>
where
  F: Future<Output = Result<T, QueryError>>,
{
  timeout(timeout_duration, future).await.map_err(|_| {
    QueryError::with_code(
      QUERY_TIMEOUT_CODE,
      format!("{QUERY_TIMEOUT}: {}", timeout_duration.as_millis()),
    )
  })?
}

async fn execute_sqlite(
  pool: &Pool<Sqlite>,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut connection = pool.acquire().await.map_err(QueryError::from)?;
  execute_sqlite_connection_with_limits(&mut connection, sql, row_limit, byte_limit).await
}

async fn execute_sqlite_connection(
  connection: &mut SqliteConnection,
  sql: &str,
  row_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  execute_sqlite_connection_with_limits(connection, sql, row_limit, DEFAULT_QUERY_BYTE_LIMIT).await
}

async fn execute_sqlite_connection_with_limits(
  connection: &mut SqliteConnection,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut rows = Vec::new();
  let summary = execute_sqlite_connection_streaming(
    connection,
    sql,
    &[],
    StreamOptions::limited(row_limit, byte_limit, row_limit.max(1)),
    &mut |batch| {
      rows.extend(batch.rows);
      Ok(())
    },
  )
  .await?;
  summary_with_rows(summary, rows)
}

/// 先清掉语句缓存，理由同 [`describe_postgres_columns`]，后果更重：SQLite 遇到结构变化会自己
/// 重新准备，而 sqlx 还按缓存里旧的列数取值——加了一列之后再跑同一句 `SELECT *`，sqlx 的
/// 工作线程下标越界 panic，这一句返回空结果。SQLite 在本机，清缓存没有往返
async fn describe_sqlite_columns(
  connection: &mut SqliteConnection,
  sql: &str,
) -> Result<Vec<QueryColumnMetadata>, QueryError> {
  connection.clear_cached_statements().await.map_err(QueryError::from)?;
  let description = (&mut *connection).describe(sql).await.map_err(QueryError::from)?;
  let names = number_duplicate_columns(description.columns().iter().map(|column| column.name()));
  Ok(
    description
      .columns()
      .iter()
      .zip(names)
      .enumerate()
      .map(|(ordinal, (column, name))| QueryColumnMetadata {
        name,
        ordinal,
        database_type: column.type_info().name().to_string(),
        logical_type: sqlite_logical_type(column.type_info().name()).to_string(),
        nullable: description.nullable(ordinal),
      })
      .collect(),
  )
}

async fn execute_sqlite_connection_streaming(
  connection: &mut SqliteConnection,
  sql: &str,
  params: &[JsonValue],
  options: StreamOptions,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<QueryExecutionSummary, QueryError> {
  if is_transaction_control_statement(sql) {
    refuse_non_query(options.non_query)?;
    let result = (&mut *connection).execute(sql).await.map_err(QueryError::from)?;
    return Ok(QueryExecutionSummary::Affected { rows_affected: result.rows_affected() });
  }

  let column_metadata = describe_sqlite_columns(connection, sql).await?;
  let columns = column_metadata.iter().map(|column| column.name.clone()).collect::<Vec<_>>();

  if columns.is_empty() && options.explain_plan == ExplainPlan::Off {
    refuse_non_query(options.non_query)?;
    let result = if params.is_empty() {
      (&mut *connection).execute(sql).await
    } else {
      (&mut *connection).execute(query_parameters::bind_sqlite(sqlx::query(sql), params)?).await
    }
    .map_err(QueryError::from)?;
    return Ok(QueryExecutionSummary::Affected { rows_affected: result.rows_affected() });
  }

  let mut stream =
    (&mut *connection).fetch(query_parameters::bind_sqlite(sqlx::query(sql), params)?);
  let mut rows = Vec::with_capacity(options.batch_size);
  let mut row_count = 0;
  let mut batch_count = 0;
  let mut bytes_read: usize = 0;
  let mut truncation_reason = None;
  while row_count < options.row_limit {
    let Some(row) = stream.try_next().await.map_err(QueryError::from)? else {
      break;
    };
    let row = decode_sqlite_row(&row, &columns)?;
    if !admit_row_bytes(&row, options.byte_limit, &mut bytes_read)? {
      truncation_reason = Some(QueryTruncationReason::ByteLimit);
      break;
    }
    rows.push(row);
    row_count += 1;
    flush_full_batch(&mut rows, options.batch_size, &mut batch_count, row_count, sink)?;
  }
  if truncation_reason.is_none()
    && row_count == options.row_limit
    && stream.try_next().await.map_err(QueryError::from)?.is_some()
  {
    truncation_reason = Some(QueryTruncationReason::RowLimit);
  }
  flush_remaining_batch(&mut rows, &mut batch_count, row_count, sink)?;
  Ok(QueryExecutionSummary::Rows {
    columns,
    column_metadata,
    row_count,
    batch_count,
    truncated: truncation_reason.is_some(),
    truncation_reason,
    row_limit: options.row_limit,
    byte_limit: options.byte_limit,
    bytes_read,
    omitted_result_sets: 0,
    extra_result_sets: Vec::new(),
  })
}

async fn execute_mysql(
  pool: &Pool<MySql>,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut connection = pool.acquire().await.map_err(QueryError::from)?;
  execute_mysql_connection_with_limits(&mut connection, sql, row_limit, byte_limit).await
}

async fn execute_mysql_connection(
  connection: &mut MySqlConnection,
  sql: &str,
  row_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  execute_mysql_connection_with_limits(connection, sql, row_limit, DEFAULT_QUERY_BYTE_LIMIT).await
}

async fn execute_mysql_connection_with_limits(
  connection: &mut MySqlConnection,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut rows = Vec::new();
  let summary = execute_mysql_connection_streaming(
    connection,
    sql,
    &[],
    StreamOptions::limited(row_limit, byte_limit, row_limit.max(1)),
    &mut |batch| {
      rows.extend(batch.rows);
      Ok(())
    },
  )
  .await?;
  summary_with_rows(summary, rows)
}

async fn describe_mysql_columns(
  connection: &mut MySqlConnection,
  sql: &str,
) -> Result<Vec<QueryColumnMetadata>, QueryError> {
  let description = (&mut *connection).describe(sql).await.map_err(QueryError::from)?;
  Ok(mysql_column_metadata(&description))
}

fn mysql_column_metadata(description: &sqlx::Describe<MySql>) -> Vec<QueryColumnMetadata> {
  let names = number_duplicate_columns(description.columns().iter().map(|column| column.name()));
  description
    .columns()
    .iter()
    .zip(names)
    .enumerate()
    .map(|(ordinal, (column, name))| QueryColumnMetadata {
      name,
      ordinal,
      database_type: column.type_info().name().to_string(),
      logical_type: mysql_logical_type(column.type_info().name()).to_string(),
      nullable: description.nullable(ordinal),
    })
    .collect()
}

/// MySQL 8.4 上 prepare 报 1295 的那几种，不先 prepare 直接走文本协议。
///
/// 先试一下再退回不行：`SHOW WARNINGS` 说的是上一条语句，那次失败的 prepare 本身就成了「上一条」，
/// 用户看到的是 1295 而不是自己那条的警告。`USE` 也报 1295，但它在前面就被拒了
fn refused_by_prepared_protocol(sql: &str) -> bool {
  let (first, second) = crate::services::transaction_state::leading_keywords(sql);
  matches!(first.as_str(), "LOCK" | "UNLOCK" | "HELP" | "HANDLER" | "XA")
    || matches!(
      (first.as_str(), second.as_str()),
      ("CHECK", "TABLE") | ("SHOW", "WARNINGS" | "ERRORS" | "COUNT")
    )
}

/// 上面那张表没列到、prepare 时报了 1295 的，同样退回文本协议
fn unsupported_by_prepared_protocol(error: &sqlx::Error) -> bool {
  const ER_UNSUPPORTED_PS: u16 = 1295;
  error
    .as_database_error()
    .and_then(|database| database.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
    .is_some_and(|mysql| mysql.number() == ER_UNSUPPORTED_PS)
}

async fn execute_mysql_connection_streaming(
  connection: &mut MySqlConnection,
  sql: &str,
  params: &[JsonValue],
  options: StreamOptions,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<QueryExecutionSummary, QueryError> {
  refuse_use_statement(sql)?;
  if is_transaction_control_statement(sql) {
    refuse_non_query(options.non_query)?;
    let result = (&mut *connection).execute(sql).await.map_err(QueryError::from)?;
    return Ok(QueryExecutionSummary::Affected { rows_affected: result.rows_affected() });
  }

  // 参数只能经预处理协议绑；文本协议那条退路走不了
  if refused_by_prepared_protocol(sql) {
    if !params.is_empty() {
      return Err(QueryError::message(query_parameters::PARAMETERS_NEED_PREPARED));
    }
    refuse_non_query(options.non_query)?;
    return stream_mysql_undescribed(connection, sql, &[], options, sink, false).await;
  }
  let column_metadata = match (&mut *connection).describe(sql).await {
    Err(error) if unsupported_by_prepared_protocol(&error) => {
      if !params.is_empty() {
        return Err(QueryError::message(query_parameters::PARAMETERS_NEED_PREPARED));
      }
      refuse_non_query(options.non_query)?;
      return stream_mysql_undescribed(connection, sql, &[], options, sink, false).await;
    }
    described => mysql_column_metadata(&described.map_err(QueryError::from)?),
  };
  let columns = column_metadata.iter().map(|column| column.name.clone()).collect::<Vec<_>>();

  if columns.is_empty() && options.explain_plan == ExplainPlan::Off {
    refuse_non_query(options.non_query)?;
    return stream_mysql_undescribed(connection, sql, params, options, sink, true).await;
  }

  let mut stream =
    (&mut *connection).fetch(query_parameters::bind_mysql(sqlx::query(sql), params)?);
  let mut rows = Vec::with_capacity(options.batch_size);
  let mut row_count = 0;
  let mut batch_count = 0;
  let mut bytes_read: usize = 0;
  let mut truncation_reason = None;
  while row_count < options.row_limit {
    let Some(row) = stream.try_next().await.map_err(QueryError::from)? else {
      break;
    };
    let row = decode_mysql_row(&row, &columns)?;
    if !admit_row_bytes(&row, options.byte_limit, &mut bytes_read)? {
      truncation_reason = Some(QueryTruncationReason::ByteLimit);
      break;
    }
    rows.push(row);
    row_count += 1;
    flush_full_batch(&mut rows, options.batch_size, &mut batch_count, row_count, sink)?;
  }
  if truncation_reason.is_none()
    && row_count == options.row_limit
    && stream.try_next().await.map_err(QueryError::from)?.is_some()
  {
    truncation_reason = Some(QueryTruncationReason::RowLimit);
  }
  flush_remaining_batch(&mut rows, &mut batch_count, row_count, sink)?;
  Ok(QueryExecutionSummary::Rows {
    columns,
    column_metadata,
    row_count,
    batch_count,
    truncated: truncation_reason.is_some(),
    truncation_reason,
    row_limit: options.row_limit,
    byte_limit: options.byte_limit,
    bytes_read,
    omitted_result_sets: 0,
    extra_result_sets: Vec::new(),
  })
}

/// describe 说没有列的语句。多数是写语句，但 `CALL` 一个查询过程也是这样：过程里的 SELECT 要执行了才有
/// 结果集。第一个结果集给用户看（与 SQL Server 一样），之后的读掉不留；一个结果集都没有就是影响行数。
///
/// 列名取自第一行：结果集是空的时候拿不到列（sqlx 不给没有行的列定义），只能报影响行数。
///
/// `prepared` 为假时走文本协议：预处理协议不收的语句（见 [`unsupported_by_prepared_protocol`]）
async fn stream_mysql_undescribed(
  connection: &mut MySqlConnection,
  sql: &str,
  params: &[JsonValue],
  options: StreamOptions,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
  prepared: bool,
) -> Result<QueryExecutionSummary, QueryError> {
  let mut stream = if prepared {
    (&mut *connection).fetch_many(query_parameters::bind_mysql(sqlx::query(sql), params)?)
  } else {
    (&mut *connection).fetch_many(sqlx::raw_sql(sql))
  };
  let mut first: Option<Vec<QueryColumnMetadata>> = None;
  // 第一个结果集读完（或到了上限）之后，后面的只是读掉
  let mut first_done = false;
  // 第一个结果集的列名（重名的编了号），行按它做键
  let mut names: Vec<String> = Vec::new();
  // 第一个结果集之后的每个结果集各以一个 OK 收尾，最后还有 CALL 自己的那一个。有行的收进
  // `extra`；没有行的 sqlx 给不出列，只能计数
  let mut extra = ExtraResultSets::new(&options);
  let mut in_extra_set = false;
  let mut empty_after_first: usize = 0;
  let mut extra_names: Vec<String> = Vec::new();
  let mut rows_affected = 0;
  let mut rows = Vec::with_capacity(options.batch_size);
  let mut row_count = 0;
  let mut batch_count = 0;
  let mut bytes_read: usize = 0;
  let mut truncation_reason = None;
  while let Some(item) = stream.try_next().await.map_err(QueryError::from)? {
    let row = match item {
      sqlx::Either::Left(result) => {
        rows_affected += result.rows_affected();
        if first_done && !std::mem::take(&mut in_extra_set) {
          empty_after_first += 1;
        }
        first_done |= first.is_some();
        continue;
      }
      sqlx::Either::Right(row) => row,
    };
    if first_done {
      if !in_extra_set {
        extra_names = number_duplicate_columns(row.columns().iter().map(|column| column.name()));
        extra.start(mysql_row_column_metadata(&row, &extra_names));
        in_extra_set = true;
      }
      extra.push(decode_mysql_row(&row, &extra_names)?)?;
      continue;
    }
    if truncation_reason.is_some() {
      continue;
    }
    if first.is_none() {
      names = number_duplicate_columns(row.columns().iter().map(|column| column.name()));
      first = Some(mysql_row_column_metadata(&row, &names));
    }
    if row_count >= options.row_limit {
      truncation_reason = Some(QueryTruncationReason::RowLimit);
      continue;
    }
    let row = decode_mysql_row(&row, &names)?;
    if !admit_row_bytes(&row, options.byte_limit, &mut bytes_read)? {
      truncation_reason = Some(QueryTruncationReason::ByteLimit);
      continue;
    }
    rows.push(row);
    row_count += 1;
    flush_full_batch(&mut rows, options.batch_size, &mut batch_count, row_count, sink)?;
  }
  let Some(column_metadata) = first else {
    return Ok(QueryExecutionSummary::Affected { rows_affected });
  };
  flush_remaining_batch(&mut rows, &mut batch_count, row_count, sink)?;
  let (extra_result_sets, omitted) = extra.finish();
  Ok(QueryExecutionSummary::Rows {
    columns: column_metadata.iter().map(|column| column.name.clone()).collect(),
    column_metadata,
    row_count,
    batch_count,
    truncated: truncation_reason.is_some(),
    truncation_reason,
    row_limit: options.row_limit,
    byte_limit: options.byte_limit,
    bytes_read,
    omitted_result_sets: omitted + empty_after_first.saturating_sub(1),
    extra_result_sets,
  })
}

fn mysql_row_column_metadata(
  row: &sqlx::mysql::MySqlRow,
  names: &[String],
) -> Vec<QueryColumnMetadata> {
  row
    .columns()
    .iter()
    .zip(names.iter().cloned())
    .enumerate()
    .map(|(ordinal, (column, name))| QueryColumnMetadata {
      name,
      ordinal,
      database_type: column.type_info().name().to_string(),
      logical_type: mysql_logical_type(column.type_info().name()).to_string(),
      nullable: None,
    })
    .collect()
}

async fn execute_postgres(
  pool: &Pool<Postgres>,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut connection = pool.acquire().await.map_err(QueryError::from)?;
  execute_postgres_connection_with_limits(&mut connection, sql, row_limit, byte_limit).await
}

async fn execute_postgres_connection(
  connection: &mut PgConnection,
  sql: &str,
  row_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  execute_postgres_connection_with_limits(connection, sql, row_limit, DEFAULT_QUERY_BYTE_LIMIT)
    .await
}

async fn execute_postgres_connection_with_limits(
  connection: &mut PgConnection,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<QueryExecutionResult, QueryError> {
  let mut rows = Vec::new();
  let summary = execute_postgres_connection_streaming(
    connection,
    sql,
    &[],
    StreamOptions::limited(row_limit, byte_limit, row_limit.max(1)),
    &mut |batch| {
      rows.extend(batch.rows);
      Ok(())
    },
  )
  .await?;
  summary_with_rows(summary, rows)
}

/// 列的元数据，外加这些列是不是都能按二进制解（`pg_decodes_binary`）
///
/// 先清掉这条连接缓存的预备语句。sqlx 按 SQL 原文缓存，`describe` 命中缓存时连服务端都不问；
/// 而表结构变了（改列名、改类型，不论哪条连接改的），PostgreSQL 对结果形状变了的旧语句
/// 一律报「cached plan must not change result type」，同一句 `SELECT *` 在这条连接上就再也
/// 跑不通。事后重试不行：在事务里这一下已经让整个事务作废。代价是缓存非空时多一次往返
/// （Close + Sync）；交互式客户端几乎不会在同一条连接上反复跑同一句，缓存本来也省不下什么。
/// 不把容量设成 0：那样 sqlx 仍给每句开一个具名语句、却不缓存也不关，服务端越攒越多
///
/// 用 `prepare` 而不是 `describe`：后者为了推断可空，另发一句目录查询和一句
/// `EXPLAIN (VERBOSE) EXECUTE sqlx_s_1`。一是慢——经 cu 那条链路每条语句多 0.78 秒（20 次平均
/// describe 1.14 秒、prepare 0.36 秒）；二是那句 EXPLAIN 在 SQL 文本里点名预备语句，事务池
/// （pgbouncer、Supabase、Neon 的连接池端口）只改得了协议里的名字，每条查询都报 26000。
/// 代价是 PostgreSQL 的结果表头不再写 NULL / NOT NULL，与说不出可空的别家一样写「未知」
async fn describe_postgres_columns(
  connection: &mut PgConnection,
  sql: &str,
) -> Result<(Vec<QueryColumnMetadata>, bool, Vec<sqlx::postgres::PgTypeInfo>), QueryError> {
  connection.clear_cached_statements().await.map_err(QueryError::from)?;
  let statement = (&mut *connection).prepare(sql).await.map_err(QueryError::from)?;
  let parameter_types = match statement.parameters() {
    Some(sqlx::Either::Left(types)) => types.to_vec(),
    _ => Vec::new(),
  };
  let binary = statement.columns().iter().all(|column| pg_decodes_binary(column.type_info()));
  let names = number_duplicate_columns(statement.columns().iter().map(|column| column.name()));
  Ok((
    statement
      .columns()
      .iter()
      .zip(names)
      .enumerate()
      .map(|(ordinal, (column, name))| QueryColumnMetadata {
        name,
        ordinal,
        database_type: column.type_info().name().to_string(),
        logical_type: postgres_logical_type(column.type_info().name()).to_string(),
        nullable: None,
      })
      .collect(),
    binary,
    parameter_types,
  ))
}

async fn execute_postgres_connection_streaming(
  connection: &mut PgConnection,
  sql: &str,
  params: &[JsonValue],
  options: StreamOptions,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<QueryExecutionSummary, QueryError> {
  if is_transaction_control_statement(sql) {
    refuse_non_query(options.non_query)?;
    let result = (&mut *connection).execute(sql).await.map_err(QueryError::from)?;
    return Ok(QueryExecutionSummary::Affected { rows_affected: result.rows_affected() });
  }

  let (column_metadata, binary, parameter_types) =
    describe_postgres_columns(connection, sql).await?;
  let columns = column_metadata.iter().map(|column| column.name.clone()).collect::<Vec<_>>();
  let bound = |sql| query_parameters::bind_postgres(sqlx::query(sql), params, &parameter_types);
  if !params.is_empty() {
    // sqlx 按 SQL 文本缓存预备语句：留着 describe 那一份，绑的值会按服务端推断的类型读，
    // 没写时区的 timestamptz 就被当成 UTC。清掉，让它按绑定的类型重新准备
    connection.clear_cached_statements().await.map_err(QueryError::from)?;
  }

  if columns.is_empty() && options.explain_plan == ExplainPlan::Off {
    refuse_non_query(options.non_query)?;
    let result = if params.is_empty() {
      (&mut *connection).execute(sql).await
    } else {
      (&mut *connection).execute(bound(sql)?).await
    }
    .map_err(QueryError::from)?;
    return Ok(QueryExecutionSummary::Affected { rows_affected: result.rows_affected() });
  }
  // 带参数只能走扩展协议，结果按二进制读；只有文本格式读得了的列就读不了
  if !binary && !params.is_empty() {
    return Err(QueryError::message(query_parameters::PARAMETERS_TEXT_RESULTS));
  }

  // 不带参数的 `&str` 走简单查询协议，结果是文本格式。能 describe 的只会是一条语句，
  // 所以这里不会顺带执行别的
  let mut stream =
    if binary { (&mut *connection).fetch(bound(sql)?) } else { (&mut *connection).fetch(sql) };
  let mut rows = Vec::with_capacity(options.batch_size);
  let mut row_count = 0;
  let mut batch_count = 0;
  let mut bytes_read: usize = 0;
  let mut truncation_reason = None;
  while row_count < options.row_limit {
    let Some(row) = stream.try_next().await.map_err(QueryError::from)? else {
      break;
    };
    let row = decode_postgres_row(&row, &columns)?;
    if !admit_row_bytes(&row, options.byte_limit, &mut bytes_read)? {
      truncation_reason = Some(QueryTruncationReason::ByteLimit);
      break;
    }
    rows.push(row);
    row_count += 1;
    flush_full_batch(&mut rows, options.batch_size, &mut batch_count, row_count, sink)?;
  }
  if truncation_reason.is_none()
    && row_count == options.row_limit
    && stream.try_next().await.map_err(QueryError::from)?.is_some()
  {
    truncation_reason = Some(QueryTruncationReason::RowLimit);
  }
  flush_remaining_batch(&mut rows, &mut batch_count, row_count, sink)?;
  Ok(QueryExecutionSummary::Rows {
    columns,
    column_metadata,
    row_count,
    batch_count,
    truncated: truncation_reason.is_some(),
    truncation_reason,
    row_limit: options.row_limit,
    byte_limit: options.byte_limit,
    bytes_read,
    omitted_result_sets: 0,
    extra_result_sets: Vec::new(),
  })
}

pub(crate) fn flush_full_batch(
  rows: &mut Vec<QueryRow>,
  batch_size: usize,
  batch_count: &mut usize,
  row_count: usize,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<(), QueryError> {
  if rows.len() < batch_size.max(1) {
    return Ok(());
  }
  send_batch(rows, batch_count, row_count, sink)
}

pub(crate) fn flush_remaining_batch(
  rows: &mut Vec<QueryRow>,
  batch_count: &mut usize,
  row_count: usize,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<(), QueryError> {
  if rows.is_empty() {
    return Ok(());
  }
  send_batch(rows, batch_count, row_count, sink)
}

fn send_batch(
  rows: &mut Vec<QueryRow>,
  batch_count: &mut usize,
  row_count: usize,
  sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
) -> Result<(), QueryError> {
  let batch_rows = std::mem::take(rows);
  let offset = row_count - batch_rows.len();
  sink(QueryResultBatch { index: *batch_count, offset, rows: batch_rows })?;
  *batch_count += 1;
  Ok(())
}

/// 把这一行计入字节预算；超出预算时返回 false，调用方据此停止读取。
///
/// `usize::MAX` 表示不设限（导出路径）。这时不能只是「比较结果恒为假」就算了——
/// `serialized_row_size` 会把整行再序列化一遍，而结果会被直接丢掉。一次全表
/// 导出就是白做几百万次。
pub(crate) fn admit_row_bytes(
  row: &QueryRow,
  byte_limit: usize,
  bytes_read: &mut usize,
) -> Result<bool, QueryError> {
  if byte_limit == usize::MAX {
    return Ok(true);
  }
  let row_bytes = serialized_row_size(row)?;
  if bytes_read.saturating_add(row_bytes) > byte_limit {
    return Ok(false);
  }
  *bytes_read += row_bytes;
  Ok(true)
}

/// 不返回结果集时给出的话。导出路径有两处会说这句（先探列、再流式读），
/// 同一件事说两种话会让人以为是两个不同的问题。
pub const NON_QUERY_MESSAGE: &str = "DATAOMNI_NON_QUERY";

/// MySQL 方言下拒绝 `USE`，见 `refuse_use_statement`。
pub const USE_STATEMENT_REFUSED: &str = "DATAOMNI_USE_STATEMENT_REFUSED";

/// MySQL 方言下不让 `USE` 跑到服务端。
///
/// 编辑器与对象树、补全、ER 图共用一个池子，目录查询按 `DATABASE()` 取当前库。
/// 一条被 `USE` 挪走的连接还回池子，下一次拿到它的目录查询就列出另一个库的
/// 表，而界面上的库名没变。MySQL 的预处理协议本来就拒绝 `USE`（1295），
/// MariaDB 的不拒，所以这一道由我们自己挡，两家说同一句话。
pub(crate) fn refuse_use_statement(sql: &str) -> Result<(), QueryError> {
  match crate::services::transaction_state::leading_keywords(sql).0.as_str() {
    "USE" => Err(QueryError::message(USE_STATEMENT_REFUSED)),
    _ => Ok(()),
  }
}

/// 导出路径遇到不返回结果集的语句时提前退出，不让它执行。
pub(crate) fn refuse_non_query(handling: NonQueryHandling) -> Result<(), QueryError> {
  match handling {
    NonQueryHandling::Execute => Ok(()),
    NonQueryHandling::Refuse => Err(QueryError::message(NON_QUERY_MESSAGE)),
  }
}

/// 一行序列化之后多少字节。只数不存：原来 `to_vec` 把整行再写一份出来只为取长度，
/// 一格大 BLOB 就要多占它十六进制的那么多内存
fn serialized_row_size(row: &QueryRow) -> Result<usize, QueryError> {
  struct ByteCounter(usize);
  impl std::io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      self.0 += buf.len();
      Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
      Ok(())
    }
  }
  let mut counter = ByteCounter(0);
  serde_json::to_writer(&mut counter, row).map_err(display_error)?;
  Ok(counter.0)
}

pub(crate) use crate::services::sqlx_decode::*;

fn summary_with_rows(
  summary: QueryExecutionSummary,
  rows: Vec<QueryRow>,
) -> Result<QueryExecutionResult, QueryError> {
  match summary {
    QueryExecutionSummary::Rows {
      columns,
      column_metadata,
      truncated,
      truncation_reason,
      row_limit,
      byte_limit,
      bytes_read,
      omitted_result_sets,
      extra_result_sets,
      ..
    } => Ok(QueryExecutionResult::Rows {
      columns,
      column_metadata,
      rows,
      truncated,
      truncation_reason,
      row_limit,
      byte_limit,
      bytes_read,
      omitted_result_sets,
      extra_result_sets,
    }),
    QueryExecutionSummary::Affected { rows_affected } => {
      Ok(QueryExecutionResult::Affected { rows_affected })
    }
  }
}

fn is_transaction_control_statement(sql: &str) -> bool {
  let normalized = sql.trim().trim_end_matches(';').trim().to_ascii_uppercase();
  [
    "BEGIN",
    "START TRANSACTION",
    "COMMIT",
    "ROLLBACK",
    "SAVEPOINT",
    "RELEASE SAVEPOINT",
    "SET TRANSACTION",
  ]
  .iter()
  .any(|keyword| normalized == *keyword || normalized.starts_with(&format!("{keyword} ")))
}

#[cfg(test)]
mod tests {
  use super::*;
  use sqlx::mysql::types::MySqlTime;
  use time::{Date, PrimitiveDateTime, Time};

  #[test]
  fn hex_encoding_matches_the_per_byte_format_it_replaced() {
    let bytes: Vec<u8> = (0..=255).collect();
    let expected: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(encode_hex(&bytes), expected);
    assert_eq!(encode_hex(&[]), "");
    assert_eq!(
      binary_json(&[0xde, 0xad]),
      serde_json::json!({ "type": "binary", "value": "dead" })
    );
  }

  #[test]
  fn serialized_row_size_counts_exactly_what_serde_writes() {
    let mut row = QueryRow::new();
    row.insert("id".to_string(), JsonValue::from(1));
    row.insert("blob".to_string(), binary_json(&vec![0xab; 4096]));
    row.insert("note".to_string(), JsonValue::from("引号\"与换行\n"));
    let expected = serde_json::to_vec(&row).map(|bytes| bytes.len()).ok();
    assert_eq!(serialized_row_size(&row).ok(), expected);
  }

  fn one_column(name: &str) -> Vec<QueryColumnMetadata> {
    vec![QueryColumnMetadata {
      name: name.to_string(),
      ordinal: 0,
      database_type: "INT".to_string(),
      logical_type: "integer".to_string(),
      nullable: None,
    }]
  }

  fn row(value: i64) -> QueryRow {
    let mut row = QueryRow::new();
    row.insert("n".to_string(), JsonValue::from(value));
    row
  }

  #[test]
  fn extra_result_sets_past_the_cap_are_only_counted() {
    let mut extra = ExtraResultSets::new(&StreamOptions::limited(10, 1 << 20, 10));
    for index in 0..MAX_EXTRA_RESULT_SETS + 3 {
      extra.start(one_column("n"));
      extra.push(row(index as i64)).expect("push");
    }
    let (sets, omitted) = extra.finish();
    assert_eq!((sets.len(), omitted), (MAX_EXTRA_RESULT_SETS, 3));
    // 没收的那几个的行也没混进最后一个收下的结果集
    assert_eq!(sets.last().map(|set| set.rows.len()), Some(1));
  }

  #[test]
  fn extra_result_sets_share_one_byte_budget() {
    let row_bytes = serialized_row_size(&row(1)).expect("size");
    let mut extra = ExtraResultSets::new(&StreamOptions::limited(10, row_bytes * 3, 10));
    extra.start(one_column("n"));
    extra.push(row(1)).expect("push");
    extra.push(row(2)).expect("push");
    extra.start(one_column("n"));
    for value in 3..6 {
      extra.push(row(value)).expect("push");
    }
    extra.start(one_column("n"));
    extra.push(row(9)).expect("push");
    let (sets, omitted) = extra.finish();
    let rows: Vec<_> = sets.iter().map(|set| set.rows.len()).collect();
    assert_eq!(rows, vec![2, 1]);
    assert_eq!(sets[1].truncation_reason, Some(QueryTruncationReason::ByteLimit));
    // 预算用完之后开始的结果集不再收，只计数
    assert_eq!(omitted, 1);
  }

  /// 期望值是 MySQL 8.4 对同一个 `TIME(6)` 值的输出去掉末尾的 0（与 DATETIME 的小数秒同一个写法）
  #[test]
  fn a_mysql_time_keeps_its_fractional_seconds_and_sign() {
    use sqlx::mysql::types::MySqlTimeSign::{Negative, Positive};
    let cases = [
      (Positive, 12, 0, 0, 500_000, "12:00:00.5"),
      (Negative, 0, 0, 1, 250_000, "-00:00:01.25"),
      (Negative, 838, 59, 59, 0, "-838:59:59"),
      (Positive, 0, 0, 0, 1, "00:00:00.000001"),
      (Positive, 0, 0, 0, 0, "00:00:00"),
    ];
    for (sign, hours, minutes, seconds, micros, expected) in cases {
      let time = MySqlTime::new(sign, hours, minutes, seconds, micros).expect("a valid TIME");
      assert_eq!(format_mysql_time(&time), expected);
    }
  }

  /// 期望值是 PostgreSQL 16 对同一个值的文本输出（默认 IntervalStyle `postgres`），逐条跑过
  #[test]
  fn an_interval_reads_the_way_postgres_prints_it() {
    let cases: [(i32, i32, i64, &str); 15] = [
      (0, 1, 7_384_000_000, "1 day 02:03:04"),
      (-3, 0, 0, "-3 mons"),
      (14, 0, 0, "1 year 2 mons"),
      (0, 0, 0, "00:00:00"),
      (0, -1, 7_200_000_000, "-1 days +02:00:00"),
      (0, 0, -7_384_500_000, "-02:03:04.5"),
      (0, 2, 0, "2 days"),
      (-12, 0, 0, "-1 years"),
      (0, 0, 360_000_000_000, "100:00:00"),
      (1, 0, 1, "1 mon 00:00:00.000001"),
      (-14, 0, 0, "-1 years -2 mons"),
      (0, 1, -1_000_000, "1 day -00:00:01"),
      (-1, 1, 0, "-1 mons +1 day"),
      (0, 0, 100_000, "00:00:00.1"),
      (12, 1, 0, "1 year 1 day"),
    ];
    for (months, days, microseconds, expected) in cases {
      let interval = sqlx::postgres::types::PgInterval { months, days, microseconds };
      assert_eq!(
        format_pg_interval(&interval),
        expected,
        "{months} mons {days} days {microseconds} µs"
      );
    }
  }
  use serde_json::json;
  use sqlx::sqlite::SqlitePoolOptions;
  use std::future::pending;

  #[test]
  fn duplicate_column_names_get_numbered() {
    assert_eq!(number_duplicate_columns(["A", "B", "A"].into_iter()), ["A", "B", "A 2"]);
    // 编号跳过已有的名字，不管那一列在前在后
    assert_eq!(number_duplicate_columns(["A", "A", "A 2"].into_iter()), ["A", "A 3", "A 2"]);
    assert_eq!(number_duplicate_columns(["A 2", "A", "A"].into_iter()), ["A 2", "A", "A 3"]);
    assert_eq!(
      number_duplicate_columns(["A 2", "A", "A", "A 2"].into_iter()),
      ["A 2", "A", "A 3", "A 2 2"]
    );
  }

  #[test]
  fn date_and_time_values_come_out_the_way_the_database_writes_them() {
    let date = Date::from_calendar_date(2026, time::Month::September, 3).expect("valid date");
    let morning = Time::from_hms(7, 4, 5).expect("valid time");
    assert_eq!(format_date(date), "2026-09-03");
    assert_eq!(format_time(morning), "07:04:05");
    assert_eq!(format_datetime(PrimitiveDateTime::new(date, morning)), "2026-09-03 07:04:05");
    // 小数秒去掉末尾的 0，但不丢前导 0
    let fractional = Time::from_hms_micro(0, 0, 0, 50_000).expect("valid time");
    assert_eq!(format_time(fractional), "00:00:00.05");
    let micro = Time::from_hms_micro(23, 59, 59, 1).expect("valid time");
    assert_eq!(format_time(micro), "23:59:59.000001");
    // SQL Server 的 time(7) / datetime2(7) 精确到 100 纳秒：按微秒写就丢了第七位，
    // 网格与导出里的值和库里的不是同一个
    let hundred_nanos = Time::from_hms_nano(8, 30, 0, 123_456_700).expect("valid time");
    assert_eq!(format_time(hundred_nanos), "08:30:00.1234567");
  }

  #[test]
  fn refuses_use_but_not_statements_that_merely_start_like_it() {
    for sql in ["USE other_db", "use `other db`;", "-- 切库\nUSE other_db", "/* x */ Use other_db"]
    {
      assert!(refuse_use_statement(sql).is_err(), "expected refusal: {sql}");
    }
    for sql in ["SELECT 'USE other_db'", "USER_DEFINED", "UPDATE users SET name = 'use'"] {
      assert!(refuse_use_statement(sql).is_ok(), "expected pass-through: {sql}");
    }
  }

  #[test]
  fn recognizes_transaction_control_statements_without_matching_prefixes() {
    for sql in [
      "BEGIN",
      "begin transaction;",
      "START TRANSACTION",
      "COMMIT WORK",
      "ROLLBACK TO SAVEPOINT before_update",
      "SAVEPOINT before_update",
      "RELEASE SAVEPOINT before_update",
      "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
    ] {
      assert!(is_transaction_control_statement(sql), "expected transaction SQL: {sql}");
    }

    assert!(!is_transaction_control_statement("SELECT 'BEGIN'"));
    assert!(!is_transaction_control_statement("BEGINNING"));
  }

  #[test]
  fn maps_driver_types_to_logical_types() {
    assert_eq!(sqlite_logical_type("INTEGER"), "integer");
    assert_eq!(sqlite_logical_type("BLOB"), "binary");
  }

  #[test]
  fn postgres_arrays_are_written_as_array_literals() {
    let items: Result<_, String> = Ok(vec![
      Some("a".to_string()),
      Some("b,c".into()),
      None,
      Some(String::new()),
      Some("null".into()),
    ]);
    assert_eq!(
      pg_array_value(items).unwrap(),
      JsonValue::String(r#"{a,"b,c",NULL,"","null"}"#.into())
    );
    let quoted: Result<_, String> = Ok(vec![Some(r#"x"y\z"#.to_string()), Some(" s".into())]);
    assert_eq!(pg_array_value(quoted).unwrap(), JsonValue::String(r#"{"x\"y\\z"," s"}"#.into()));
    let numbers: Result<_, String> = Ok(vec![Some(1), None, Some(-3)]);
    assert_eq!(pg_array_value(numbers).unwrap(), JsonValue::String("{1,NULL,-3}".into()));
    let empty: Result<Vec<Option<i64>>, String> = Ok(vec![]);
    assert_eq!(pg_array_value(empty).unwrap(), JsonValue::String("{}".into()));
  }

  #[test]
  fn mysql_bin_collated_text_reads_as_text_but_real_bytes_stay_binary() {
    // 回归：MariaDB 的 JSON 列显示成 0x7b2261223a317d
    assert_eq!(mysql_bytes_value(b"{\"a\":1}".to_vec()), JsonValue::String("{\"a\":1}".into()));
    assert_eq!(
      mysql_bytes_value("多字节 ✓\n第二行".as_bytes().to_vec()),
      JsonValue::String("多字节 ✓\n第二行".into())
    );
    // 不是合法 UTF-8
    assert_eq!(mysql_bytes_value(vec![0x00, 0xff, 0x10]), tagged_value("binary", "00ff10".into()));
    // 合法 UTF-8 但带控制字符：NUL 开头的定长 BINARY
    assert_eq!(mysql_bytes_value(vec![0x00, 0x41]), tagged_value("binary", "0041".into()));
    assert_eq!(mysql_logical_type("DECIMAL"), "decimal");
    assert_eq!(mysql_logical_type("JSON"), "json");
    assert_eq!(postgres_logical_type("TIMESTAMPTZ"), "datetime");
    assert_eq!(postgres_logical_type("UUID"), "text");
    assert_eq!(postgres_logical_type("CUSTOM"), "unknown");
  }

  #[tokio::test]
  async fn a_session_on_a_closed_pool_reports_the_connection_as_lost() {
    // 断开之后在任务面板里点「重试」备份 / 导出 / 导入，走的就是这里：
    // 界面上印的是驱动原话「attempted to acquire a connection on a closed pool」，
    // 也不会把连接标成断了
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.expect("connect");
    pool.close().await;
    let pool = DbPool::Sqlite(pool);
    let Err(error) = SessionConnection::acquire(&pool).await else {
      panic!("a closed pool handed out a connection");
    };
    assert_eq!(error.code.as_deref(), Some(crate::services::query_error::CONNECTION_LOST_CODE));
  }

  /// MySQL 的 `EXPLAIN FORMAT=JSON` 描述出 0 列却有一行：按空名字表取键，那一行整个是空的，计划解析不出来
  #[test]
  fn a_row_without_described_columns_keeps_the_driver_names() {
    assert_eq!(row_key(&[], 0, "EXPLAIN"), "EXPLAIN");
    assert_eq!(row_key(&["id".to_string(), "id 2".to_string()], 1, "id"), "id 2");
  }

  /// 回归：`SELECT 1 AS id, 'a' AS name, 2 AS id` 第一列显示的是 2——行按列名做键，后一列顶掉了前一列
  /// （rpm 0.4.103 连 PG 16 实测）。SQL Server、Oracle、ClickHouse 早就给重名的列编了号
  #[tokio::test]
  async fn columns_with_the_same_name_keep_their_own_values() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result = execute_query(&DbPool::Sqlite(pool), "SELECT 1 AS id, 'a' AS name, 2 AS id")
      .await
      .expect("execute");

    match result {
      QueryExecutionResult::Rows { columns, rows, .. } => {
        assert_eq!(columns, vec!["id", "name", "id 2"]);
        let value = |key: &str| rows[0].get(key).and_then(|cell| cell.get("value")).cloned();
        assert_eq!(value("id"), Some(JsonValue::from("1")));
        assert_eq!(value("id 2"), Some(JsonValue::from("2")));
      }
      QueryExecutionResult::Affected { .. } => panic!("expected a row result"),
    }
  }

  /// 编出来的号撞上本来就叫这个名字的列：两列都叫 `id 2`，后一列又顶掉了前一列
  #[tokio::test]
  async fn a_numbered_column_does_not_take_the_name_of_a_real_one() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result = execute_query(&DbPool::Sqlite(pool), "SELECT 1 AS id, 2 AS id, 3 AS \"id 2\"")
      .await
      .expect("execute");

    match result {
      QueryExecutionResult::Rows { columns, rows, .. } => {
        assert_eq!(columns, vec!["id", "id 3", "id 2"]);
        let value = |key: &str| rows[0].get(key).and_then(|cell| cell.get("value")).cloned();
        assert_eq!(value("id"), Some(JsonValue::from("1")));
        assert_eq!(value("id 3"), Some(JsonValue::from("2")));
        assert_eq!(value("id 2"), Some(JsonValue::from("3")));
      }
      QueryExecutionResult::Affected { .. } => panic!("expected a row result"),
    }
  }

  #[tokio::test]
  async fn uses_driver_metadata_for_empty_result_sets() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result = execute_query(&DbPool::Sqlite(pool), "SELECT 1 AS value WHERE 1 = 0")
      .await
      .expect("execute empty query");

    match result {
      QueryExecutionResult::Rows { columns, rows, truncated, row_limit, .. } => {
        assert_eq!(columns, vec!["value"]);
        assert!(rows.is_empty());
        assert!(!truncated);
        assert_eq!(row_limit, DEFAULT_QUERY_ROW_LIMIT);
      }
      QueryExecutionResult::Affected { .. } => panic!("expected a row result"),
    }
  }

  /// SQLite 不查 TEXT 是不是 UTF-8，老程序按 latin1 写进来的字节原样存着；
  /// 按严格 UTF-8 解的话这一格让整条查询报错，表整张打不开
  #[tokio::test]
  async fn sqlite_text_that_is_not_utf8_still_reads() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result =
      execute_query(&DbPool::Sqlite(pool), "SELECT CAST(x'436166e9' AS TEXT) AS name, 1 AS id")
        .await
        .expect("execute query");
    let QueryExecutionResult::Rows { rows, .. } = result else {
      panic!("expected a row result");
    };
    assert_eq!(rows[0]["name"], "Caf\u{fffd}");
    assert_eq!(rows[0]["id"]["value"], "1");
  }

  /// JSON 里没有无穷，serde_json 给 null：网格上 `1e999` 看着是 NULL
  #[tokio::test]
  async fn sqlite_infinite_reals_read_as_sqlites_own_spelling() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result =
      execute_query(&DbPool::Sqlite(pool), "SELECT 1e999 AS up, -1e999 AS down, 0.1 AS x")
        .await
        .expect("execute query");
    let QueryExecutionResult::Rows { rows, .. } = result else {
      panic!("expected a row result");
    };
    assert_eq!(rows[0]["up"], "Inf");
    assert_eq!(rows[0]["down"], "-Inf");
    assert_eq!(rows[0]["x"], 0.1);
  }

  #[tokio::test]
  async fn stops_fetching_after_the_configured_row_limit() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let result = execute_query_with_limit(
      &DbPool::Sqlite(pool),
      "SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3",
      2,
    )
    .await
    .expect("execute limited query");

    match result {
      QueryExecutionResult::Rows { rows, truncated, row_limit, .. } => {
        assert_eq!(rows.len(), 2);
        assert!(truncated);
        assert_eq!(row_limit, 2);
      }
      QueryExecutionResult::Affected { .. } => panic!("expected a row result"),
    }
  }

  /// 编辑器里的参数（rfcs/bind-parameters.md）：值照种类绑，写语句与查询都走得通
  #[tokio::test]
  async fn binds_editor_parameters_on_sqlite() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let mut connection = pool.acquire().await.expect("acquire SQLite connection");
    let mut run = async |sql: &str, params: Vec<JsonValue>| {
      let mut rows = Vec::new();
      let summary = execute_sqlite_connection_streaming(
        &mut connection,
        sql,
        &params,
        StreamOptions::limited(100, DEFAULT_QUERY_BYTE_LIMIT, 100),
        &mut |batch| {
          rows.extend(batch.rows);
          Ok(())
        },
      )
      .await
      .unwrap_or_else(|error| panic!("{sql}: {error:?}"));
      (summary, rows)
    };
    run("CREATE TABLE codes (id INTEGER PRIMARY KEY, code TEXT)", vec![]).await;
    let (inserted, _) = run(
      "INSERT INTO codes VALUES (?, ?), (?, ?)",
      vec![json!(1), json!("007"), json!(2), json!("7")],
    )
    .await;
    assert!(matches!(inserted, QueryExecutionSummary::Affected { rows_affected: 2 }));

    let (_, rows) = run("SELECT id FROM codes WHERE code = ?", vec![json!("007")]).await;
    assert_eq!(rows.len(), 1, "文本 007 不能被当成 7");
    let (_, rows) =
      run("SELECT id FROM codes WHERE id = ? OR id = ?", vec![json!(1), json!(2)]).await;
    assert_eq!(rows.len(), 2);
    let (_, rows) = run("SELECT id FROM codes ORDER BY id LIMIT ?", vec![json!(1)]).await;
    assert_eq!(rows.len(), 1);
    let (_, rows) = run("SELECT id FROM codes ORDER BY id LIMIT ?", vec![json!("1")]).await;
    assert_eq!(rows.len(), 1, "LIMIT 绑文本也收（SQL Server 的 TOP 不收，见它的真库用例）");
    let (_, rows) = run("SELECT ? IS NULL AS missing", vec![JsonValue::Null]).await;
    assert_eq!(rows[0]["missing"]["value"], json!("1"));
  }

  #[tokio::test]
  async fn streams_ordered_result_batches() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let mut connection = pool.acquire().await.expect("acquire SQLite connection");
    let mut batches = Vec::new();
    let summary = execute_sqlite_connection_streaming(
      &mut connection,
      "SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5",
      &[],
      StreamOptions::limited(5, DEFAULT_QUERY_BYTE_LIMIT, 2),
      &mut |batch| {
        batches.push(batch);
        Ok(())
      },
    )
    .await
    .expect("stream rows");

    match summary {
      QueryExecutionSummary::Rows { row_count, batch_count, truncated, .. } => {
        assert_eq!(row_count, 5);
        assert_eq!(batch_count, 3);
        assert!(!truncated);
      }
      QueryExecutionSummary::Affected { .. } => panic!("expected rows"),
    }
    assert_eq!(batches.iter().map(|batch| batch.offset).collect::<Vec<_>>(), vec![0, 2, 4]);
    assert_eq!(batches.iter().map(|batch| batch.rows.len()).collect::<Vec<_>>(), vec![2, 2, 1]);
  }

  #[tokio::test]
  async fn stops_streaming_before_exceeding_the_byte_budget() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    let mut connection = pool.acquire().await.expect("acquire SQLite connection");
    let mut batches = Vec::new();
    let summary = execute_sqlite_connection_streaming(
      &mut connection,
      "SELECT 'a value larger than the budget' AS value",
      &[],
      StreamOptions::limited(100, 1, 10),
      &mut |batch| {
        batches.push(batch);
        Ok(())
      },
    )
    .await
    .expect("enforce byte budget");

    match summary {
      QueryExecutionSummary::Rows {
        row_count, truncated, truncation_reason, bytes_read, ..
      } => {
        assert_eq!(row_count, 0);
        assert!(truncated);
        assert_eq!(truncation_reason, Some(QueryTruncationReason::ByteLimit));
        assert_eq!(bytes_read, 0);
      }
      QueryExecutionSummary::Affected { .. } => panic!("expected rows"),
    }
    assert!(batches.is_empty());
  }

  #[tokio::test]
  async fn distinguishes_affected_rows_from_returning_rows() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("connect to SQLite");
    sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
      .execute(&pool)
      .await
      .expect("create table");
    let pool = DbPool::Sqlite(pool);

    let affected =
      execute_query(&pool, "INSERT INTO items (name) VALUES ('first')").await.expect("insert row");
    assert!(matches!(affected, QueryExecutionResult::Affected { rows_affected: 1 }));

    let returned =
      execute_query(&pool, "INSERT INTO items (name) VALUES ('second') RETURNING id, name")
        .await
        .expect("insert returning row");
    match returned {
      QueryExecutionResult::Rows { columns, rows, .. } => {
        assert_eq!(columns, vec!["id", "name"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "second");
      }
      QueryExecutionResult::Affected { .. } => panic!("expected returned rows"),
    }
  }

  #[tokio::test]
  async fn reports_timeout_with_a_stable_error_code() {
    let error = with_timeout(
      async {
        pending::<()>().await;
        Ok::<(), QueryError>(())
      },
      Duration::from_millis(1),
    )
    .await;

    let error = error.expect_err("pending query should time out");
    // 码单独成一个字段，不再拼在消息前缀里：消息是要翻译的，
    // 按前缀匹配等于把「这是超时」的判断绑在某一种语言上
    assert_eq!(error.code.as_deref(), Some(QUERY_TIMEOUT_CODE));
    assert_eq!(error.message, format!("{QUERY_TIMEOUT}: 1"));
  }

  /// 跨层契约（H18）：`QueryColumnMetadata` 序列化结果必须与 `fixtures/contract-conformance.json` 一致
  #[test]
  fn query_column_metadata_matches_shared_contract_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../fixtures/contract-conformance.json"
    )))
    .expect("valid contract fixture json");
    let sample = QueryColumnMetadata {
      name: "total_amount".to_string(),
      ordinal: 2,
      database_type: "NUMERIC".to_string(),
      logical_type: "decimal".to_string(),
      nullable: Some(false),
    };
    assert_eq!(serde_json::to_value(&sample).expect("serialize"), fixture["query_column_metadata"]);
  }
}
