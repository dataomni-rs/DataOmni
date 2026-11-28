//! 后端自己持有的连接池，按连接串登记。
//!
//! MySQL / PostgreSQL / SQLite 的 sqlx 池子与其余驱动（SQL Server、Oracle、DuckDB、
//! ClickHouse、MongoDB、Redis、Neo4j、Elasticsearch）统一用这一套注册表按不带口令
//! （或标准连接串）的键登记；锁只在哈希表读写那几纳秒持有，绝不跨 `.await` 持锁。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct PoolRegistry<P> {
  pools: Mutex<HashMap<String, Arc<P>>>,
}

impl<P> Default for PoolRegistry<P> {
  fn default() -> Self {
    Self { pools: Mutex::new(HashMap::new()) }
  }
}

impl<P> PoolRegistry<P> {
  pub fn insert(&self, key: String, pool: Arc<P>) {
    // 把可能被覆盖的旧池移出锁外再析构，避免连接池 Drop（如 Oracle/达梦清理）在持锁期执行
    let replaced = match self.pools.lock() {
      Ok(mut pools) => pools.insert(key, pool),
      Err(_) => {
        log::error!(
          "连接池注册表锁已中毒，无法登记连接池: {}",
          crate::services::connection_service::redact_connection_string(&key)
        );
        None
      }
    };
    drop(replaced);
  }

  pub fn get(&self, key: &str) -> Option<Arc<P>> {
    match self.pools.lock() {
      Ok(pools) => pools.get(key).cloned(),
      Err(_) => {
        log::error!(
          "连接池注册表锁已中毒，无法读取连接池: {}",
          crate::services::connection_service::redact_connection_string(key)
        );
        None
      }
    }
  }

  pub fn take(&self, key: &str) -> Option<Arc<P>> {
    match self.pools.lock() {
      Ok(mut pools) => pools.remove(key),
      Err(_) => {
        log::error!(
          "连接池注册表锁已中毒，无法移除连接池: {}",
          crate::services::connection_service::redact_connection_string(key)
        );
        None
      }
    }
  }

  pub fn remove(&self, key: &str) -> bool {
    self.take(key).is_some()
  }
}

/// 两个自建池子（SQL Server、Oracle）的空闲连接，带着放回来的时刻。
///
/// 放得太久的不再拿出来用：VPN、NAT、云负载均衡会把空闲几分钟的 TCP 流悄悄丢掉，
/// 不发 RST。拿一条这样的连接去跑目录查询，要等满 30 秒超时，然后报一句「查询
/// 超时，调大超时」——而问题根本不在查询。回收时限与 sqlx 那两家一致
/// （`sqlx_pool::IDLE_TIMEOUT`）。
pub struct IdleConnections<T> {
  entries: Mutex<Vec<(T, std::time::Instant)>>,
  max: usize,
}

impl<T> IdleConnections<T> {
  pub fn new(first: T, max: usize) -> Self {
    Self { entries: Mutex::new(vec![(first, std::time::Instant::now())]), max }
  }

  /// 最近放回的一条还新鲜的连接；放太久的一并交回给调用方处置——Oracle 的连接
  /// 关的时候要走一次网络，不能在异步线程上直接 drop
  pub fn take(&self) -> (Option<T>, Vec<T>) {
    let Ok(mut entries) = self.entries.lock() else {
      return (None, Vec::new());
    };
    let mut stale = Vec::new();
    while let Some((connection, since)) = entries.pop() {
      if since.elapsed() < crate::services::sqlx_pool::IDLE_TIMEOUT {
        return (Some(connection), stale);
      }
      stale.push(connection);
    }
    (None, stale)
  }

  /// 全部交回给调用方处置。一条拿出来的连接报了断开，同一批的多半也断了：
  /// 服务端重启、DBA 清会话、故障转移都是一起断的
  pub fn drain(&self) -> Vec<T> {
    match self.entries.lock() {
      Ok(mut entries) => entries.drain(..).map(|(connection, _)| connection).collect(),
      Err(_) => Vec::new(),
    }
  }

  /// 归还一条空闲连接；若池已满或锁中毒，把多余连接原样交回调用方在锁外处置
  /// （Oracle / 达梦连接 Drop 走同步网络登出，不能在持锁期或 Tokio worker 上直接析构）
  pub fn put(&self, connection: T) -> Option<T> {
    let Ok(mut entries) = self.entries.lock() else {
      return Some(connection);
    };
    if entries.len() < self.max {
      entries.push((connection, std::time::Instant::now()));
      None
    } else {
      Some(connection)
    }
  }

  #[cfg(test)]
  fn age_all(&self, by: std::time::Duration) {
    if let Ok(mut entries) = self.entries.lock() {
      for entry in entries.iter_mut() {
        entry.1 = entry.1.checked_sub(by).unwrap_or(entry.1);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_connection_idle_past_the_timeout_is_not_handed_out_again() {
    let idle = IdleConnections::new("first", 4);
    idle.age_all(crate::services::sqlx_pool::IDLE_TIMEOUT);
    idle.put("fresh");
    // 新放回的先拿到；再拿一次，放太久的那条被交回处置，而不是拿去用
    assert_eq!(idle.take(), (Some("fresh"), vec![]));
    assert_eq!(idle.take(), (None, vec!["first"]));
  }

  #[test]
  fn draining_hands_back_every_waiting_connection() {
    let idle = IdleConnections::new(1, 4);
    idle.put(2);
    assert_eq!(idle.drain(), vec![1, 2]);
    assert_eq!(idle.take(), (None, vec![]));
  }

  #[test]
  fn at_most_max_connections_wait_in_the_list() {
    let idle = IdleConnections::new(1, 2);
    assert_eq!(idle.put(2), None);
    assert_eq!(idle.put(3), Some(3));
    assert_eq!(idle.take().0, Some(2));
  }

  #[test]
  fn replacing_a_pool_drops_previous_value_outside_the_registry_lock() {
    struct Probe {
      registry: Arc<PoolRegistry<Probe>>,
      checked: Arc<std::sync::atomic::AtomicBool>,
    }

    impl Drop for Probe {
      fn drop(&mut self) {
        // 如果 insert 在持锁期间析构旧值，这里二次加锁会立刻死锁
        let _ = self.registry.get("k");
        self.checked.store(true, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let registry = Arc::new(PoolRegistry::default());
    let checked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    registry.insert(
      "k".to_string(),
      Arc::new(Probe { registry: Arc::clone(&registry), checked: Arc::clone(&checked) }),
    );
    registry.insert(
      "k".to_string(),
      Arc::new(Probe {
        registry: Arc::clone(&registry),
        checked: Arc::new(std::sync::atomic::AtomicBool::new(false)),
      }),
    );
    assert!(checked.load(std::sync::atomic::Ordering::SeqCst));
  }
}
