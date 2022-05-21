use crate::CACHE_DURATION;
use http::{HeaderMap, HeaderValue, Response, StatusCode};
use hyper::Body;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, Instant};

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
    users: RwLock<FxHashMap<String, CachedResponse>>,
    invites: RwLock<FxHashMap<String, CachedResponse>>,
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
        insert(&self.users, key, value, headers, statuscode)
    }

    pub fn insert_invite(
        &self,
        key: String,
        value: Vec<u8>,
        headers: HeaderMap<HeaderValue>,
        statuscode: StatusCode,
    ) {
        insert(&self.invites, key, value, headers, statuscode)
    }

    pub fn get_user(&self, key: &str) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
        get(&self.users, key)
    }

    pub fn get_invite(&self, key: &str) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
        get(&self.users, key)
    }

    pub fn cache_status(&self) -> Response<Body> {
        let users = self.users.read().len();
        let invites = self.invites.read().len();
        let assembled = format!("{{\"users\": {users}, \"invites\": {invites}}}");
        Response::builder().body(Body::from(assembled)).unwrap()
    }
}

pub async fn reaper(cache: Arc<Cache>) {
    let mut interval = interval(Duration::from_secs(120));
    loop {
        interval.tick().await;
        clean_cache(&cache.invites);
        clean_cache(&cache.users);
    }
}

fn clean_cache(cache: &RwLock<FxHashMap<String, CachedResponse>>) {
    let now = Instant::now();
    cache
        .write()
        .retain(|_, value| (now - value.cached_at).as_secs() < *CACHE_DURATION);
}

pub fn get(
    cache: &RwLock<FxHashMap<String, CachedResponse>>,
    key: &str,
) -> Option<(Vec<u8>, HeaderMap<HeaderValue>, StatusCode)> {
    if let Some(cached) = cache.read().get(key) {
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
    cache: &RwLock<FxHashMap<String, CachedResponse>>,
    key: String,
    value: Vec<u8>,
    headers: HeaderMap<HeaderValue>,
    statuscode: StatusCode,
) {
    cache
        .write()
        .insert(key, CachedResponse::new(value, headers, statuscode));
}
