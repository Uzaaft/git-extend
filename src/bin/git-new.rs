use anyhow::Result;
use clap::Parser;
use git_extend::commands::new;

#[derive(Parser)]
#[command(name = "git-new")]
#[command(
    about = "Create a new commit with current changes and start fresh work (similar to jj new)"
)]
#[command(version)]
#[command(after_help = "Examples:
  git new                           # Commit all changes and start fresh
  git new -m \"Fix bug #123\"         # Commit with a specific message
  git new --amend                   # Amend the last commit if no changes
  git new --no-edit                 # Commit without opening editor")]
struct Cli {
    /// Commit message
    #[arg(short, long)]
    message: Option<String>,

    /// Amend the last commit if there are no changes
    #[arg(long)]
    amend: bool,

    /// Skip opening the editor for commit message
    #[arg(long)]
    no_edit: bool,

    /// Include untracked files in the commit
    #[arg(short, long)]
    untracked: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = new::Config {
        message: cli.message,
        amend: cli.amend,
        no_edit: cli.no_edit,
        include_untracked: cli.untracked,
    };

    new::execute(&config)
}