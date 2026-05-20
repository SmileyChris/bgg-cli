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

    /// Subcommand to run. If omitted, prints a one-screen summary of the
    /// local cache (auth state, item count, top-line stats).
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
    /// List cached collection items as a table. By default shows owned base
    /// games (no expansions).
    ///
    /// --filter accepts: owned, prev-owned, wishlist, want-to-play, want-to-buy,
    /// preordered, for-trade, expansion, rated, played, solo, all.
    /// Comma-separated; prefix `not:` to invert. Filters AND together. Defaults
    /// to `owned,not:expansion`. Use `--filter all` to see everything.
    ///
    /// --sort accepts: name, year, bggid, plays, rating, time, added, geek, players.
    /// Each has a natural direction (e.g. plays desc, time asc); append `:asc`
    /// or `:desc` to override (e.g. `--sort time:desc` for longest first).
    ///
    /// --cols accepts: year, name, bggid, plays, rating, time, players, geek,
    /// or `all`. Defaults to `year,name`. When --cols is not provided, the
    /// field used by --sort is added implicitly if it has a column.
    ///
    /// --json prints the full unfiltered collection as JSON for piping into jq
    /// (ignores --filter).
    List {
        /// Filters to narrow the table view.
        #[arg(long, default_value = "owned,not:expansion")]
        filter: String,
        /// Sort order (table view only).
        #[arg(long, default_value = "name")]
        sort: String,
        /// Columns to show, comma-separated, or `all`.
        #[arg(long)]
        cols: Option<String>,
        /// Cap the table to the first N rows after filtering and sorting.
        #[arg(long)]
        limit: Option<usize>,
        /// Emit the full collection as JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Summarize the cached collection: counts, plays, ratings, year, time, players.
    ///
    /// Owned-game stats (plays, ratings, year, time, players) restrict to the
    /// `boardgame` subtype, matching `bgg list`. Status counts (wishlist, etc.)
    /// include all subtypes.
    ///
    /// Run without a subcommand for the full overview. Pick a subcommand for a
    /// deep dive into one area.
    Stats {
        /// Emit stats as JSON instead of a text report.
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        section: Option<StatsCommand>,
    },
}

#[derive(Subcommand)]
pub enum StatsCommand {
    /// Deep dive: full play-count ranking, H-index, dimes/nickels/quarters, histogram.
    Plays {
        /// Emit as JSON instead of a text report.
        #[arg(long)]
        json: bool,
    },
    /// Deep dive: rating distribution bar chart, comparison with BGG, biggest deltas.
    Ratings {
        #[arg(long)]
        json: bool,
    },
    /// Deep dive: year published distribution, decade summary, full bar chart.
    Year {
        #[arg(long)]
        json: bool,
    },
    /// Deep dive: playing time distribution, per-bucket game lists, full sorted list.
    Time {
        #[arg(long)]
        json: bool,
    },
    /// Deep dive: player-count matrix, best-at-each-count, exclusives.
    Players {
        #[arg(long)]
        json: bool,
    },
}
