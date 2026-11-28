//! SQLite / MySQL / PostgreSQL 的 `sqlx` 行与列类型解码及格式化（H17 / P2-2 从 `query_executor.rs` 拆出）。

use crate::services::query_error::QueryError;
use crate::services::query_executor::{COLUMN_DECODE_FAILED, UNSUPPORTED_COLUMN_TYPE};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Map, Value as JsonValue};
use sqlx::{
  mysql::{types::MySqlTime, MySqlRow, MySqlValueRef},
  postgres::{PgRow, PgValueRef},
  sqlite::{SqliteRow, SqliteValueRef},
  Column, Row, TypeInfo, Value, ValueRef,
};
use time::{Date, PrimitiveDateTime, Time};

pub(crate) fn sqlite_logical_type(database_type: &str) -> &'static str {
  match database_type {
    "BOOLEAN" => "boolean",
    "INTEGER" => "integer",
    "REAL" | "NUMERIC" => "decimal",
    "BLOB" => "binary",
    "DATE" => "date",
    "TIME" => "time",
    "DATETIME" => "datetime",
    "TEXT" => "text",
    _ => "unknown",
  }
}

pub(crate) fn mysql_logical_type(database_type: &str) -> &'static str {
  match database_type {
    "BOOLEAN" => "boolean",
    "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "BIGINT" | "TINYINT UNSIGNED"
    | "SMALLINT UNSIGNED" | "INT UNSIGNED" | "MEDIUMINT UNSIGNED" | "BIGINT UNSIGNED" | "YEAR" => {
      "integer"
    }
    "DECIMAL" | "FLOAT" | "DOUBLE" => "decimal",
    "TINYBLOB" | "MEDIUMBLOB" | "BLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => "binary",
    "DATE" => "date",
    "TIME" => "time",
    "DATETIME" | "TIMESTAMP" => "datetime",
    "JSON" => "json",
    "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => "text",
    "BIT" => "integer",
    "GEOMETRY" => "binary",
    _ => "unknown",
  }
}

pub(crate) fn postgres_logical_type(database_type: &str) -> &'static str {
  match database_type {
    "BOOL" => "boolean",
    "INT2" | "INT4" | "INT8" => "integer",
    "NUMERIC" | "FLOAT4" | "FLOAT8" => "decimal",
    "BYTEA" => "binary",
    "INTERVAL" => "time",
    "TEXT[]" | "VARCHAR[]" | "NAME[]" | "INT2[]" | "INT4[]" | "INT8[]" => "unknown",
    "DATE" => "date",
    "TIME" | "TIMETZ" => "time",
    "TIMESTAMP" | "TIMESTAMPTZ" => "datetime",
    "JSON" | "JSONB" => "json",
    "CHAR" | "VARCHAR" | "TEXT" | "NAME" | "UUID" => "text",
    _ => "unknown",
  }
}

/// 行里这一列的键：描述出来的列名（重名的编了号）。描述报 0 列的语句（MySQL 的
/// `EXPLAIN FORMAT=JSON`）照样有行，那时只有驱动给的原名可用
pub(crate) fn row_key(names: &[String], index: usize, original: &str) -> String {
  names.get(index).cloned().unwrap_or_else(|| original.to_string())
}

pub(crate) fn decode_sqlite_row(
  row: &SqliteRow,
  names: &[String],
) -> Result<Map<String, JsonValue>, QueryError> {
  let mut values = Map::new();
  for (index, column) in row.columns().iter().enumerate() {
    let value = row.try_get_raw(index).map_err(QueryError::from)?;
    let decoded = decode_sqlite(value).map_err(|error| {
      format!("{COLUMN_DECODE_FAILED}: {} · {} · {error}", column.name(), column.type_info().name())
    })?;
    values.insert(row_key(names, index, column.name()), decoded);
  }
  Ok(values)
}

pub(crate) fn decode_mysql_row(
  row: &MySqlRow,
  names: &[String],
) -> Result<Map<String, JsonValue>, QueryError> {
  let mut values = Map::new();
  for (index, column) in row.columns().iter().enumerate() {
    let value = row.try_get_raw(index).map_err(QueryError::from)?;
    let decoded = decode_mysql(value).map_err(|error| {
      format!("{COLUMN_DECODE_FAILED}: {} · {} · {error}", column.name(), column.type_info().name())
    })?;
    values.insert(row_key(names, index, column.name()), decoded);
  }
  Ok(values)
}

pub(crate) fn decode_postgres_row(
  row: &PgRow,
  names: &[String],
) -> Result<Map<String, JsonValue>, QueryError> {
  let mut values = Map::new();
  for (index, column) in row.columns().iter().enumerate() {
    let value = row.try_get_raw(index).map_err(QueryError::from)?;
    let decoded = decode_postgres(value).map_err(|error| {
      format!("{COLUMN_DECODE_FAILED}: {} · {} · {error}", column.name(), column.type_info().name())
    })?;
    values.insert(row_key(names, index, column.name()), decoded);
  }
  Ok(values)
}

