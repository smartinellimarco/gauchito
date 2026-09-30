//! App driver — config load, terminal init/restore boundary and the
//! event loop.
//!
//! The config runs before the terminal enters raw mode, so a config
//! that only prints and quits (`--version`, `--health`) never touches
//! the terminal. The loop itself is the RFC §4 diagram literally:
//! paint the current frame, drain key events into the script runtime,
//! drain task wakes from the script's mpsc — `tokio::select!`
//! resolves whichever fires first.

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use gauchito_script::{ScriptRuntime, TaskWakeRx};

pub async fn run(argv: Vec<String>) -> anyhow::Result<()> {
    let (mut script, task_rx) = ScriptRuntime::new(argv).unwrap();
    script
        .load_config(&gauchito_script::user_config_path())
        .map_err(|e| anyhow::anyhow!("config: {e}"))?;

    if script.should_quit() {
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let result = main_loop(script, task_rx, &mut terminal).await;
    ratatui::restore();
    result
}

async fn main_loop(
    mut script: ScriptRuntime,
    mut task_rx: TaskWakeRx,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let mut events = EventStream::new();
    loop {
        let frame = script.frame();
        terminal.draw(|f| frame.borrow().flush(f))?;

        if script.should_quit() {
            return Ok(());
        }

        tokio::select! {
            event = events.next() => {
                if let Some(Ok(Event::Key(key))) = event {
                    if key.kind == KeyEventKind::Press {
                        script.feed_key(key)?;
                    }
                }
            }
            wake = task_rx.recv() => {
                if let Some(wake) = wake {
                    script.resume_task(wake)?;
                }
            }
        }
    }
}
