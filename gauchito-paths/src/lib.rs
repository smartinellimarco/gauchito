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

pub fn data_dir() -> PathBuf {
    dirs::data_local_dir() //TODO: respects XDG?
        .expect("no data-local directory")
        .join("gauchito")
}

pub fn log_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("no cache directory")
        .join("gauchito")
}