pub(crate) fn decode_sqlite(value: SqliteValueRef<'_>) -> Result<JsonValue, QueryError> {
  if value.is_null() {
    return Ok(JsonValue::Null);
  }

  match value.type_info().name() {
    // SQLite 不查 TEXT 是不是 UTF-8；解不出就把非法的字节换成 U+FFFD，
    // 改这一格时守卫拿替换后的值比不上，整批退回，不会写坏
    "TEXT" => match ValueRef::to_owned(&value).try_decode::<String>() {
      Ok(text) => Ok(JsonValue::String(text)),
      Err(_) => ValueRef::to_owned(&value)
        .try_decode_unchecked::<Vec<u8>>()
        .map(|bytes| JsonValue::String(String::from_utf8_lossy(&bytes).into_owned()))
        .map_err(display_error),
    },
    "INTEGER" | "NUMERIC" => {
      tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<i64>())
    }
    "REAL" => float_value(ValueRef::to_owned(&value).try_decode::<f64>(), "Inf"),
    "BOOLEAN" => json_value(ValueRef::to_owned(&value).try_decode::<bool>()),
    "DATE" => {
      tagged_formatted_value("date", ValueRef::to_owned(&value).try_decode::<Date>(), format_date)
    }
    "TIME" => {
      tagged_formatted_value("time", ValueRef::to_owned(&value).try_decode::<Time>(), format_time)
    }
    "DATETIME" => tagged_formatted_value(
      "datetime",
      ValueRef::to_owned(&value).try_decode::<PrimitiveDateTime>(),
      format_datetime,
    ),
    "BLOB" => tagged_binary_value(ValueRef::to_owned(&value).try_decode::<Vec<u8>>()),
    "NULL" => Ok(JsonValue::Null),
    type_name => Err(QueryError::message(format!("{UNSUPPORTED_COLUMN_TYPE}: {type_name}"))),
  }
}

pub(crate) fn decode_mysql(value: MySqlValueRef<'_>) -> Result<JsonValue, QueryError> {
  // sqlx 的 `is_null` 把零日期也算作 NULL，日期类的列得先于它认
  let type_info = value.type_info();
  let type_name = type_info.name();
  if matches!(type_name, "DATE" | "DATETIME" | "TIMESTAMP") {
    return mysql_date_time(&value, type_name);
  }
  if value.is_null() {
    return Ok(JsonValue::Null);
  }
  match type_name {
    "JSON" => tagged_json_value(ValueRef::to_owned(&value).try_decode::<JsonValue>()),
    "DECIMAL" => {
      let decimal = ValueRef::to_owned(&value)
        .try_decode::<sqlx::types::BigDecimal>()
        .map_err(QueryError::from)?;
      Ok(tagged_value("decimal", decimal_text(&decimal)))
    }
    "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "INT UNSIGNED" | "MEDIUMINT UNSIGNED"
    | "BIGINT UNSIGNED" | "YEAR" => {
      tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<u64>())
    }
    "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
      json_value(ValueRef::to_owned(&value).try_decode::<String>())
    }
    // BIT(n) 按无符号整数取值，保持精确；BIT(64) 仍在 u64 范围内
    "BIT" => tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<u64>()),
    "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "BIGINT" => {
      tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<i64>())
    }
    // MySQL 存不了 NaN 与无穷
    "FLOAT" => float_value(ValueRef::to_owned(&value).try_decode::<f32>().map(widen_f32), "Inf"),
    "DOUBLE" => float_value(ValueRef::to_owned(&value).try_decode::<f64>(), "Inf"),
    // sqlx 把显示宽度为 1 的 TINYINT 都报成 BOOLEAN，而 `tinyint(1)` 常存状态码：
    // 按布尔解 2 就成了 true。照存的数取，与 mysql 客户端相同；UNSIGNED 的 i64 不认，改取 u64
    "BOOLEAN" => match ValueRef::to_owned(&value).try_decode::<i64>() {
      Ok(number) => Ok(tagged_value("bigint", number.to_string())),
      Err(_) => tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<u64>()),
    },
    "TIME" => mysql_time(&value),
    "TINYBLOB" | "MEDIUMBLOB" | "BLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => {
      let bytes = ValueRef::to_owned(&value).try_decode::<Vec<u8>>().map_err(display_error)?;
      Ok(mysql_bytes_value(bytes))
    }
    // SRID + WKB：内容上可能恰好是合法 UTF-8，但它从来不是文本。
    // sqlx 的 Vec<u8> 不认 GEOMETRY 这个类型名，只能跳过类型检查取字节
    "GEOMETRY" => tagged_binary_value(ValueRef::to_owned(&value).try_decode_unchecked::<Vec<u8>>()),
    "NULL" => Ok(JsonValue::Null),
    _ => Err(QueryError::message(format!("{UNSUPPORTED_COLUMN_TYPE}: {type_name}"))),
  }
}

