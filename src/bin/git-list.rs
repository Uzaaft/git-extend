use anyhow::Result;
use clap::Parser;
use git_extend::{commands, get_base_dir};

#[derive(Parser)]
#[command(name = "git-list")]
#[command(about = "List all git repositories and their status")]
struct Cli {
    /// Output format: tree, flat, or dump
    #[arg(short, long, default_value = "tree")]
    output: String,

    /// Root directory to search for repositories (defaults to $GIT_PATH)
    #[arg(short, long)]
    dir: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base_dir = get_base_dir(cli.dir)?;
    commands::list::execute(&cli.output, &base_dir)
}

