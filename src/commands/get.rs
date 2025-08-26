use crate::url_parser::{RepoInfo, parse_repo_url};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Config {
    pub base_dir: String,
    pub branch: Option<String>,
    pub default_host: String,
    pub default_scheme: String,
    pub skip_host: bool,
}

pub fn execute(url: &str, config: &Config) -> Result<()> {
    let mut repo_info = parse_repo_url(url).context("Failed to parse repository URL")?;

    // Apply default host if needed
    if repo_info.host.is_empty() {
        repo_info.host = config.default_host.clone();
    }

    // Update URL based on scheme preference
    if !url.contains("://") && !url.starts_with("git@") {
        repo_info.full_url = build_url(&repo_info, &config.default_scheme);
    }

    let clone_path = get_clone_path(&repo_info, &config.base_dir, config.skip_host);

    if clone_path.exists() {
        return Err(anyhow::anyhow!(
            "Repository already exists at: {}",
            clone_path.display()
        ));
    }

    if let Some(parent) = clone_path.parent() {
        fs::create_dir_all(parent).context("Failed to create parent directories")?;
    }

    println!("Cloning into {}", clone_path.display());

    clone_repository(&repo_info.full_url, &clone_path, &config.branch)?;

    println!(
        "Successfully cloned repository to: {}",
        clone_path.display()
    );
    Ok(())
}

pub fn execute_dump(dump_file: &str, config: &Config) -> Result<()> {
    let content = fs::read_to_string(dump_file).context("Failed to read dump file")?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(url) = parts.next() else { continue };
        
        let config_clone = Config {
            base_dir: config.base_dir.clone(),
            branch: parts.next().map(|s| s.to_string()).or_else(|| config.branch.clone()),
            default_host: config.default_host.clone(),
            default_scheme: config.default_scheme.clone(),
            skip_host: config.skip_host,
        };

        match execute(url, &config_clone) {
            Ok(_) => println!("✓ Cloned {}", url),
            Err(e) => eprintln!("✗ Failed to clone {}: {}", url, e),
        }
    }

    Ok(())
}

fn get_clone_path(repo_info: &RepoInfo, base_dir: &str, skip_host: bool) -> PathBuf {
    let mut path = PathBuf::from(base_dir);

    if !skip_host {
        path.push(&repo_info.host);
    }

    path.extend([&repo_info.owner, &repo_info.name]);
    path
}

fn build_url(repo_info: &RepoInfo, scheme: &str) -> String {
    match scheme {
        "ssh" => format!(
            "git@{}:{}/{}.git",
            repo_info.host, repo_info.owner, repo_info.name
        ),
        _ => format!(
            "https://{}/{}/{}.git",
            repo_info.host, repo_info.owner, repo_info.name
        ),
    }
}

fn clone_repository(url: &str, path: &Path, branch: &Option<String>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("clone");

    if let Some(branch) = branch {
        cmd.arg("-b").arg(branch);
    }

    cmd.arg(url).arg(path);

    let output = cmd.output().context("Failed to execute git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git clone failed: {}", stderr));
    }

    Ok(())
}
