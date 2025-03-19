pub mod models;
pub mod server;
pub mod tcp;

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub enum Metrics {
    TcpIo(String),
    Borrowck(String),
}
