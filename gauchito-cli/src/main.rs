mod app;

use std::path::PathBuf;

use clap::Parser;

// TODO: make these flags configurable from the lua config script (except verbosity)
#[derive(Parser)]
#[command(name = "gauchito")]
struct Args {
    file: Option<PathBuf>,

    /// Increase log verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = setup_logging(args.verbose) {
        eprintln!("logging setup failed: {e}");
    }

    tracing::debug!("gauchito starting, verbosity={}", args.verbose);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    if let Err(e) = rt.block_on(app::run(args.file)) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn setup_logging(verbosity: u8) -> anyhow::Result<()> {
    use tracing_subscriber::EnvFilter;

    // TODO: do not run our crates always in debug. Respect the flag
    // Always log gauchito crates at debug level so Lua/Rust errors are captured.
    // Verbosity flag controls third-party noise.
    let filter = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let log_path = log_file();

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_writer(file)
        .with_ansi(false)
        .init();

    Ok(())
}

// TODO: move to paths crate. Also create a command for opening the logfile in the lua config
fn log_file() -> PathBuf {
    let dir = gauchito_paths::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("gauchito.log")
}
