pub mod ai_tasks;
pub mod auth_middleware;
pub mod frame_extractor;
pub mod leases;
pub mod recordings;
pub mod retention;
pub mod search;
pub mod state_store;
pub mod state_store_client;
pub mod streams;
pub mod thumbnail;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
