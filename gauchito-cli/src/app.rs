use std::path::PathBuf;

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use gauchito_core::document::{Document, ViewId};
use gauchito_script::{ScriptRuntime, SharedState};
use gauchito_ui::{EditorState, Effect};
use ratatui::prelude::*;

use gauchito_ui::{Cursor, Pane, PromptOverlay, StatusLine};

pub struct App {
    state: SharedState,
    script: ScriptRuntime,
}

impl App {
    pub fn new(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let mut script = ScriptRuntime::new().map_err(|e| anyhow::anyhow!("script init: {e}"))?;

        // TODO: ugly
        script
            .load_user_config(&gauchito_paths::init_file())
            .map_err(|e| anyhow::anyhow!("script config: {e}"))?;

        let view_id = ViewId::next();
        let initial_mode = script.initial_mode();
        let components = script.component_registry();

        let doc = match path {
            Some(p) => gauchito_core::fileio::load(p)?,
            None => Document::new(),
        };

        let doc_id = doc.id;
        let mut state = EditorState::new(view_id, initial_mode, components);

        state.add_document(doc);
        state.add_view(view_id, doc_id);

        let state = gauchito_script::shared(state);
        script.run_initial_mode_callback(&state);

        Ok(App { state, script })
    }

    pub async fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
        let mut event_stream = EventStream::new();

        loop {
            terminal.draw(|f| {
                let mut state = self.state.borrow_mut();

                // TODO: structure this so we dont have to check for each ui widget
                let has_statusline = state.components.has_statusline();
                let statusline_height = if has_statusline { 1 } else { 0 };

                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(statusline_height)])
                        .split(f.area());

                Pane::render(f, chunks[0], &mut state);
                Cursor::apply_style(&state);

                if has_statusline {
                    StatusLine::render(f, chunks[1], &state);
                }

                PromptOverlay::render(f, f.area(), &state);
            })?;

            tokio::select! {
                event = event_stream.next() => {
                    if let Some(Ok(Event::Key(key))) = event {
                        if key.kind == KeyEventKind::Press {
                            let (key_name, is_printable) = key_event_to_name(key.code, key.modifiers);

                            let effects = self.script.dispatch_key(
                                &key_name,
                                is_printable,
                                &self.state,
                            );

                            if self.process_effects(effects)? {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    /// Process all effects produced by a command. Returns true if the app should quit.
    fn process_effects(&mut self, effects: Vec<Effect>) -> anyhow::Result<bool> {
        for effect in effects {
            match effect {
                Effect::Quit => return Ok(true),
                Effect::Edit(_, _) => {}
                Effect::OpenFile(path) => {
                    let doc = gauchito_core::fileio::load(path)?;
                    self.state.borrow_mut().open_document(doc);
                }
                Effect::CloseView => {
                    if self.state.borrow_mut().close_view() {
                        return Ok(true);
                    }
                }
                Effect::SplitFocused(direction) => {
                    self.state.borrow_mut().split_focused(direction);
                }
                Effect::FocusNext => self.state.borrow_mut().focus_next(),
                Effect::FocusPrev => self.state.borrow_mut().focus_prev(),
                Effect::SwitchBuffer(id) => self.state.borrow_mut().switch_to_document(id),
            }
        }
        Ok(false)
    }
}

// TODO: move this to its own file or crate
/// Convert a crossterm key event into a string name for Lua dispatch and an
/// optional printable character.
fn key_event_to_name(code: KeyCode, modifiers: KeyModifiers) -> (String, Option<char>) {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);

    let (base, printable) = match code {
        KeyCode::Char(ch) => {
            let name = match ch {
                ' ' => "space".to_string(),
                _ => ch.to_string(),
            };
            let printable = if ctrl || alt { None } else { Some(ch) };
            (name, printable)
        }
        KeyCode::Enter => ("enter".to_string(), None),
        KeyCode::Esc => ("esc".to_string(), None),
        KeyCode::Backspace => ("backspace".to_string(), None),
        KeyCode::Delete => ("del".to_string(), None),
        KeyCode::Tab => ("tab".to_string(), None),
        KeyCode::Left => ("left".to_string(), None),
        KeyCode::Right => ("right".to_string(), None),
        KeyCode::Up => ("up".to_string(), None),
        KeyCode::Down => ("down".to_string(), None),
        KeyCode::Home => ("home".to_string(), None),
        KeyCode::End => ("end".to_string(), None),
        KeyCode::PageUp => ("pageup".to_string(), None),
        KeyCode::PageDown => ("pagedown".to_string(), None),
        KeyCode::F(n) => (format!("f{n}"), None),
        _ => ("unknown".to_string(), None),
    };

    let name = if ctrl && alt {
        format!("ctrl-alt-{base}")
    } else if ctrl {
        format!("ctrl-{base}")
    } else if alt {
        format!("alt-{base}")
    } else {
        base
    };

    (name, printable)
}

pub async fn run(path: Option<PathBuf>) -> anyhow::Result<()> {
    let mut app = App::new(path)?;
    let mut terminal = ratatui::init();

    let result = app.run(&mut terminal).await;

    ratatui::restore();

    result
}
