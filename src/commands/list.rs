use anyhow::Result;
use gix::bstr::ByteSlice;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct RepoStatus {
    path: PathBuf,
    current_branch: String,
    all_branches: Vec<BranchInfo>,
    uncommitted_count: usize,
    untracked_count: usize,
}

#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    status: BranchStatus,
}

#[derive(Debug, Clone)]
enum BranchStatus {
    Ok,
    Ahead(usize),
    Behind(usize),
    Diverged { ahead: usize, behind: usize },
    NoUpstream,
    Uncommitted { count: usize },
    Untracked { count: usize },
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
        return Ok(());
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
    let current_branch = head
        .as_ref()
        .and_then(|r| r.name().as_bstr().to_str().ok())
        .unwrap_or("HEAD")
        .to_string();

    // Count uncommitted and untracked files
    let (uncommitted_count, untracked_count) = count_changes(&repo)?;

    // Get all branches with their status
    let all_branches = get_all_branches(&repo, uncommitted_count, untracked_count)?;

    Ok(RepoStatus {
        path: repo_path.to_path_buf(),
        current_branch,
        all_branches,
        uncommitted_count,
        untracked_count,
    })
}

fn count_changes(repo: &gix::Repository) -> Result<(usize, usize)> {
    // Use git status to count changes
    let mut uncommitted = 0;
    let mut untracked = 0;

    // Try to get the status - this is a simplified approach
    // In a real implementation, we'd use gix's status functionality
    if let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(
            repo.work_dir()
                .unwrap_or(repo.path())
                .to_string_lossy()
                .as_ref(),
        )
        .arg("status")
        .arg("--porcelain")
        .output()
    {
        let status_str = String::from_utf8_lossy(&output.stdout);
        for line in status_str.lines() {
            if line.starts_with("??") {
                untracked += 1;
            } else if !line.is_empty() {
                uncommitted += 1;
            }
        }
    }

    Ok((uncommitted, untracked))
}

fn get_all_branches(
    repo: &gix::Repository,
    uncommitted: usize,
    untracked: usize,
) -> Result<Vec<BranchInfo>> {
    let mut branches = Vec::new();

    // Get current branch info
    let current_branch_name = if let Ok(head) = repo.head_ref() {
        head.as_ref()
            .and_then(|r| r.name().as_bstr().to_str().ok())
            .unwrap_or("HEAD")
            .strip_prefix("refs/heads/")
            .unwrap_or("HEAD")
            .to_string()
    } else {
        "HEAD".to_string()
    };

    // Get current branch with proper status
    let status = get_branch_tracking_status(repo, &current_branch_name)?;

    // If we have uncommitted/untracked changes, update the status
    if uncommitted > 0 || untracked > 0 {
        // Show both the tracking status and the working directory status
        branches.push(BranchInfo {
            name: current_branch_name.clone(),
            status,
        });

        if uncommitted > 0 {
            branches.push(BranchInfo {
                name: String::new(), // Empty name for status line
                status: BranchStatus::Uncommitted { count: uncommitted },
            });
        }

        if untracked > 0 {
            branches.push(BranchInfo {
                name: String::new(), // Empty name for status line
                status: BranchStatus::Untracked { count: untracked },
            });
        }
    } else {
        branches.push(BranchInfo {
            name: current_branch_name.clone(),
            status,
        });
    }

    // Get other branches
    if let Ok(refs) = repo.references() {
        if let Ok(local_branches) = refs.local_branches() {
            for branch_ref in local_branches.flatten() {
                let branch_name = branch_ref
                    .name()
                    .as_bstr()
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                let branch_name = branch_name
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&branch_name);

                // Skip current branch as we already added it
                if branch_name == current_branch_name {
                    continue;
                }

                let status = get_branch_tracking_status(repo, branch_name)?;
                branches.push(BranchInfo {
                    name: branch_name.to_string(),
                    status,
                });
            }
        }
    }

    Ok(branches)
}

fn get_branch_tracking_status(repo: &gix::Repository, branch_name: &str) -> Result<BranchStatus> {
    // Use git rev-list to check ahead/behind status
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(
            repo.work_dir()
                .unwrap_or(repo.path())
                .to_string_lossy()
                .as_ref(),
        )
        .arg("rev-list")
        .arg("--left-right")
        .arg("--count")
        .arg(format!("{}...{}@{{u}}", branch_name, branch_name))
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let counts = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = counts.trim().split('\t').collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse::<usize>().unwrap_or(0);
                let behind = parts[1].parse::<usize>().unwrap_or(0);

                match (ahead, behind) {
                    (0, 0) => Ok(BranchStatus::Ok),
                    (a, 0) if a > 0 => Ok(BranchStatus::Ahead(a)),
                    (0, b) if b > 0 => Ok(BranchStatus::Behind(b)),
                    (a, b) => Ok(BranchStatus::Diverged {
                        ahead: a,
                        behind: b,
                    }),
                }
            } else {
                Ok(BranchStatus::Ok)
            }
        }
        _ => Ok(BranchStatus::NoUpstream),
    }
}

