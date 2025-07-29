use anyhow::Result;
use gix::bstr::ByteSlice;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct RepoStatus {
    path: PathBuf,
    branch: String,
    is_dirty: bool,
}

pub fn execute(output_format: &str, base_dir: &str) -> Result<()> {
    let repos = find_git_repositories(base_dir)?;
    
    match output_format {
        "tree" => print_tree(&repos, base_dir),
        "flat" => print_flat(&repos),
        "dump" => print_dump(&repos),
        _ => return Err(anyhow::anyhow!("Invalid output format: {}", output_format)),
    }
    
    Ok(())
}

fn find_git_repositories(base_dir: &str) -> Result<Vec<RepoStatus>> {
    let mut repos = Vec::new();
    find_repos_recursive(Path::new(base_dir), &mut repos)?;
    repos.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(repos)
}

fn find_repos_recursive(dir: &Path, repos: &mut Vec<RepoStatus>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    
    // Don't recurse into git repositories
    let git_dir = dir.join(".git");
    if git_dir.exists() && git_dir.is_dir() {
        if let Ok(status) = get_repo_status(dir) {
            repos.push(status);
        }
        return Ok(()); // Don't recurse into git repositories
    }
    
    // Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    find_repos_recursive(&entry.path(), repos)?;
                }
            }
        }
    }
    
    Ok(())
}

fn get_repo_status(repo_path: &Path) -> Result<RepoStatus> {
    let repo = gix::open(repo_path)?;
    
    // Get current branch
    let head = repo.head_ref()?;
    let branch = head
        .as_ref()
        .and_then(|r| r.name().as_bstr().to_str().ok())
        .unwrap_or("HEAD")
        .to_string();
    
    // Check if working directory is dirty
    // For now, we'll just check if there are any untracked or modified files
    // This is a simplified check - a full implementation would need more detail
    let is_dirty = false; // TODO: Implement proper dirty check with gix
    
    Ok(RepoStatus {
        path: repo_path.to_path_buf(),
        branch,
        is_dirty,
    })
}

fn print_tree(repos: &[RepoStatus], base_dir: &str) {
    println!("Git repositories in {}:", base_dir);
    println!();
    
    for repo in repos {
        let relative_path = repo.path.strip_prefix(base_dir)
            .unwrap_or(&repo.path);
        
        let status_indicator = if repo.is_dirty { "*" } else { "" };
        println!("  {} ({}{})", 
            relative_path.display(), 
            repo.branch,
            status_indicator
        );
    }
    
    if repos.is_empty() {
        println!("  No git repositories found");
    }
}

fn print_flat(repos: &[RepoStatus]) {
    for repo in repos {
        let status_indicator = if repo.is_dirty { "*" } else { "" };
        println!("{} ({}{})", 
            repo.path.display(), 
            repo.branch,
            status_indicator
        );
    }
}

fn print_dump(repos: &[RepoStatus]) {
    for repo in repos {
        // Try to get the remote URL
        if let Ok(gix_repo) = gix::open(&repo.path) {
            if let Ok(remote) = gix_repo.find_remote("origin") {
                if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                    println!("{} {}", url.to_bstring(), repo.branch);
                }
            }
        }
    }
}