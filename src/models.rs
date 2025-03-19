use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FustcRequest {
    GetCache,
    CacheCheck { mir: String },
    CacheSave { mir: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WrapperResponse {
    WholeCache { cache: HashSet<String> },
    CacheStatus { cached: bool },
}