fn print_tree(repos: &[RepoStatus], base_dir: &str) {
    println!("{}", base_dir);

    if repos.is_empty() {
        println!("  No git repositories found");
        return;
    }

    // Build tree structure
    let tree = build_tree_structure(repos, base_dir);

    // Print the tree
    print_tree_node(&tree, "", true);
}

#[derive(Debug)]
struct TreeNode {
    name: String,
    children: HashMap<String, TreeNode>,
    repo_status: Option<RepoStatus>,
}

impl TreeNode {
    fn new(name: String) -> Self {
        TreeNode {
            name,
            children: HashMap::new(),
            repo_status: None,
        }
    }
}

fn build_tree_structure(repos: &[RepoStatus], base_dir: &str) -> TreeNode {
    let mut root = TreeNode::new(String::new());

    for repo in repos {
        if let Ok(relative_path) = repo.path.strip_prefix(base_dir) {
            let components: Vec<_> = relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();

            let mut current_node = &mut root;

            // Navigate/create path to the repository
            for (i, component) in components.iter().enumerate() {
                current_node = current_node
                    .children
                    .entry(component.clone())
                    .or_insert_with(|| TreeNode::new(component.clone()));

                // If this is the last component, it's the repository
                if i == components.len() - 1 {
                    current_node.repo_status = Some(RepoStatus {
                        path: repo.path.clone(),
                        current_branch: repo.current_branch.clone(),
                        all_branches: repo.all_branches.clone(),
                        uncommitted_count: repo.uncommitted_count,
                        untracked_count: repo.untracked_count,
                    });
                }
            }
        }
    }

    root
}

fn print_tree_node(node: &TreeNode, prefix: &str, is_last: bool) {
    if !node.name.is_empty() {
        let connector = if is_last { "└── " } else { "├── " };
        print!("{}{}{}", prefix, connector, node.name);

        // If this node is a repository, print its status
        if let Some(ref status) = node.repo_status {
            let mut first_branch = true;

            for branch in status.all_branches.iter() {
                if first_branch && !branch.name.is_empty() {
                    // First branch on same line as repo name
                    print!(" {}", branch.name);
                    print_branch_status(&branch.status);
                    first_branch = false;
                } else if !branch.name.is_empty() {
                    // Other branches on new lines
                    let branch_prefix = if is_last {
                        format!("{}    ", prefix)
                    } else {
                        format!("{}│   ", prefix)
                    };
                    // Calculate spacing based on repo name length
                    let spacing = " ".repeat(20_usize.saturating_sub(node.name.len()));
                    print!("\n{}{}{}", branch_prefix, spacing, branch.name);
                    print_branch_status(&branch.status);
                } else {
                    // Status lines (uncommitted/untracked) without branch name
                    print_branch_status(&branch.status);
                }
            }
        }
        println!();
    }

    // Print children (sorted alphabetically)
    let mut children: Vec<_> = node.children.iter().collect();
    children.sort_by_key(|(name, _)| name.as_str());
    let children_count = children.len();

    for (i, (_, child)) in children.iter().enumerate() {
        let is_last_child = i == children_count - 1;
        let child_prefix = if node.name.is_empty() {
            prefix.to_string()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        print_tree_node(child, &child_prefix, is_last_child);
    }
}

fn print_branch_status(status: &BranchStatus) {
    match status {
        BranchStatus::Ok => print!(" ok"),
        BranchStatus::Ahead(n) => print!(" {} ahead", n),
        BranchStatus::Behind(n) => print!(" {} behind", n),
        BranchStatus::Diverged { ahead, behind } => print!(" {} ahead {} behind", ahead, behind),
        BranchStatus::NoUpstream => print!(" no upstream"),
        BranchStatus::Uncommitted { count } => print!("  [ {} uncommitted ]", count),
        BranchStatus::Untracked { count } => print!("  [ {} untracked ]", count),
    }
}

fn print_flat(repos: &[RepoStatus]) {
    for repo in repos {
        print!("{}", repo.path.display());
        if let Some(branch) = repo
            .all_branches
            .iter()
            .find(|b| b.name == repo.current_branch)
        {
            print!(" ({})", branch.name);
            print_branch_status(&branch.status);
        }
        println!();
    }
}

fn print_dump(repos: &[RepoStatus]) {
    for repo in repos {
        // Try to get the remote URL
        if let Ok(gix_repo) = gix::open(&repo.path) {
            if let Ok(remote) = gix_repo.find_remote("origin") {
                if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                    println!("{} {}", url.to_bstring(), repo.current_branch);
                }
            }
        }
    }
}

