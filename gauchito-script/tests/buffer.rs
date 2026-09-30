use gauchito_script::ScriptRuntime;

fn make() -> ScriptRuntime {
    ScriptRuntime::new(vec![]).unwrap().0
}

#[test]
fn kernels_walk_multibyte_text_by_byte_offsets() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({ gauchito.splice.new(0, 0, "ñandú añejo") })

        assert(buf:scan_class_fwd(0) == 7)
        assert(buf:right(7) == 8)
        assert(buf:right(8) == 9)
        assert(buf:left(11) == 9)
        assert(buf:scan_class_bwd(11) == 8)
        assert(buf:visual_col(11) == 8)
    "#,
    )
    .unwrap();
}

#[test]
fn positions_inside_a_char_are_a_lua_error() {
    let rt = make();
    let err = rt
        .eval::<()>(
            r#"
            local buf = gauchito.buf.scratch()
            buf:apply({ gauchito.splice.new(0, 0, "ñ") })
            buf:apply({ gauchito.splice.new(1, 1, "x") })
        "#,
        )
        .unwrap_err();
    assert!(err.to_string().contains("not a char boundary"), "{err}");
}
