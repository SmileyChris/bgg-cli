mod auth;
mod bgg;
mod cache;
mod cli;
mod cmd;
mod config;
mod error;
mod model;
mod paths;
mod secrets;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    reset_sigpipe();
    let parsed = Cli::parse();
    init_logging(parsed.verbose);
    let result = match parsed.command {
        Some(Command::Auth { username, clear }) => cmd::auth::run(username, clear),
        Some(Command::Sync { full }) => cmd::sync::run(full),
        Some(Command::List { sort, cols, json }) => cmd::list::run(sort, cols, json),
        None | Some(Command::Status) => cmd::status::run(),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code());
    }
}

/// Restore the default SIGPIPE handler so piping into `head`, `less`, etc.
/// terminates cleanly instead of panicking on the next println.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: setting a signal handler is a one-shot startup operation.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn init_logging(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level))
        .with_target(false)
        .try_init();
}
