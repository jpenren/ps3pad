use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Self-contained Sony DualShock 3 / SIXAXIS driver and tester.
#[derive(Debug, Parser)]
#[command(
    name = "ps3pad",
    version,
    about,
    long_about = "Read a DualShock 3 over USB or Bluetooth, decode every report, and expose it as a standard HID gamepad through the operating system's virtual HID interface.",
    propagate_version = true
)]
pub struct Cli {
    /// Increase log verbosity (repeat for more detail).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List supported controllers visible to the operating system.
    List {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Print live input reports from a controller.
    Dump(DumpArgs),

    /// Check driver, permission and virtual-HID prerequisites.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Bridge a physical controller to a virtual HID gamepad.
    Daemon(DaemonArgs),

    /// Open the graphical tester.
    #[cfg(feature = "gui")]
    Gui(GuiArgs),

    /// Create a virtual gamepad and exercise it without a physical controller.
    #[command(name = "uhid-test")]
    UhidTest {
        /// How long to keep the virtual device alive.
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },

    /// Set the player LED without starting the bridge.
    Led {
        /// Player number, 1 through 4.
        #[arg(default_value_t = 1)]
        player: u8,

        /// Device index or path.
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Store the host Bluetooth address in a USB-connected controller.
    Pair {
        /// Host address, for example aa:bb:cc:dd:ee:ff.
        host_mac: String,

        /// Device index or path.
        #[arg(short, long)]
        device: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct DumpArgs {
    /// Device index (from `ps3pad list`) or hidraw path.
    #[arg(short, long)]
    pub device: Option<String>,

    /// Print the raw report bytes instead of the decoded state.
    #[arg(long)]
    pub raw: bool,

    /// Emit one JSON object per report.
    #[arg(long)]
    pub json: bool,

    /// Stop after this many seconds. Zero means run forever.
    #[arg(long, default_value_t = 0)]
    pub seconds: u64,

    /// Do not send the operational-mode handshake.
    #[arg(long)]
    pub no_init: bool,
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    /// Device index (from `ps3pad list`) or hidraw path.
    #[arg(short, long)]
    pub device: Option<String>,

    /// Unix socket used for status and control commands. Defaults to
    /// $XDG_RUNTIME_DIR/ps3pad/control.sock.
    #[arg(long, default_value = "")]
    pub socket: PathBuf,

    /// Do not create a virtual gamepad; decode reports only.
    #[arg(long)]
    pub no_virtual: bool,

    /// Player LED to use when the host does not request one.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub player: u8,

    /// Forward virtual-device rumble to the controller.
    #[arg(long, default_value_t = true)]
    pub rumble: bool,
}

#[cfg(feature = "gui")]
#[derive(Debug, Args)]
pub struct GuiArgs {
    /// Device index (from `ps3pad list`) or hidraw path.
    #[arg(short, long)]
    pub device: Option<String>,

    /// Create a virtual gamepad automatically after connecting.
    #[arg(long)]
    pub virtual_pad: bool,
}
