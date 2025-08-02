use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub struct Config {
    pub message: Option<String>,
    pub amend: bool,
    pub no_edit: bool,
    pub include_untracked: bool,
}

pub fn execute(config: &Config) -> Result<()> {
    // First check if we're in a git repository
    let status_output = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    if !status_output.status.success() {
        return Err(anyhow::anyhow!("Not in a git repository"));
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    let has_changes = !status_str.trim().is_empty();

    // If no changes and amend flag is set, amend the last commit
    if !has_changes && config.amend {
        println!("No changes detected. Amending last commit...");
        amend_last_commit(config)?;
        return Ok(());
    }

    // If no changes and no amend flag, nothing to do
    if !has_changes {
        println!("No changes to commit. Working directory is clean.");
        return Ok(());
    }

    // Stage all changes
    stage_all_changes(config.include_untracked)?;

    // Create commit
    create_commit(config)?;

    println!("Successfully created new commit. Working directory is now clean.");
    Ok(())
}

fn stage_all_changes(include_untracked: bool) -> Result<()> {
    let mut args = vec!["add"];
    
    if include_untracked {
        args.push("-A");
    } else {
        args.push("-u");
    }

    let output = Command::new("git")
        .args(&args)
        .output()
        .context("Failed to stage changes")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to stage changes: {}", stderr));
    }

    Ok(())
}

fn create_commit(config: &Config) -> Result<()> {
    let mut args = vec!["commit"];

    if let Some(message) = &config.message {
        args.push("-m");
        args.push(message);
    } else if config.no_edit {
        // Use a default message if no editor is wanted
        args.push("-m");
        args.push("Work in progress");
    }

    let output = Command::new("git")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .context("Failed to create commit")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to create commit"));
    }

    Ok(())
}

fn amend_last_commit(config: &Config) -> Result<()> {
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

    Ok(())
}