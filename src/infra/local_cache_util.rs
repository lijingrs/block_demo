use moka::future::CacheBuilder;
use moka::Expiry;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone)]
pub struct LocalCache {}
pub struct InMemExpiry;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expiration {
    Never,
    Minutes5,
    Seconds30,
    Seconds60,
    Hours2,
    AfterDuration(Duration),
}
impl Expiration {
    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            Expiration::Never => None,
            Expiration::Minutes5 => Some(Duration::from_secs(60*5)),
            Expiration::Seconds30 => Some(Duration::from_secs(30)),
            Expiration::Seconds60 => Some(Duration::from_secs(60)),
            Expiration::Hours2 => Some(Duration::from_secs(60*60*2)),
            Expiration::AfterDuration(d) => Some(*d),
        }
    }
}
impl Expiry<String, (Expiration, String)> for InMemExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &(Expiration, String),
        _current_time: Instant,
    ) -> Option<Duration> {
        let duration = value.0.as_duration();
        info!("InMemExpiry: expire_after_create called with key {_key}. Returning {duration:?}.");
        duration
    }
}

static LOCAL_CACHE: OnceLock<moka::future::Cache<String, (Expiration, String)>> = OnceLock::new();

impl LocalCache {
    fn instance() -> &'static moka::future::Cache<String, (Expiration, String)> {
        LOCAL_CACHE.get_or_init(|| CacheBuilder::default()
            .expire_after(InMemExpiry)
            .build())
    }
    pub async fn insert(key: String, value: (Expiration, String)) {
        LocalCache::instance().insert(key, value).await;
    }
    pub async fn remove(key: &str) {
        LocalCache::instance().remove(key).await;
    }
    pub async fn get(key: &str)->Option<(Expiration, String)> {
        LocalCache::instance().get(key).await
    }
}