/// 日期、DATETIME 与 TIMESTAMP 照协议里的字段拼成 MySQL 自己的写法，不经日历类型。
///
/// 旧库里常见零日期 `0000-00-00` 与部分为零的 `2020-00-15`（宽松的 sql_mode 下写得进去）。
/// sqlx 拿它们去构造日历日期，整条语句报错、这种表整张打不开；全零的还被 `is_null` 当成 NULL。
/// TIMESTAMP 不必换时区：sqlx 连接时把会话时区设成 +00:00，字段就是 UTC，写回时同一个会话
/// 按同一个时区解释，往返不变。
///
/// 二进制格式是长度字节加 0、4、7 或 11 个字节：年（u16 小端）、月、日、时、分、秒、微秒（u32 小端），
/// 省掉的尾部都是 0。文本格式本来就是服务端的写法，最短的 `0000-00-00` 也有 10 字节，
/// 和二进制的总长（1、5、8、12）不会撞；sqlx 不公开值的格式，只好按这个认
pub(crate) fn mysql_date_time(
  value: &MySqlValueRef<'_>,
  type_name: &str,
) -> Result<JsonValue, QueryError> {
  let tag = if type_name == "DATE" { "date" } else { "datetime" };
  // 只是拿字节，不按类型解，所以绕过兼容检查。拿不到字节只有一种情形：协议里这一格就是 NULL
  // （零日期有值，是一个 0 长度字节）
  let Ok(bytes) = ValueRef::to_owned(value).try_decode_unchecked::<Vec<u8>>() else {
    return Ok(JsonValue::Null);
  };
  let binary = matches!(
    bytes.as_slice(),
    [length @ (0 | 4 | 7 | 11), rest @ ..] if usize::from(*length) == rest.len()
  );
  if !binary {
    return Ok(tagged_value(tag, String::from_utf8_lossy(&bytes).into_owned()));
  }
  let field = |index: usize| bytes.get(index).copied().unwrap_or(0);
  let year = u16::from_le_bytes([field(1), field(2)]);
  let date = format!("{year:04}-{:02}-{:02}", field(3), field(4));
  if type_name == "DATE" {
    return Ok(tagged_value(tag, date));
  }
  let micros = u32::from_le_bytes([field(8), field(9), field(10), field(11)]);
  let time = Time::from_hms_micro(field(5), field(6), field(7), micros).map_err(display_error)?;
  Ok(tagged_value(tag, format!("{date} {}", format_time(time))))
}

pub(crate) fn decode_postgres(value: PgValueRef<'_>) -> Result<JsonValue, QueryError> {
  if value.is_null() {
    return Ok(JsonValue::Null);
  }

  let type_info = value.type_info();
  let type_name = type_info.name();
  // 文本格式（见 `pg_decodes_binary`）：sqlx 解不了文本的区间、数组与认不得的类型直接用服务端的文本，
  // 其余的 sqlx 两种格式都认，照下面的分支走
  if value.format() == sqlx::postgres::PgValueFormat::Text {
    let text = || value.as_str().map(str::to_owned).map_err(display_error);
    match type_name {
      "INTERVAL" => return Ok(tagged_value("time", text()?)),
      _ if matches!(type_info.kind(), sqlx::postgres::PgTypeKind::Array(_))
        || !pg_decodes_binary(&type_info) =>
      {
        return Ok(JsonValue::String(text()?))
      }
      _ => {}
    }
  }
  match type_name {
    "INT2" => json_value(ValueRef::to_owned(&value).try_decode::<i16>()),
    "INT4" => json_value(ValueRef::to_owned(&value).try_decode::<i32>()),
    // 数按 numeric 存，经 serde_json 读成双精度会改掉超出 u64 的整数与长小数，照服务端的原文给。
    // jsonb 的二进制格式是版本字节 1 加文本；文本格式与 json 本来就是原文
    "JSON" | "JSONB" => ValueRef::to_owned(&value)
      .try_decode_unchecked::<String>()
      .map(|text| {
        let text = text.strip_prefix('\u{1}').map(str::to_string).unwrap_or(text);
        tagged_value("json", text)
      })
      .map_err(display_error),
    "CHAR" | "VARCHAR" | "TEXT" | "NAME" => {
      json_value(ValueRef::to_owned(&value).try_decode::<String>())
    }
    // UUID 此前与字符串共用分支，但 sqlx 不允许把 UUID 解成 String，
    // 任何 uuid 列都会报 mismatched types
    "UUID" => tagged_display_value("text", ValueRef::to_owned(&value).try_decode::<uuid::Uuid>()),
    "INT8" => tagged_display_value("bigint", ValueRef::to_owned(&value).try_decode::<i64>()),
    "NUMERIC" => {
      if let Some(special) = pg_numeric_special(&value) {
        return Ok(tagged_value("decimal", special.to_owned()));
      }
      let decimal = ValueRef::to_owned(&value)
        .try_decode::<sqlx::types::BigDecimal>()
        .map_err(QueryError::from)?;
      let decimal = match pg_numeric_display_scale(&value) {
        Some(scale) => decimal.with_scale(scale),
        None => decimal,
      };
      Ok(tagged_value("decimal", decimal_text(&decimal)))
    }
    "FLOAT4" => {
      float_value(ValueRef::to_owned(&value).try_decode::<f32>().map(widen_f32), "Infinity")
    }
    "FLOAT8" => float_value(ValueRef::to_owned(&value).try_decode::<f64>(), "Infinity"),
    "BOOL" => json_value(ValueRef::to_owned(&value).try_decode::<bool>()),
    "DATE" | "TIMESTAMP" | "TIMESTAMPTZ" => pg_instant(&value, type_name),
    "TIME" => pg_time(&value),
    "BYTEA" => tagged_binary_value(ValueRef::to_owned(&value).try_decode::<Vec<u8>>()),
    "TEXT[]" | "VARCHAR[]" | "NAME[]" => {
      pg_array_value(ValueRef::to_owned(&value).try_decode::<Vec<Option<String>>>())
    }
    "INT2[]" => pg_array_value(ValueRef::to_owned(&value).try_decode::<Vec<Option<i16>>>()),
    "INT4[]" => pg_array_value(ValueRef::to_owned(&value).try_decode::<Vec<Option<i32>>>()),
    "INT8[]" => pg_array_value(ValueRef::to_owned(&value).try_decode::<Vec<Option<i64>>>()),
    "INTERVAL" => {
      let interval = ValueRef::to_owned(&value)
        .try_decode::<sqlx::postgres::types::PgInterval>()
        .map_err(QueryError::from)?;
      Ok(tagged_value("time", format_pg_interval(&interval)))
    }
    "VOID" => Ok(JsonValue::Null),
    // 枚举的二进制与文本形式都是标签本身
    _ if matches!(type_info.kind(), sqlx::postgres::PgTypeKind::Enum(_)) => {
      let bytes = value.as_bytes().map_err(display_error)?;
      Ok(JsonValue::String(String::from_utf8_lossy(bytes).into_owned()))
    }
    // 元素逐个按 String 解，而解码本身不查类型；只是 try_decode 那道兼容检查不认枚举
    _ if pg_is_enum_array(&type_info) => {
      pg_array_value(ValueRef::to_owned(&value).try_decode_unchecked::<Vec<Option<String>>>())
    }
    _ => Err(QueryError::message(format!("{UNSUPPORTED_COLUMN_TYPE}: {type_name}"))),
  }
}

