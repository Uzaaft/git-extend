use anyhow::Result;
use std::env;

pub mod url_parser;
pub mod commands;

pub fn get_base_dir(provided_dir: Option<String>) -> Result<String> {
    match provided_dir {
        Some(dir) => Ok(dir),
        None => env::var("GIT_PATH").map_err(|_| anyhow::anyhow!("GIT_PATH environment variable not set"))
    }
}