use super::*;

// --- GC intrinsics: the happy path (golden dump) ---

#[test]
fn gc_intrinsics_golden() {
    let file = file(vec![fun(
        "main",
        vec![unsafe_block(vec![
            val("s", str_lit("hello")),
            val("h", call("pin", vec![var("s")])),
            val("s2", call("unpin", vec![var("h")])),
            val("g", call("getGcHandle", vec![var("s")])),
            val("s3", call("releaseGcHandle", vec![var("g")])),
            stmt(call("gcCollect", vec![])),
            val_ty("n", Some(ty_named("ULong")), call("gcStats", vec![])),
        ])],
    )]);
    let module = lower_user_with_gc(file).expect("the GC program must lower");
    assert_eq!(local_ty(&module, "s2"), "String");
    assert_eq!(local_ty(&module, "s3"), "String");
    assert_eq!(local_ty(&module, "n"), "ULong");
    let expected = include_str!("snapshots/gc_intrinsics_golden.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}
