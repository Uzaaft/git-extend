use anyhow::Result;
use clap::Parser;
use git_extend::commands::desc;

#[derive(Parser)]
#[command(name = "git-desc")]
#[command(
    about = "Edit commit messages easily (similar to jj desc)"
)]
#[command(version)]
#[command(after_help = "Examples:
  git desc                          # Edit current commit message
  git desc -m \"New message\"         # Set message directly
  git desc -r HEAD~3                # Edit commit 3 commits back
  git desc -r abc123                # Edit specific commit
  git desc --amend                  # Edit last commit (alias for -r HEAD)")]
struct Cli {
    /// New commit message
    #[arg(short, long)]
    message: Option<String>,

    /// Revision to edit (default: current commit)
    #[arg(short, long)]
    revision: Option<String>,

    /// Edit the last commit (shorthand for -r HEAD)
    #[arg(long)]
    amend: bool,

    /// Skip opening the editor
    #[arg(long)]
    no_edit: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let revision = if cli.amend {
        Some("HEAD".to_string())
    } else {
        cli.revision
    };

    let config = desc::Config {
        message: cli.message,
        revision,
        no_edit: cli.no_edit,
    };

    desc::execute(&config)
}