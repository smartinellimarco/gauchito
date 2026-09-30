use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use script::ScriptRuntime;

// One thread: Lua is not Send, so every task runs on this LocalSet.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let argv = std::env::args().skip(1).collect();
    let result = tokio::task::LocalSet::new().run_until(run(argv)).await;
    if let Err(e) = result {
        eprintln!("gau: {e}");
        std::process::exit(1);
    }
}

async fn run(argv: Vec<String>) -> anyhow::Result<()> {
    let script = ScriptRuntime::new(argv)?;
    script.load_config(&script::user_config_path())?;

    // Let the config's first task run, so a config that only prints and quits never opens the terminal.
    tokio::task::yield_now().await;
    if script.done() {
        return check(&script);
    }

    let mut terminal = ratatui::init();
    let result = event_loop(&script, &mut terminal).await;
    ratatui::restore();
    result
}

async fn event_loop(
    script: &ScriptRuntime,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let mut events = EventStream::new();
    loop {
        check(script)?;
        if script.done() {
            return Ok(());
        }

        let frame = script.frame();
        terminal.draw(|f| frame.borrow().flush(f))?;

        tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => script.feed_key(key),
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
                _ => {}
            },
            _ = script.woken() => {}
        }
    }
}

fn check(script: &ScriptRuntime) -> anyhow::Result<()> {
    match script.error() {
        Some(e) => Err(anyhow::anyhow!(e)),
        None => Ok(()),
    }
}
