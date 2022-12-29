use crate::parse_env;
use ahash::AHashMap;
use http::{HeaderMap, HeaderValue, StatusCode};
use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::{interval, Instant};

#[cfg(feature = "expose-metrics")]
use std::env;

#[cfg(feature = "expose-metrics")]
use metrics::{gauge, increment_gauge};

#[cfg(feature = "expose-metrics")]
lazy_static! {
    static ref METRIC_KEY_USERS: String = format!(
        "{}_cache_users",
        env::var("METRIC_KEY").unwrap_or_else(|_| "twilight_http_proxy".into())
    );
}

#[cfg(feature = "expose-metrics")]
lazy_static! {
    static ref METRIC_KEY_INVITES: String = format!(
        "{}_cache_invites",
        env::var("METRIC_KEY").unwrap_or_else(|_| "twilight_http_proxy".into())
    );
}
lazy_static! {
    static ref CACHE_DURATION: u64 = parse_env("CACHE_DURATION").unwrap_or(60 * 10);
}

pub struct CachedResponse {
    cached_at: Instant,
    bytes: Vec<u8>,
    headers: HeaderMap<HeaderValue>,
    statuscode: StatusCode,
}

impl CachedResponse {
    pub fn new(
        bytes: Vec<u8>,
        headers: HeaderMap<HeaderValue>,
        statuscode: StatusCode,
    ) -> CachedResponse {
        CachedResponse {
            cached_at: Instant::now(),
            bytes,
            headers,
            statuscode,
        }
    }
}

pub struct Cache {
    users: RwLock<AHashMap<String, CachedResponse>>,
    invites: RwLock<AHashMap<String, CachedResponse>>,
}

impl Cache {
    pub fn new() -> Arc<Cache> {
        let c = Arc::new(Cache {
            users: Default::default(),
            invites: Default::default(),
        });

        tokio::spawn(reaper(c.clone()));

        c
    }

    pub fn insert_user(
        &self,
        key: String,
        value: Vec<u8>,
        headers: HeaderMap<HeaderValue>,
        statuscode: StatusCode,
    ) {
        #[cfg(feature = "expose-metrics")]
        increment_gauge!(METRIC_KEY_USERS.as_str(), 1f64);
        insert(&self.users, key, value, headers, statuscode)
    }

    pub fn insert_invite(
        &self,
        key: String,
        value: Vec<u8>,
        headers: HeaderMap<HeaderValue>,
        statuscode: StatusCode,
    ) {
        #[cfg(feature = "expose-metrics")]
        increment_gauge!(METRIC_KEY_INVITES.as_str(), 1f64);
        insert(&self.invites, key, value, headers, statuscode)
    }

    pub fn get_user(&self, key: &str) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
        get(&self.users, key)
    }

    pub fn get_invite(&self, key: &str) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
        get(&self.users, key)
    }
}

pub async fn reaper(cache: Arc<Cache>) {
    let mut interval = interval(Duration::from_secs(120));
    loop {
        interval.tick().await;
        clean_cache(&cache.invites);
        clean_cache(&cache.users);

        #[cfg(feature = "expose-metrics")]
        {
            gauge!(
                METRIC_KEY_INVITES.as_str(),
                cache.invites.read().expect("Cache got poisoned").len() as f64
            );
            gauge!(
                METRIC_KEY_USERS.as_str(),
                cache.users.read().expect("Cache got poisoned").len() as f64
            );
        }
    }
}

fn clean_cache(cache: &RwLock<AHashMap<String, CachedResponse>>) {
    let now = Instant::now();
    let mut cache = cache.write().expect("Cache got poisoned");
    cache.retain(|_, value| (now - value.cached_at).as_secs() < *CACHE_DURATION);
}

pub fn get(
    cache: &RwLock<AHashMap<String, CachedResponse>>,
    key: &str,
) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
    if let Some(cached) = cache.read().expect("cache got poisoned").get(key) {
        if (Instant::now() - cached.cached_at).as_secs() < *CACHE_DURATION {
            return Some((
                cached.bytes.clone(),
                cached.headers.clone(),
                cached.statuscode,
            ));
        }
    }
    None
}

pub fn insert(
    cache: &RwLock<AHashMap<String, CachedResponse>>,
    key: String,
    value: Vec<u8>,
    headers: HeaderMap<HeaderValue>,
    statuscode: StatusCode,
) {
    cache
        .write()
        .expect("Cache got poisoned")
        .insert(key, CachedResponse::new(value, headers, statuscode));
}
