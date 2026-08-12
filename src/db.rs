use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 共享数据库：newtype 包装，方便添加方法
#[derive(Debug, Clone, Default)]
pub struct Db(Arc<RwLock<HashMap<String, Vec<u8>>>>);

impl Deref for Db {
    type Target = RwLock<HashMap<String, Vec<u8>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// 创建一个新的空数据库
pub fn new() -> Db {
    Db::default()
}

impl Db {
    /// 获取 key 对应的 value（克隆）
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let guard = self.0.read().await;
        guard.get(key).cloned()
    }

    /// 插入/覆盖 key-value，返回旧值（如果有）
    pub async fn set(&self, key: String, value: Vec<u8>) -> Option<Vec<u8>> {
        let mut guard = self.0.write().await;
        guard.insert(key, value)
    }

    /// 删除 key，返回是否存在并被删除
    pub async fn del(&self, key: &str) -> bool {
        let mut guard = self.0.write().await;
        guard.remove(key).is_some()
    }

    /// 判断 key 是否存在
    pub async fn exists(&self, key: &str) -> bool {
        let guard = self.0.read().await;
        guard.contains_key(key)
    }
}
