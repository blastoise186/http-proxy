use crate::CACHE_DURATION;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use http::{HeaderMap, HeaderValue, StatusCode};
use tokio::time::{interval, Instant};

pub struct CachedResponse {
    cached_at: Instant,
    bytes: Vec<u8>,
    headers: HeaderMap<HeaderValue>,
    statuscode: StatusCode
}

impl CachedResponse {
    pub fn new(bytes: Vec<u8>, headers: HeaderMap<HeaderValue>, statuscode: StatusCode) -> CachedResponse {
        CachedResponse {
            cached_at: Instant::now(),
            bytes,
            headers,
            statuscode
        }
    }
}

pub struct Cache {
    inner: RwLock<HashMap<String, CachedResponse>>,
}

impl Cache {
    pub fn new() -> Arc<Cache> {
        let c = Arc::new(Cache {
            inner: Default::default(),
        });

        tokio::spawn(reaper(c.clone()));

        c
    }

    pub fn insert(&self, key: String, value: Vec<u8>, headers: HeaderMap<HeaderValue>, statuscode: StatusCode) {
        self.inner.write().insert(key, CachedResponse::new(value, headers, statuscode));
    }

    pub fn get(&self, key: &str) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
        if let Some(cached) = self.inner.read().get(key) {
            if (Instant::now() - cached.cached_at).as_secs() < *CACHE_DURATION {
                return Some((cached.bytes.clone(), cached.headers.clone(), cached.statuscode));
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
        cache
            .inner
            .write()
            .retain(|_, value| (now - value.cached_at).as_secs() < *CACHE_DURATION);
    }
}
