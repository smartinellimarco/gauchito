use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("gauchito");
    }

    // macOS CLI users usually prefer ~/.config over Application Support.
    if let Some(home) = dirs::home_dir() {
        let dot_config = home.join(".config").join("gauchito");
        if dot_config.exists() {
            return dot_config;
        }
    }

    dirs::config_dir()
        .expect("no config directory")
        .join("gauchito")
}

pub fn init_file() -> PathBuf {
    config_dir().join("init.lua")
}