/// 日期是 2000-01-01 起的天数（i32），时间戳是同一刻起的微秒数（i64），两端的最值是 `infinity` /
/// `-infinity`。sqlx 拿它们当普通的数去加：日期与带时区的时间戳 panic（`overflow adding duration
/// to date`），不带时区的读成 294277 年。这里自己加，越出 `time` / `chrono` 的范围就报错
pub(crate) fn pg_instant(value: &PgValueRef<'_>, type_name: &str) -> Result<JsonValue, QueryError> {
  let tag = if type_name == "DATE" { "date" } else { "datetime" };
  if value.format() == sqlx::postgres::PgValueFormat::Text {
    let text = value.as_str().map_err(display_error)?;
    // 日期与 timestamp 服务端的文本（ISO 的 DateStyle）就是下面二进制分支拼出的写法
    if type_name != "TIMESTAMPTZ" || matches!(text, "infinity" | "-infinity") {
      return Ok(tagged_value(tag, text.to_owned()));
    }
    // sqlx 对文本是解析而不是去加，不会越界
    return tagged_formatted_value(
      tag,
      ValueRef::to_owned(value).try_decode::<DateTime<Utc>>(),
      |value| value.to_rfc3339(),
    );
  }
  let out_of_range = || QueryError::message(format!("{type_name} out of range"));
  let bytes = value.as_bytes().map_err(display_error)?;
  if type_name == "DATE" {
    let days = i32::from_be_bytes(bytes.try_into().map_err(display_error)?);
    return Ok(tagged_value(
      tag,
      match days {
        i32::MAX => "infinity".to_owned(),
        i32::MIN => "-infinity".to_owned(),
        days => {
          let julian_day = PG_EPOCH_JULIAN_DAY.checked_add(days).ok_or_else(out_of_range)?;
          pg_era(Date::from_julian_day(julian_day).map_err(|_| out_of_range())?, None)
        }
      },
    ));
  }
  let micros = i64::from_be_bytes(bytes.try_into().map_err(display_error)?);
  let text = match micros {
    i64::MAX => "infinity".to_owned(),
    i64::MIN => "-infinity".to_owned(),
    micros if type_name == "TIMESTAMP" => {
      let epoch = Date::from_julian_day(PG_EPOCH_JULIAN_DAY).map_err(display_error)?.midnight();
      let stamp =
        epoch.checked_add(time::Duration::microseconds(micros)).ok_or_else(out_of_range)?;
      pg_era(stamp.date(), Some(stamp.time()))
    }
    micros => DateTime::<Utc>::from_timestamp_micros(
      micros.checked_add(PG_EPOCH_UNIX_MICROS).ok_or_else(out_of_range)?,
    )
    .ok_or_else(out_of_range)?
    .to_rfc3339(),
  };
  Ok(tagged_value(tag, text))
}

