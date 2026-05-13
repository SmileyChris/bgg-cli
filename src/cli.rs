use clap::{Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ListSort {
    /// By name (default), case-insensitive.
    Name,
    /// By year published ascending, then name. Items without a year sort last.
    Year,
    /// By BGG object id ascending, then name.
    Bggid,
}

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
    ///
    /// On a terminal: prints a table of owned base games.
    /// Piped or redirected: prints the full collection as JSON.
    List {
        /// Sort order (table view only). Name is always the tie-breaker.
        #[arg(long, value_enum, default_value_t = ListSort::Name)]
        sort: ListSort,
    },
    /// Show auth state, cached username, item count, and last sync time.
    Status,
}
