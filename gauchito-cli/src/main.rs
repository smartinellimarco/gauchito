//! Gauchito CLI entry point.
//!
//! Single-thread tokio runtime (`new_current_thread`) — the
//! substrate promises one Lua state on the main thread, and Lua
//! coroutines never cross threads. Running tokio on the same thread
//! keeps that promise cheap: no `LocalSet`, no worker pool. Async
//! producers wake on mpsc; the main thread resumes coroutines.

mod app;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    if let Err(e) = rt.block_on(app::run(argv)) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
