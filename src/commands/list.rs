use anyhow::Result;
use gix::bstr::ByteSlice;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Debug, Clone)]
struct RepoStatus {
    path: PathBuf,
    current_branch: String,
    all_branches: Vec<BranchInfo>,
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
    if git_dir.exists() {
        // Quick check if it's a directory (not a submodule file)
        if git_dir.is_dir() {
            if let Ok(status) = get_repo_status(dir) {
                repos.push(status);
            }
        }
        return Ok(());
    }

    // Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();

            // Skip based on name first (no file system call needed)
            if let Some(name_str) = name.to_str() {
                // Skip hidden directories (except .git which is handled above)
                if name_str.starts_with('.') {
                    continue;
                }

                // Skip common non-repository directories
                match name_str {
                    "node_modules" | "target" | "build" | "dist" | "out" | "__pycache__"
                    | ".cache" | "vendor" | "bin" | "obj" => continue,
                    _ => {}
                }
            }

            // Only check file type after name checks pass
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

    // Get current branch using simpler API
    let current_branch = repo
        .head_name()?
        .map(|name| {
            name.as_bstr()
                .to_str()
                .unwrap_or("HEAD")
                .strip_prefix("refs/heads/")
                .unwrap_or("HEAD")
                .to_string()
        })
        .unwrap_or_else(|| "HEAD".to_string());

    // Count uncommitted and untracked files
    let (uncommitted_count, untracked_count) = count_changes(&repo)?;

    // Get all branches with their status
    let all_branches = get_all_branches(&repo, uncommitted_count, untracked_count)?;

    Ok(RepoStatus {
        path: repo_path.to_path_buf(),
        current_branch,
        all_branches,
    })
}

fn count_changes(repo: &gix::Repository) -> Result<(usize, usize)> {
    let mut uncommitted = 0;
    let mut untracked = 0;

    let work_dir = repo.workdir().unwrap_or(repo.path());
    
    // Use regular porcelain format which is simpler to parse
    if let Ok(output) = std::process::Command::new("git")
        .args(&[
            "-C",
            work_dir.to_string_lossy().as_ref(),
            "status",
            "--porcelain",
        ])
        .output()
    {
        // Process output line by line
        for line in output.stdout.split(|&b| b == b'\n') {
            if line.len() >= 2 {
                if line[0] == b'?' && line[1] == b'?' {
                    untracked += 1;
                } else if line[0] != b' ' || line[1] != b' ' {
                    uncommitted += 1;
                }
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

    // Get current branch name (already extracted in get_repo_status)
    let current_branch_name = repo
        .head_name()?
        .map(|name| {
            name.as_bstr()
                .to_str()
                .unwrap_or("HEAD")
                .strip_prefix("refs/heads/")
                .unwrap_or("HEAD")
                .to_string()
        })
        .unwrap_or_else(|| "HEAD".to_string());

    // Get all branches using gix native API
    let mut branch_statuses = HashMap::new();
    
    // Try native gix approach first, fall back to git command if needed
    if let Ok(refs) = repo.references() {
        if let Ok(branches) = refs.local_branches() {
            for branch in branches.flatten() {
                if let Some((category, short_name)) = branch.name().category_and_short_name() {
                    if matches!(category, gix::reference::Category::LocalBranch) {
                        // For now, we still need git for tracking info
                        branch_statuses.insert(short_name.to_string(), BranchStatus::Ok);
                    }
                }
            }
        }
    }
    
    // If we got branches natively, get their tracking status via git
    if !branch_statuses.is_empty() {
        let work_dir = repo.workdir().unwrap_or(repo.path());
        if let Ok(output) = std::process::Command::new("git")
            .args(&[
                "-C",
                work_dir.to_string_lossy().as_ref(),
                "for-each-ref",
                "--format=%(refname:short) %(upstream:track)",
                "refs/heads",
            ])
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let mut parts_iter = line.split_whitespace();
                let branch_name = match parts_iter.next() {
                    Some(name) => name,
                    None => continue,
                };

                let status = match (
                    parts_iter.next(),
                    parts_iter.next(),
                    parts_iter.next(),
                    parts_iter.next(),
                ) {
                    (None, _, _, _) => BranchStatus::Ok,
                    (Some("[ahead"), Some(count_str), Some("behind"), Some(behind_str)) => {
                        let ahead = count_str
                            .trim_end_matches(',')
                            .parse::<usize>()
                            .unwrap_or(0);
                        let behind = behind_str
                            .trim_end_matches(']')
                            .parse::<usize>()
                            .unwrap_or(0);
                        BranchStatus::Diverged { ahead, behind }
                    }
                    (Some("[ahead"), Some(count_str), _, _) => {
                        let count = count_str
                            .trim_end_matches(']')
                            .parse::<usize>()
                            .unwrap_or(0);
                        BranchStatus::Ahead(count)
                    }
                    (Some("[behind"), Some(count_str), _, _) => {
                        let count = count_str
                            .trim_end_matches(']')
                            .parse::<usize>()
                            .unwrap_or(0);
                        BranchStatus::Behind(count)
                    }
                    _ => BranchStatus::NoUpstream,
                };

                // Update the status we got from native API
                if branch_statuses.contains_key(branch_name) {
                    branch_statuses.insert(branch_name.to_string(), status);
                }
            }
        }
    }

    // Add current branch
    let current_status = branch_statuses
        .get(&current_branch_name)
        .cloned()
        .unwrap_or(BranchStatus::NoUpstream);

    if uncommitted > 0 || untracked > 0 {
        branches.push(BranchInfo {
            name: current_branch_name.clone(),
            status: current_status,
        });

        if uncommitted > 0 {
            branches.push(BranchInfo {
                name: String::new(),
                status: BranchStatus::Uncommitted { count: uncommitted },
            });
        }

        if untracked > 0 {
            branches.push(BranchInfo {
                name: String::new(),
                status: BranchStatus::Untracked { count: untracked },
            });
        }
    } else {
        branches.push(BranchInfo {
            name: current_branch_name.clone(),
            status: current_status,
        });
    }

    // Add other branches
    for (branch_name, status) in branch_statuses {
        if branch_name != current_branch_name {
            branches.push(BranchInfo {
                name: branch_name,
                status,
            });
        }
    }

    Ok(branches)
}

fn print_tree(repos: &[RepoStatus], base_dir: &str) {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    
    writeln!(stdout, "{}", base_dir).unwrap();

    if repos.is_empty() {
        writeln!(stdout, "  No git repositories found").unwrap();
        return;
    }

    // Build tree structure
    let tree = build_tree_structure(repos, base_dir);

    // Print the tree
    print_tree_node(&tree, "", true, &mut stdout);
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
            let components: Vec<_> = relative_path.components().collect();
            let components_len = components.len();

            let mut current_node = &mut root;

            // Navigate/create path to the repository
            for (i, component) in components.iter().enumerate() {
                let component_str = component.as_os_str().to_string_lossy();
                current_node = current_node
                    .children
                    .entry(component_str.to_string())
                    .or_insert_with(|| TreeNode::new(component_str.to_string()));

                // If this is the last component, it's the repository
                if i == components_len - 1 {
                    current_node.repo_status = Some(repo.clone());
                }
            }
        }
    }

    root
}

