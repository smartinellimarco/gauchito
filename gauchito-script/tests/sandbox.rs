//! Sandbox contract. Distro Lua is untrusted third-party code, so the
//! shared standard libraries are frozen — but the distro must still be
//! able to hold its own state and override the documented extension
//! points. These tests pin that boundary.

use gauchito_script::ScriptRuntime;

fn rt() -> ScriptRuntime {
    ScriptRuntime::new(vec![]).unwrap().0
}

#[test]
fn std_libraries_are_frozen() {
    let rt = rt();
    // Untrusted code must not monkeypatch shared std libs for everyone else.
    assert!(
        rt.eval::<()>("string.rep = function() return 'pwned' end")
            .is_err(),
        "expected writing to a std lib to be rejected under sandbox"
    );
    assert!(
        rt.eval::<()>("table.insert = nil").is_err(),
        "expected clearing a std lib field to be rejected under sandbox"
    );
}

#[test]
fn distro_can_hold_its_own_state() {
    let rt = rt();
    assert_eq!(
        rt.eval::<i64>("my_distro_state = 41; return my_distro_state + 1")
            .unwrap(),
        42
    );
}

#[test]
fn distro_can_override_extension_points() {
    let rt = rt();
    // The prelude documents gauchito.keys.name as distro-overridable.
    rt.eval::<()>("gauchito.keys.name = function() return 'overridden' end")
        .unwrap();
    assert_eq!(
        rt.eval::<String>("return gauchito.keys.name({})").unwrap(),
        "overridden"
    );
}