/// 公元前照 psql 写：年份记作公元前第几年，` BC` 放在最后（`0044-03-15 12:00:00 BC`）。
/// `time` 按天文纪年，公元前 1 年是第 0 年，直接写出来是 `-043-03-15`
pub(crate) fn pg_era(date: Date, time: Option<Time>) -> String {
  let (date, era) = match date.year() {
    year if year > 0 => (format_date(date), ""),
    year => (format!("{:04}-{:02}-{:02}", 1 - year, u8::from(date.month()), date.day()), " BC"),
  };
  match time {
    Some(time) => format!("{date} {}{era}", format_time(time)),
    None => format!("{date}{era}"),
  }
}

/// time 是午夜起的微秒数。`24:00:00` 是合法值，sqlx 从午夜加上去就绕回 `00:00:00`
pub(crate) fn pg_time(value: &PgValueRef<'_>) -> Result<JsonValue, QueryError> {
  if value.format() == sqlx::postgres::PgValueFormat::Text {
    return Ok(tagged_value("time", value.as_str().map_err(display_error)?.to_owned()));
  }
  let bytes = value.as_bytes().map_err(display_error)?;
  let micros = i64::from_be_bytes(bytes.try_into().map_err(display_error)?);
  if micros == 86_400_000_000 {
    return Ok(tagged_value("time", "24:00:00".to_owned()));
  }
  let seconds = micros.div_euclid(1_000_000);
  let time = Time::from_hms_micro(
    u8::try_from(seconds / 3600).map_err(display_error)?,
    u8::try_from(seconds / 60 % 60).map_err(display_error)?,
    u8::try_from(seconds % 60).map_err(display_error)?,
    u32::try_from(micros.rem_euclid(1_000_000)).map_err(display_error)?,
  )
  .map_err(display_error)?;
  Ok(tagged_value("time", format_time(time)))
}

/// PostgreSQL 日期与时间戳的起点 2000-01-01（UTC）
const PG_EPOCH_JULIAN_DAY: i32 = 2_451_545;
const PG_EPOCH_UNIX_MICROS: i64 = 946_684_800_000_000;

/// `time` 的 Display 是给人读的调试形态：`7:04:05.0`——小时不补零、秒后面恒带
/// 小数，`OffsetDateTime` 还挂一段 ` +00:00:00`。数据库自己不这么写，日期选择器
/// 认不出（小时要两位），拿它去比较或写回也要赌服务端的宽容。
/// 这里照数据库的文本输出：两位小时，小数秒只在非零时出现并去掉末尾的 0
/// （PostgreSQL 就是这么输出的；MySQL 按列的精度补 0，值相同）。
pub(crate) fn format_date(date: Date) -> String {
  format!("{:04}-{:02}-{:02}", date.year(), u8::from(date.month()), date.day())
}

pub(crate) fn format_time(time: Time) -> String {
  let whole = format!("{:02}:{:02}:{:02}", time.hour(), time.minute(), time.second());
  // 按纳秒写：MySQL 与 PostgreSQL 只到微秒，去掉末尾的 0 之后一样；SQL Server 到 100 纳秒
  match time.nanosecond() {
    0 => whole,
    nanos => format!("{whole}.{}", format!("{nanos:09}").trim_end_matches('0')),
  }
}

pub(crate) fn format_datetime(value: PrimitiveDateTime) -> String {
  format!("{} {}", format_date(value.date()), format_time(value.time()))
}

/// 十进制数照数据库自己的文本输出写：不用科学计数法，标度里的 0 一个不少。
///
/// `BigDecimal` 的 `Display` 两处不照：零不管标度一律写成 `0`（`NUMERIC(10,2)` 的
/// `0.00` 在网格里是 `0`，而同一列别的行都带两位小数），小数点后前导 0 多了改写成
/// `1E-8`。
pub(crate) fn decimal_text(value: &sqlx::types::BigDecimal) -> String {
  value.to_plain_string()
}

