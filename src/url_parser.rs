use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub full_url: String,
}

impl RepoInfo {
    pub fn get_clone_path(&self, base_dir: &str) -> PathBuf {
        PathBuf::from(base_dir)
            .join(&self.host)
            .join(&self.owner)
            .join(&self.name)
    }
}

pub fn parse_repo_url(url: &str) -> Result<RepoInfo> {
    let url = url.trim();

    // Handle different URL formats:
    // - https://github.com/owner/repo
    // - https://github.com/owner/repo.git
    // - git@github.com:owner/repo.git
    // - github.com/owner/repo
    // - owner/repo (assume github.com)

    if url.starts_with("https://") || url.starts_with("http://") {
        parse_https_url(url)
    } else if url.starts_with("git@") {
        parse_ssh_url(url)
    } else if url.contains('/') {
        // Handle short formats
        if url.matches('/').count() == 1 {
            // owner/repo format
            parse_short_url(url)
        } else {
            // host/owner/repo format
            parse_host_url(url)
        }
    } else {
        Err(anyhow::anyhow!("Invalid repository URL format: {}", url))
    }
}

fn parse_https_url(url: &str) -> Result<RepoInfo> {
    let url = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Invalid HTTPS URL format"));
    }

    let host = parts[0];
    let owner = parts[1];
    let name = parts[2].trim_end_matches(".git");

    Ok(RepoInfo {
        host: host.to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        full_url: format!("https://{}/{}/{}.git", host, owner, name),
    })
}

fn parse_ssh_url(url: &str) -> Result<RepoInfo> {
    let url = url.trim_start_matches("git@");
    let parts: Vec<&str> = url.split(':').collect();

    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid SSH URL format"));
    }

    let host = parts[0];
    let path_parts: Vec<&str> = parts[1].split('/').collect();

    if path_parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid SSH URL path format"));
    }

    let owner = path_parts[0];
    let name = path_parts[1].trim_end_matches(".git");

    Ok(RepoInfo {
        host: host.to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        full_url: format!("git@{}:{}/{}.git", host, owner, name),
    })
}

fn parse_short_url(url: &str) -> Result<RepoInfo> {
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid short URL format"));
    }

    let owner = parts[0];
    let name = parts[1].trim_end_matches(".git");

    Ok(RepoInfo {
        host: String::new(), // Let the caller set the default host
        owner: owner.to_string(),
        name: name.to_string(),
        full_url: String::new(), // Let the caller build the URL based on scheme
    })
}

fn parse_host_url(url: &str) -> Result<RepoInfo> {
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Invalid host URL format"));
    }

    let host = parts[0];
    let owner = parts[1];
    let name = parts[2].trim_end_matches(".git");

    Ok(RepoInfo {
        host: host.to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        full_url: format!("https://{}/{}/{}.git", host, owner, name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_https_url() {
        let info = parse_repo_url("https://github.com/rust-lang/rust").unwrap();
        assert_eq!(info.host, "github.com");
        assert_eq!(info.owner, "rust-lang");
        assert_eq!(info.name, "rust");
    }

    #[test]
    fn test_parse_ssh_url() {
        let info = parse_repo_url("git@github.com:rust-lang/rust.git").unwrap();
        assert_eq!(info.host, "github.com");
        assert_eq!(info.owner, "rust-lang");
        assert_eq!(info.name, "rust");
    }

    #[test]
    fn test_parse_short_url() {
        let info = parse_repo_url("rust-lang/rust").unwrap();
        assert_eq!(info.host, "");
        assert_eq!(info.owner, "rust-lang");
        assert_eq!(info.name, "rust");
    }

    #[test]
    fn test_parse_host_url() {
        let info = parse_repo_url("gitlab.com/owner/repo").unwrap();
        assert_eq!(info.host, "gitlab.com");
        assert_eq!(info.owner, "owner");
        assert_eq!(info.name, "repo");
    }
}