fn print_tree_node(node: &TreeNode, prefix: &str, is_last: bool, out: &mut StandardStream) {
    if !node.name.is_empty() {
        let connector = if is_last { "└── " } else { "├── " };
        write!(out, "{}{}{}", prefix, connector, node.name).unwrap();

        // If this node is a repository, print its status
        if let Some(ref status) = node.repo_status {
            let mut first_branch = true;

            for branch in status.all_branches.iter() {
                if first_branch && !branch.name.is_empty() {
                    // First branch on same line as repo name
                    write!(out, " {}", branch.name).unwrap();
                    print_branch_status(&branch.status, out);
                    first_branch = false;
                } else if !branch.name.is_empty() {
                    // Other branches on new lines
                    write!(out, "\n{}", prefix).unwrap();
                    write!(out, "{}", if is_last { "    " } else { "│   " }).unwrap();
                    // Calculate spacing based on repo name length
                    let spacing_needed = 20_usize.saturating_sub(node.name.len());
                    for _ in 0..spacing_needed {
                        write!(out, " ").unwrap();
                    }
                    write!(out, "{}", branch.name).unwrap();
                    print_branch_status(&branch.status, out);
                } else {
                    // Status lines (uncommitted/untracked) without branch name
                    print_branch_status(&branch.status, out);
                }
            }
        }
        writeln!(out).unwrap();
    }

    // Print children (sorted alphabetically)
    let mut children: Vec<_> = node.children.iter().collect();
    children.sort_by_key(|(name, _)| name.as_str());
    
    let children_count = children.len();
    let mut child_prefix = String::with_capacity(prefix.len() + 4);
    
    for (i, (_, child)) in children.iter().enumerate() {
        let is_last_child = i == children_count - 1;
        
        // Reuse the string buffer
        child_prefix.clear();
        child_prefix.push_str(prefix);
        if !node.name.is_empty() {
            child_prefix.push_str(if is_last { "    " } else { "│   " });
        }

        print_tree_node(child, &child_prefix, is_last_child, out);
    }
}

fn print_branch_status(status: &BranchStatus, out: &mut StandardStream) {
    match status {
        BranchStatus::Ok => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
            write!(out, " ok").unwrap();
            out.reset().unwrap();
        }
        BranchStatus::Ahead(n) => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
            write!(out, " {} ahead", n).unwrap();
            out.reset().unwrap();
        }
        BranchStatus::Behind(n) => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
            write!(out, " {} behind", n).unwrap();
            out.reset().unwrap();
        }
        BranchStatus::Diverged { ahead, behind } => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
            write!(out, " {} ahead {} behind", ahead, behind).unwrap();
            out.reset().unwrap();
        }
        BranchStatus::NoUpstream => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
            write!(out, " no upstream").unwrap();
            out.reset().unwrap();
        }
        BranchStatus::Uncommitted { count } => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
            write!(out, "  [ {} uncommitted ]", count).unwrap();
            out.reset().unwrap();
        }
        BranchStatus::Untracked { count } => {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
            write!(out, "  [ {} untracked ]", count).unwrap();
            out.reset().unwrap();
        }
    }
}

fn print_flat(repos: &[RepoStatus]) {
    let mut out = StandardStream::stdout(ColorChoice::Always);
    
    for repo in repos {
        write!(out, "{}", repo.path.display()).unwrap();
        if let Some(branch) = repo
            .all_branches
            .iter()
            .find(|b| b.name == repo.current_branch)
        {
            write!(out, " ({})", branch.name).unwrap();
            print_branch_status(&branch.status, &mut out);
        }
        writeln!(out).unwrap();
    }
}

fn print_dump(repos: &[RepoStatus]) {
    let mut out = StandardStream::stdout(ColorChoice::Always);
    
    for repo in repos {
        // Try to get the remote URL
        if let Ok(gix_repo) = gix::open(&repo.path) {
            if let Ok(remote) = gix_repo.find_remote("origin") {
                if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                    writeln!(out, "{} {}", url.to_bstring(), repo.current_branch).unwrap();
                }
            }
        }
    }
}
