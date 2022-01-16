use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use tokio::time::{Instant, interval};
use crate::CACHE_DURATION;

pub struct CachedResponse {
    cached_at: Instant,
    bytes: Vec<u8>,
}

impl CachedResponse {
    pub fn new(bytes: Vec<u8>) -> CachedResponse {
        CachedResponse {
            cached_at: Instant::now(),
            bytes,
        }
    }
}

pub struct Cache {
    inner: RwLock<HashMap<String, CachedResponse>>,
}

impl Cache {
    pub fn new() -> Arc<Cache> {
        let c = Arc::new(
            Cache {
                inner: Default::default()
            }
        );

        tokio::spawn(reaper(c.clone()));

        c
    }

    pub fn insert(&self, key: String, value: Vec<u8>) {
        self.inner.write().insert(key, CachedResponse::new(value));
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(cached) = self.inner.read().get(key) {
            if (cached.cached_at - Instant::now()).as_secs() < *CACHE_DURATION {
                return Some(cached.bytes.clone());
            }
        }
        None
    }
}

pub async fn reaper(cache: Arc<Cache>) {
    let mut interval = interval(Duration::from_secs(120));
    loop {
        interval.tick().await;
        let now = Instant::now();
        cache.inner.write().retain(|_, value| (value.cached_at - now).as_secs() < *CACHE_DURATION);
    }
}
