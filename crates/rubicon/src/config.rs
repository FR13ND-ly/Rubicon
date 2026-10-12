use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub origin: String,
    pub server_entry: PathBuf,
    pub browser_dir: PathBuf,
}
