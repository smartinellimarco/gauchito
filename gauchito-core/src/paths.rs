//! Filesystem location of the distro config.
//!
//! Config resolution order is deliberate:
//!   1. `XDG_CONFIG_HOME` if set — an explicit user override always
//!      wins.
//!   2. `~/.config/gauchito` if it already exists — common on macOS,
//!      where CLI users routinely prefer the Linux convention over
//!      `~/Library/Application Support`.
//!   3. Platform default via `dirs::config_dir()`.

use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    // 1. Explicit XDG_CONFIG_HOME
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("gauchito");
    }

    // 2. ~/.config/gauchito if it already exists (common for CLI tools on macOS)
    if let Some(home) = dirs::home_dir() {
        let dot_config = home.join(".config").join("gauchito");
        if dot_config.exists() {
            return dot_config;
        }
    }

    // 3. Platform default (~/Library/Application Support on macOS, ~/.config on Linux)
    dirs::config_dir()
        .expect("no config directory")
        .join("gauchito")
}

pub fn init_file() -> PathBuf {
    config_dir().join("init.lua")
}
