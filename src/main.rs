mod auth;
mod bgg;
mod cache;
mod cli;
mod config;
mod error;
mod model;
mod paths;
mod secrets;

use clap::Parser;

fn main() {
    let _cli = cli::Cli::parse();
    eprintln!("dispatch not yet wired");
    std::process::exit(1);
}
