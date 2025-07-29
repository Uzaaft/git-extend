use anyhow::Result;
use clap::Parser;
use git_extend::commands::get;
use std::env;

#[derive(Parser)]
#[command(name = "git-get")]
#[command(
    about = "Clone git repository into an automatically created directory tree based on the repo's URL."
)]
#[command(version)]
#[command(after_help = "Examples:
  git get grdl/git-get
  git get https://github.com/grdl/git-get.git
  git get git@github.com:grdl/git-get.git
  git get -d path/to/dump/file")]
struct Cli {
    /// Repository to clone
    #[arg(value_name = "REPO")]
    repo: Option<String>,

    /// Branch (or tag) to checkout after cloning
    #[arg(short, long)]
    branch: Option<String>,

    /// Path to a dump file listing repos to clone. Ignored when <REPO> argument is used
    #[arg(short, long)]
    dump: Option<String>,

    /// Host to use when <REPO> doesn't have a specified host
    #[arg(short = 't', long, default_value = "github.com")]
    host: String,

    /// Path to repos root where repositories are cloned
    #[arg(short, long, default_value = "~/repositories")]
    root: String,

    /// Scheme to use when <REPO> doesn't have a specified scheme
    #[arg(short = 'c', long, default_value = "ssh")]
    scheme: String,

    /// Don't create a directory for host
    #[arg(short, long)]
    skip_host: bool,
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return path.replacen("~", &home, 1);
        }
    }
    path.to_string()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let base_dir = if let Ok(git_path) = env::var("GIT_PATH") {
        git_path
    } else {
        expand_tilde(&cli.root)
    };

    let config = get::Config {
        base_dir,
        branch: cli.branch,
        default_host: cli.host,
        default_scheme: cli.scheme,
        skip_host: cli.skip_host,
    };

    if let Some(dump_file) = cli.dump {
        get::execute_dump(&dump_file, &config)
    } else if let Some(repo) = cli.repo {
        get::execute(&repo, &config)
    } else {
        eprintln!("Error: Either provide a repository URL or use -d flag with a dump file");
        std::process::exit(1);
    }
}