/// `NaN`（PG 14 起还有 `Infinity` / `-Infinity`）BigDecimal 装不下。二进制头部第三个 16 位整数是符号，
/// 这几个值各有一个专用的符号值；文本格式就是这几个词
pub(crate) fn pg_numeric_special(value: &PgValueRef<'_>) -> Option<&'static str> {
  if value.format() == sqlx::postgres::PgValueFormat::Text {
    return ["NaN", "Infinity", "-Infinity"]
      .into_iter()
      .find(|word| value.as_str().ok() == Some(*word));
  }
  match value.as_bytes().ok()? {
    [_, _, _, _, 0xC0, 0x00, ..] => Some("NaN"),
    [_, _, _, _, 0xD0, 0x00, ..] => Some("Infinity"),
    [_, _, _, _, 0xF0, 0x00, ..] => Some("-Infinity"),
    _ => None,
  }
}

/// PostgreSQL 二进制 NUMERIC 头里的 `dscale`：声明的（或字面量自带的）小数位数。
///
/// sqlx 0.8 转 BigDecimal 时不看它，标度按万进制数字组的个数算：`NUMERIC(10,2)`
/// 的 `10.50` 读成 `10.5000`，`10.00` 读成 `10`，`10000` 的标度是负的。PostgreSQL
/// 自己的文本输出正是按 dscale 写的，这里照它重设标度。重设不丢值：dscale 之外
/// 的数字按协议恒为 0。
///
/// 头部是四个 16 位大端整数：ndigits、weight、sign、dscale。文本格式的值没有这个头。
pub(crate) fn pg_numeric_display_scale(value: &PgValueRef<'_>) -> Option<i64> {
  if value.format() != sqlx::postgres::PgValueFormat::Binary {
    return None;
  }
  match value.as_bytes().ok()? {
    [_, _, _, _, _, _, high, low, ..] => Some(i64::from(u16::from_be_bytes([*high, *low]))),
    _ => None,
  }
}

/// MySQL 的 TIME 是时长而非时刻（-838:59:59 ~ 838:59:59），装不进 time::Time。
///
/// sqlx 0.8.6 在两条路上都把负号弄丢过：转 time::Duration 时符号只给了整秒（`-00:00:01.25`
/// 成了 -0.75 秒），解析文本时把 `-00` 当整数读成 0（`-00:00:01.25` 成了正的）。
/// 所以二进制只借它拆字段，文本直接用服务端的写法。认格式的办法同 [`mysql_date_time`]：
/// 二进制是长度字节加 0、8 或 12 个字节，文本最短的 `00:00:00` 也有 8 字节且不以这几个字节开头
pub(crate) fn mysql_time(value: &MySqlValueRef<'_>) -> Result<JsonValue, QueryError> {
  let Ok(bytes) = ValueRef::to_owned(value).try_decode_unchecked::<Vec<u8>>() else {
    return Ok(JsonValue::Null);
  };
  let binary = matches!(
    bytes.as_slice(),
    [length @ (0 | 8 | 12), rest @ ..] if usize::from(*length) == rest.len()
  );
  if !binary {
    // 服务端按列的精度补足了小数位（`TIME(6)` 的 `12:00:00.500000`）；去掉，与二进制那一路一样
    let text = String::from_utf8_lossy(&bytes);
    let trimmed =
      if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { &text };
    return Ok(tagged_value("time", trimmed.to_owned()));
  }
  let time = ValueRef::to_owned(value).try_decode::<MySqlTime>().map_err(QueryError::from)?;
  Ok(tagged_value("time", format_mysql_time(&time)))
}

/// 按 MySQL 自己的 `[-]HH:MM:SS[.ffffff]` 写，小数秒去掉末尾的 0
pub(crate) fn format_mysql_time(time: &MySqlTime) -> String {
  // 问 `sign()`，不问 `is_negative()`：sqlx 0.8.6 的后者返回的是 `is_positive()`
  let sign = if time.sign().is_negative() { "-" } else { "" };
  let whole = format!("{sign}{:02}:{:02}:{:02}", time.hours(), time.minutes(), time.seconds());
  // 小数秒的写法与 `format_time` 相同
  match time.microseconds() {
    0 => whole,
    micros => format!("{whole}.{}", format!("{micros:06}").trim_end_matches('0')),
  }
}

/// PostgreSQL 的 interval 由「月 / 日 / 微秒」三段组成，没有统一的标量表示，
/// 这里按 PostgreSQL 默认的文本形式（IntervalStyle `postgres`）拼回去：`1 year 2 mons 3 days 04:05:06.5`。
///
/// 照 `EncodeInterval` 的规矩：各段为 0 就不写，只有 1 时用单数（`-1 days` 是复数）；
/// 前一段为负而这一段为正时带 `+`，否则 `-1 days +02:00:00` 读起来像 `-1 days -02:00:00`。
/// 此前写成 `1 days 7384 secs`，写回去 PostgreSQL 认，人读得先心算
pub(crate) fn format_pg_interval(interval: &sqlx::postgres::types::PgInterval) -> String {
  let mut text = String::new();
  let mut previous_negative = false;
  let date_parts = [
    (i64::from(interval.months / 12), "year"),
    (i64::from(interval.months % 12), "mon"),
    (i64::from(interval.days), "day"),
  ];
  for (value, unit) in date_parts {
    if value == 0 {
      continue;
    }
    let separator = if text.is_empty() { "" } else { " " };
    let plus = if previous_negative && value > 0 { "+" } else { "" };
    let plural = if value == 1 { "" } else { "s" };
    text.push_str(&format!("{separator}{plus}{value} {unit}{plural}"));
    previous_negative = value < 0;
  }

  let micros = interval.microseconds;
  if text.is_empty() || micros != 0 {
    let separator = if text.is_empty() { "" } else { " " };
    let sign = if micros < 0 {
      "-"
    } else if previous_negative {
      "+"
    } else {
      ""
    };
    let magnitude = micros.unsigned_abs();
    let hours = magnitude / 3_600_000_000;
    let minutes = magnitude / 60_000_000 % 60;
    let seconds = magnitude / 1_000_000 % 60;
    let fraction = magnitude % 1_000_000;
    text.push_str(&format!("{separator}{sign}{hours:02}:{minutes:02}:{seconds:02}"));
    if fraction != 0 {
      text.push_str(format!(".{fraction:06}").trim_end_matches('0'));
    }
  }
  text
}

/// `decode_postgres` 按二进制格式解得了的列。结果里有一列不在这里，整条语句就改走
/// 简单查询协议：服务端给每一列发文本，认不得的类型照这段文本显示
pub(crate) fn pg_decodes_binary(type_info: &sqlx::postgres::PgTypeInfo) -> bool {
  matches!(
    type_info.name(),
    "INT2"
      | "INT4"
      | "INT8"
      | "FLOAT4"
      | "FLOAT8"
      | "NUMERIC"
      | "BOOL"
      | "JSON"
      | "JSONB"
      | "CHAR"
      | "VARCHAR"
      | "TEXT"
      | "NAME"
      | "UUID"
      | "DATE"
      | "TIME"
      | "TIMESTAMP"
      | "TIMESTAMPTZ"
      | "INTERVAL"
      | "BYTEA"
      | "TEXT[]"
      | "VARCHAR[]"
      | "NAME[]"
      | "INT2[]"
      | "INT4[]"
      | "INT8[]"
      | "VOID"
  ) || matches!(type_info.kind(), sqlx::postgres::PgTypeKind::Enum(_))
    || pg_is_enum_array(type_info)
}

pub(crate) fn pg_is_enum_array(type_info: &sqlx::postgres::PgTypeInfo) -> bool {
  matches!(
    type_info.kind(),
    sqlx::postgres::PgTypeKind::Array(element)
      if matches!(element.kind(), sqlx::postgres::PgTypeKind::Enum(_))
  )
}

/// PostgreSQL 数组按它自己的文本输出（`array_out`）写成一个字符串：`{a,"b,c",NULL}`。
///
/// 交给界面一个 JSON 数组的话，网格按 `String(value)` 印成 `a,b,c`——分不清
/// `{a,b}` 和 `{"a,b"}`，也看不出 NULL 元素；改完写回去的也不是数组字面量。
/// 这个字面量原样绑回去（带 `::text[]` 之类的转换）就是同一个值。
pub(crate) fn pg_array_value<T, E>(
  value: Result<Vec<Option<T>>, E>,
) -> Result<JsonValue, QueryError>
where
  T: ToString,
  E: std::fmt::Display,
{
  let items = value.map_err(display_error)?;
  let elements = items
    .iter()
    .map(|item| match item {
      None => "NULL".to_string(),
      Some(item) => pg_array_element(&item.to_string()),
    })
    .collect::<Vec<_>>();
  Ok(JsonValue::String(format!("{{{}}}", elements.join(","))))
}

/// `array_out` 的加引号规则：空串、字面的 NULL、含分隔符 / 花括号 / 引号 /
/// 反斜杠 / 空白的元素要加双引号，引号与反斜杠再各用反斜杠转义
pub(crate) fn pg_array_element(text: &str) -> String {
  let needs_quotes = text.is_empty()
    || text.eq_ignore_ascii_case("NULL")
    || text.chars().any(|c| matches!(c, ',' | '{' | '}' | '"' | '\\') || c.is_ascii_whitespace());
  if !needs_quotes {
    return text.to_string();
  }
  let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
  format!("\"{escaped}\"")
}

/// JSON 里没有 NaN 与无穷，serde_json 把它们变成 null——网格上看着是 NULL。这几个按数据库自己的
/// 拼法给文本（`infinity` 是正无穷那个词，负的前面加 `-`）
pub(crate) fn float_value<E: std::fmt::Display>(
  value: Result<f64, E>,
  infinity: &str,
) -> Result<JsonValue, QueryError> {
  let value = value.map_err(display_error)?;
  Ok(match serde_json::Number::from_f64(value) {
    Some(number) => JsonValue::Number(number),
    None if value.is_nan() => JsonValue::from("NaN"),
    None if value > 0.0 => JsonValue::from(infinity),
    None => JsonValue::from(format!("-{infinity}")),
  })
}

/// 单精度按它自己的最短写法放宽：直接转 f64 的话 `0.1` 成了 0.10000000149011612，
/// 而数据库的客户端写的是 0.1。两者是同一个 f32，写回去不变
pub(crate) fn widen_f32(value: f32) -> f64 {
  value.to_string().parse().unwrap_or(f64::from(value))
}

pub(crate) fn json_value<T, E>(value: Result<T, E>) -> Result<JsonValue, QueryError>
where
  T: Serialize,
  E: std::fmt::Display,
{
  let decoded = value.map_err(display_error)?;
  serde_json::to_value(decoded).map_err(display_error)
}

pub(crate) fn tagged_display_value<T, E>(
  value_type: &str,
  value: Result<T, E>,
) -> Result<JsonValue, QueryError>
where
  T: std::fmt::Display,
  E: std::fmt::Display,
{
  value.map(|value| tagged_value(value_type, value.to_string())).map_err(display_error)
}

pub(crate) fn tagged_formatted_value<T, E>(
  value_type: &str,
  value: Result<T, E>,
  format: impl FnOnce(T) -> String,
) -> Result<JsonValue, QueryError>
where
  E: std::fmt::Display,
{
  value.map(|value| tagged_value(value_type, format(value))).map_err(display_error)
}

pub(crate) fn tagged_json_value<E>(value: Result<JsonValue, E>) -> Result<JsonValue, QueryError>
where
  E: std::fmt::Display,
{
  let value = value.map_err(display_error)?;
  let serialized = serde_json::to_string(&value).map_err(display_error)?;
  Ok(tagged_value("json", serialized))
}

/// MySQL 家的「二进制」列里哪些其实是文本。
///
/// sqlx 只凭列上的 BINARY 标志给类型名，拿不到字符集；而 `_bin` 排序规则的文本列
/// 也带这个标志——MariaDB 的 JSON 列（LONGTEXT + utf8mb4_bin）、大小写敏感的
/// varchar、MySQL 8 的 utf8mb4_0900_bin，于是全被画成 0x7b2261…。
/// 按内容判：合法 UTF-8 且没有控制字符（制表、换行除外）就当文本。真正的二进制
/// （哈希、UUID、图片）几乎不可能恰好满足：16 个随机字节全落在可打印范围的概率
/// 在 1e-7 量级。
pub(crate) fn mysql_bytes_value(bytes: Vec<u8>) -> JsonValue {
  match String::from_utf8(bytes) {
    Ok(text) if !text.chars().any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r')) => {
      JsonValue::String(text)
    }
    Ok(text) => binary_json(text.as_bytes()),
    Err(error) => binary_json(error.as_bytes()),
  }
}

