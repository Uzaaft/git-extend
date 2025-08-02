use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::io::Write;
use std::env;

pub struct Config {
    pub message: Option<String>,
    pub revision: Option<String>,
    pub no_edit: bool,
}

pub fn execute(config: &Config) -> Result<()> {
    // Check if we're in a git repository
    let output = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()
        .context("Failed to check git repository")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Not in a git repository"));
    }

    let revision = config.revision.as_deref().unwrap_or("HEAD");
    
    // Check if the revision exists
    let output = Command::new("git")
        .args(&["rev-parse", revision])
        .output()
        .context("Failed to parse revision")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Invalid revision: {}", revision));
    }

    let commit_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Handle the case where we're editing HEAD
    if revision == "HEAD" {
        edit_head_commit(config)?;
    } else {
        // For non-HEAD commits, we need to use interactive rebase
        edit_past_commit(&commit_sha, config)?;
    }

    Ok(())
}

fn edit_head_commit(config: &Config) -> Result<()> {
    let mut args = vec!["commit", "--amend"];

    if config.no_edit {
        args.push("--no-edit");
    } else if let Some(message) = &config.message {
        args.push("-m");
        args.push(message);
    }

    let output = Command::new("git")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .context("Failed to amend commit")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to amend commit"));
    }

    println!("Successfully updated commit message");
    Ok(())
}

fn edit_past_commit(commit_sha: &str, config: &Config) -> Result<()> {
    // Get the parent of the commit we want to edit
    let output = Command::new("git")
        .args(&["rev-parse", &format!("{}^", commit_sha)])
        .output()
        .context("Failed to get parent commit")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Cannot edit root commit with this method"));
    }

    let parent_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Create a script for interactive rebase
    let rebase_script = if let Some(message) = &config.message {
        create_rebase_script_with_message(commit_sha, message)?
    } else if config.no_edit {
        return Err(anyhow::anyhow!("Cannot use --no-edit when editing past commits without a message"));
    } else {
        create_rebase_script_interactive(commit_sha)?
    };

    // Set up environment for the rebase
    let editor_command = format!("echo '{}' >", rebase_script);
    
    let output = Command::new("sh")
        .arg("-c")
        .arg(&format!("GIT_SEQUENCE_EDITOR='{}' git rebase -i {}", editor_command, parent_sha))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .context("Failed to run interactive rebase")?;

    if !output.status.success() {
        // Try to abort the rebase if it failed
        let _ = Command::new("git")
            .args(&["rebase", "--abort"])
            .output();
        return Err(anyhow::anyhow!("Failed to complete rebase"));
    }

    println!("Successfully updated commit message");
    Ok(())
}

fn create_rebase_script_with_message(commit_sha: &str, message: &str) -> Result<String> {
    // Create a temporary file with the new commit message
    let temp_dir = env::temp_dir();
    let msg_file = temp_dir.join(format!("git-desc-msg-{}.txt", std::process::id()));
    
    let mut file = std::fs::File::create(&msg_file)
        .context("Failed to create temporary message file")?;
    
    writeln!(file, "{}", message)?;
    
    // The script will mark the commit for edit and use our message file
    Ok(format!(
        "sed -i '' 's/pick {}/edit {}/' $1 && git commit --amend -F {} --no-edit",
        &commit_sha[..7],
        &commit_sha[..7],
        msg_file.display()
    ))
}

fn create_rebase_script_interactive(commit_sha: &str) -> Result<String> {
    // The script will just mark the commit for edit
    Ok(format!(
        "sed -i '' 's/pick {}/edit {}/' $1",
        &commit_sha[..7],
        &commit_sha[..7]
    ))
}