use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Sync a BoardGameGeek user's collection to a local cache."
)]
pub struct Cli {
    /// Verbose logging (-v info, -vv debug, -vvv trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Subcommand to run. If omitted, runs `status`.
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Authenticate with BGG and store cookies in the OS keyring.
    /// Use `--clear` to remove stored cookies instead.
    Auth {
        /// Username (defaults to the value in config.toml if set).
        username: Option<String>,
        /// Clear stored cookies for the current user instead of logging in.
        #[arg(long)]
        clear: bool,
    },
    /// Sync the collection. By default, incremental via `modifiedsince`.
    Sync {
        /// Ignore modifiedsince and pull the whole collection. Required to detect deletions.
        #[arg(long)]
        full: bool,
    },
    /// List cached collection items.
    List {
        /// Only owned items.
        #[arg(long)]
        owned: bool,
        /// JSON output instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Show auth state, cached username, item count, and last sync time.
    Status,
}