/// 二进制单元格：`{ type: "binary", value: <十六进制> }`。各驱动共用这一份，前端只认这一种形状
pub(crate) fn binary_json(bytes: &[u8]) -> JsonValue {
  tagged_value("binary", encode_hex(bytes))
}

/// 小写十六进制。原来逐字节 `format!` 一次——一个 50 MB 的 BLOB 就是五千万次堆分配；
/// 这里一次分配到位，按表查字符
pub(crate) fn encode_hex(bytes: &[u8]) -> String {
  const DIGITS: &[u8; 16] = b"0123456789abcdef";
  let mut encoded = String::with_capacity(bytes.len() * 2);
  for byte in bytes {
    encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
    encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
  }
  encoded
}

pub(crate) fn tagged_binary_value<E>(value: Result<Vec<u8>, E>) -> Result<JsonValue, QueryError>
where
  E: std::fmt::Display,
{
  value.map(|bytes| binary_json(&bytes)).map_err(display_error)
}

/// 解码失败、序列化失败这类：错误来自我们这一侧，没有数据库给的结构可取，
/// 只留一句话。和 `QueryError::from(sqlx::Error)` 分开是为了不把
/// 「数据库说的」和「我们说的」混成一种东西。
pub(crate) fn display_error(error: impl std::fmt::Display) -> QueryError {
  QueryError::message(error.to_string())
}

