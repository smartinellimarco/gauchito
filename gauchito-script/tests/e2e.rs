use std::path::PathBuf;
use std::time::Duration;

use gauchito_script::{KeyCode, KeyEvent, KeyModifiers, ScriptRuntime, TaskWakeRx};

struct Editor {
    rt: ScriptRuntime,
    rx: TaskWakeRx,
}

impl Editor {
    async fn start(argv: &[&str]) -> Self {
        let (mut rt, rx) = ScriptRuntime::new(argv.iter().map(|s| s.to_string()).collect()).unwrap();
        rt.load_config(&PathBuf::from("/nonexistent/init.lua")).unwrap();
        let mut editor = Editor { rt, rx };
        editor.settle().await;
        editor
    }

    async fn settle(&mut self) {
        while let Ok(Some(wake)) = tokio::time::timeout(Duration::from_millis(100), self.rx.recv()).await {
            self.rt.resume_task(wake).unwrap();
        }
    }

    async fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.rt.feed_key(KeyEvent::new(code, modifiers)).unwrap();
        self.settle().await;
    }

    async fn type_str(&mut self, text: &str) {
        for c in text.chars() {
            self.press(KeyCode::Char(c), KeyModifiers::NONE).await;
        }
    }

    async fn save(&mut self) {
        self.press(KeyCode::Char('s'), KeyModifiers::CONTROL).await;
    }
}

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gauchito-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[tokio::test(flavor = "current_thread")]
async fn edits_an_existing_file_and_saves_it() {
    let path = temp_path("existing.txt");
    std::fs::write(&path, "héllo\nworld\n").unwrap();

    let mut ed = Editor::start(&[path.to_str().unwrap()]).await;
    ed.press(KeyCode::End, KeyModifiers::NONE).await;
    ed.type_str("!").await;
    ed.press(KeyCode::Down, KeyModifiers::NONE).await;
    ed.press(KeyCode::Backspace, KeyModifiers::NONE).await;
    ed.save().await;

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "héllo!\nword\n");
}

#[tokio::test(flavor = "current_thread")]
async fn a_missing_file_opens_empty_and_saves_there() {
    let path = temp_path("missing.txt");
    let _ = std::fs::remove_file(&path);

    let mut ed = Editor::start(&[path.to_str().unwrap()]).await;
    ed.type_str("añ").await;
    ed.press(KeyCode::Enter, KeyModifiers::NONE).await;
    ed.type_str("b").await;
    ed.press(KeyCode::Left, KeyModifiers::NONE).await;
    ed.press(KeyCode::Up, KeyModifiers::NONE).await;
    ed.press(KeyCode::Delete, KeyModifiers::NONE).await;
    ed.save().await;

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "ñ\nb\n");
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_q_quits() {
    let mut ed = Editor::start(&[]).await;
    assert!(!ed.rt.should_quit());
    ed.press(KeyCode::Char('q'), KeyModifiers::CONTROL).await;
    assert!(ed.rt.should_quit());
}

#[tokio::test(flavor = "current_thread")]
async fn version_flag_quits_before_the_editor_starts() {
    let ed = Editor::start(&["--version"]).await;
    assert!(ed.rt.should_quit());
}

#[tokio::test(flavor = "current_thread")]
async fn keys_typed_while_the_file_opens_are_kept_in_order() {
    let path = temp_path("early.txt");
    std::fs::write(&path, "x\n").unwrap();

    let (mut rt, rx) = ScriptRuntime::new(vec![path.to_str().unwrap().to_string()]).unwrap();
    rt.load_config(&PathBuf::from("/nonexistent/init.lua")).unwrap();
    for c in "abc".chars() {
        rt.feed_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
    }
    let mut ed = Editor { rt, rx };
    ed.settle().await;
    ed.save().await;

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "abcx\n");
}
