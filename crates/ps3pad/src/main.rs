mod cli;
mod commands;
mod daemon;
#[cfg(feature = "gui")]
mod gui;
#[cfg(unix)]
mod socket;
mod logger;
mod util;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    #[cfg(windows)]
    logger::init(level);
    #[cfg(not(windows))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    match cli.command {
        Command::List { json } => commands::list(json),
        Command::Dump(args) => commands::dump(args),
        Command::Doctor { json } => commands::doctor(json),
        Command::Daemon(args) => daemon::run(args),
        #[cfg(feature = "gui")]
        Command::Gui(args) => gui::run(args),
        Command::UhidTest { seconds } => commands::uhid_test(seconds),
        Command::Led { player, device } => commands::led(player, device.as_deref()),
        Command::Pair { host_mac, device } => commands::pair(&host_mac, device.as_deref()),
    }
}