/// 同名的列用编号区分：结果行按列名做键，`SELECT a.id, b.id` 的两个 `id` 会互相覆盖。
/// 各家的流式读取都用它（SQL Server 先给没名字的列起名，见那边的 `label_columns`）。
/// 编号跳过已经有列叫的名字：`SELECT 1 AS id, 2 AS id, 3 AS "id 2"` 不能编出第二个 `id 2`
pub(crate) fn number_duplicate_columns<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
  let names: Vec<&str> = names.collect();
  let mut taken: std::collections::HashSet<String> =
    names.iter().map(|name| name.to_string()).collect();
  let mut seen = std::collections::HashMap::<&str, usize>::new();
  names
    .iter()
    .map(|&name| {
      let count = seen.entry(name).or_insert(0);
      *count += 1;
      if *count == 1 {
        return name.to_string();
      }
      loop {
        let label = format!("{name} {count}");
        if taken.insert(label.clone()) {
          return label;
        }
        *count += 1;
      }
    })
    .collect()
}

pub(crate) fn tagged_value(value_type: &str, value: String) -> JsonValue {
  JsonValue::Object(Map::from_iter([
    ("type".to_string(), JsonValue::String(value_type.to_string())),
    ("value".to_string(), JsonValue::String(value)),
  ]))
}
