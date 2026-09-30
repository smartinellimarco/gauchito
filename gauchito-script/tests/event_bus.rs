//! Integration tests for the Lua-side pubsub utility.
//!
//! The substrate itself does not emit events — `gauchito.event` is a
//! pure-Lua pubsub primitive distros use however they like. These tests
//! verify subscribe/emit/once/cancel round-trip in Lua.

use gauchito_script::ScriptRuntime;

fn new_runtime() -> ScriptRuntime {
    let (rt, _task_rx) = ScriptRuntime::new(vec![]).unwrap();
    rt
}

#[test]
fn user_emit_round_trips() {
    let rt = new_runtime();
    rt.eval::<()>(
        r#"
        _hits = 0
        local h = gauchito.event.on('test:foo', function() _hits = _hits + 1 end)
        gauchito.event.emit('test:foo', nil)
        gauchito.event.emit('test:foo', nil)
        h:cancel()
        gauchito.event.emit('test:foo', nil)
        "#,
    )
    .unwrap();
    let hits: i64 = rt.eval("return _hits").unwrap();
    assert_eq!(hits, 2);
}

#[test]
fn once_fires_at_most_once() {
    let rt = new_runtime();
    rt.eval::<()>(
        r#"
        _hits = 0
        gauchito.event.once('test:foo', function() _hits = _hits + 1 end)
        gauchito.event.emit('test:foo', nil)
        gauchito.event.emit('test:foo', nil)
        "#,
    )
    .unwrap();
    let hits: i64 = rt.eval("return _hits").unwrap();
    assert_eq!(hits, 1);
}
