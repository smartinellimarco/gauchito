//! Integration tests for `gauchito.task` and `gauchito.keys.read`.
//!
//! Drives the runtime under a tokio current-thread runtime so spawned
//! futures (sleep) actually fire. Each test reads its observable side
//! effects out of Lua via `rt.eval(...)`.

use std::time::Duration;

use gauchito_script::{KeyCode, KeyEvent, KeyModifiers, ScriptRuntime, TaskWakeRx};

fn make() -> (ScriptRuntime, TaskWakeRx) {
    ScriptRuntime::new(vec![]).unwrap()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ── A task that doesn't yield runs synchronously ──────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn spawn_runs_immediately_when_no_yield() {
    let (rt, _rx) = make();

    rt.eval::<()>(
        r#"
        _hits = 0
        gauchito.task.spawn(function() _hits = _hits + 1 end)
    "#,
    )
    .unwrap();
    assert_eq!(rt.eval::<i64>("return _hits").unwrap(), 1);
}

// ── A task that errors is caught (doesn't crash the runtime) ──────────────

#[tokio::test(flavor = "current_thread")]
async fn spawn_swallows_errors_in_task_body() {
    let (rt, _rx) = make();

    rt.eval::<()>(
        r#"
        gauchito.task.spawn(function() error('boom') end)
    "#,
    )
    .unwrap();

    rt.eval::<()>("_after = true").unwrap();
    assert_eq!(rt.eval::<bool>("return _after").unwrap(), true);
}

// ── task.sleep: the real async path ────────────────────────────────────────

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sleep_suspends_and_resumes_after_timer() {
    let (mut rt, mut rx) = make();

    rt.eval::<()>(
        r#"
        _phase = 'before'
        gauchito.task.spawn(function()
            gauchito.task.sleep(50):await()
            _phase = 'after'
        end)
    "#,
    )
    .unwrap();

    assert_eq!(rt.eval::<String>("return _phase").unwrap(), "before");

    tokio::time::advance(Duration::from_millis(60)).await;

    let wake = rx
        .recv()
        .await
        .expect("expected a TaskWake from the sleep");
    rt.resume_task(wake).unwrap();

    assert_eq!(rt.eval::<String>("return _phase").unwrap(), "after");
}

// ── Multiple tasks in flight, resolved in order ──────────────────────────

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn multiple_tasks_resolve_independently() {
    let (mut rt, mut rx) = make();

    rt.eval::<()>(
        r#"
        _events = {}
        gauchito.task.spawn(function()
            gauchito.task.sleep(10):await()
            table.insert(_events, 'fast')
        end)
        gauchito.task.spawn(function()
            gauchito.task.sleep(50):await()
            table.insert(_events, 'slow')
        end)
    "#,
    )
    .unwrap();

    tokio::time::advance(Duration::from_millis(15)).await;
    let wake = rx.recv().await.unwrap();
    rt.resume_task(wake).unwrap();
    assert_eq!(rt.eval::<String>("return _events[1]").unwrap(), "fast");

    tokio::time::advance(Duration::from_millis(60)).await;
    let wake = rx.recv().await.unwrap();
    rt.resume_task(wake).unwrap();
    assert_eq!(rt.eval::<String>("return _events[2]").unwrap(), "slow");
    assert_eq!(rt.eval::<i64>("return #_events").unwrap(), 2);
}

// ── gauchito.io.read: read + sniff via spawn_blocking ────────────────────

#[tokio::test(flavor = "current_thread")]
async fn io_read_returns_rope_and_sniffed_options() {
    let (mut rt, mut rx) = make();

    let tmp = std::env::temp_dir().join(format!(
        "gauchito-io-read-{}.txt",
        std::process::id()
    ));
    std::fs::write(&tmp, "hello world\nsecond line\n").unwrap();
    let path = tmp.to_string_lossy().to_string();

    rt.eval::<()>(&format!(
        r#"
        _io = nil
        gauchito.task.spawn(function()
            local result = gauchito.io.read({path:?}):await()
            _io = {{
                len = result.rope:len(),
                first_line = result.rope:line(0),
                line_ending = result.sniffed.line_ending,
                final_newline = result.sniffed.final_newline,
            }}
        end)
    "#,
    ))
    .unwrap();

    assert!(rt.eval::<bool>("return _io == nil").unwrap());

    let wake = rx.recv().await.expect("io.read wake");
    rt.resume_task(wake).unwrap();

    assert_eq!(
        rt.eval::<i64>("return _io.len").unwrap(),
        ("hello world\nsecond line\n".chars().count()) as i64
    );
    assert_eq!(
        rt.eval::<String>("return _io.first_line").unwrap(),
        "hello world"
    );
    assert_eq!(rt.eval::<String>("return _io.line_ending").unwrap(), "lf");
    assert_eq!(rt.eval::<bool>("return _io.final_newline").unwrap(), true);

    std::fs::remove_file(&tmp).ok();
}

// ── gauchito.buf.scratch: returns a fresh, unattached LuaBuffer ─────────

#[tokio::test(flavor = "current_thread")]
async fn buf_scratch_returns_a_fresh_buffer() {
    let (rt, _rx) = make();

    rt.eval::<()>(
        r#"
        _buf = gauchito.buf.scratch(nil, { indent = { unit = '  ', tab_width = 2 } })
    "#,
    )
    .unwrap();

    let id: i64 = rt.eval("return _buf.id").unwrap();
    assert!(id > 0);

    let path: Option<String> = rt.eval("return _buf.path").unwrap();
    assert!(path.is_none(), "scratch should have no path");
}

#[tokio::test(flavor = "current_thread")]
async fn io_read_propagates_missing_file_as_lua_error() {
    let (mut rt, mut rx) = make();

    rt.eval::<()>(
        r#"
        _ok = nil
        _err = nil
        gauchito.task.spawn(function()
            local ok, err = pcall(function()
                gauchito.io.read('/nonexistent/path/should/not/exist'):await()
            end)
            _ok, _err = ok, err
        end)
    "#,
    )
    .unwrap();

    let wake = rx.recv().await.expect("io.read wake");
    rt.resume_task(wake).unwrap();

    assert_eq!(rt.eval::<bool>("return _ok").unwrap(), false);
    let err: String = rt.eval("return tostring(_err)").unwrap();
    assert!(err.contains("io.read"), "expected io.read error, got: {err}");
}

// ── gauchito.keys.read: pull-based key dispatch ───────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn keys_read_resumes_with_delivered_key() {
    let (mut rt, _rx) = make();

    rt.eval::<()>(
        r#"
        _keys = {}
        gauchito.task.spawn(function()
            while #_keys < 3 do
                local name, ch = gauchito.keys.read():await()
                table.insert(_keys, name .. '/' .. tostring(ch))
            end
        end)
    "#,
    )
    .unwrap();

    rt.feed_key(key(KeyCode::Char('h'))).unwrap();
    rt.feed_key(key(KeyCode::Char('i'))).unwrap();
    rt.feed_key(key(KeyCode::Esc)).unwrap();

    assert_eq!(rt.eval::<String>("return _keys[1]").unwrap(), "h/h");
    assert_eq!(rt.eval::<String>("return _keys[2]").unwrap(), "i/i");
    assert_eq!(rt.eval::<String>("return _keys[3]").unwrap(), "esc/nil");
}
