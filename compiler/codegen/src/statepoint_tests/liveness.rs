use super::*;

#[test]
fn verifier_checks_pointer_uses_in_operand_bundles() {
    for (before, value, diagnostic) in [
        ("", "%live", "stale post-site use"),
        (
            "%derived = getelementptr i8, ptr addrspace(1) %live, i64 8",
            "%derived",
            "derived AS1 address crosses",
        ),
    ] {
        let context = Context::create();
        let after = format!(
            "call void @consume(ptr addrspace(1) %relocated) [ \"opaque\"(ptr addrspace(1) {value}, ptr addrspace(1) {value}) ]"
        );
        let module = parse(&context, &statepoint_ir(7, before, &after));
        let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
            .expect_err("operand bundles keep their pointer operands live");
        assert!(error.0.contains(diagnostic), "{error}");
    }
}

#[test]
fn verifier_checks_a_derived_statepoint_call_argument() {
    let context = Context::create();
    let ir = statepoint_ir(
        7,
        "%derived = getelementptr i8, ptr addrspace(1) %live, i64 8",
        "",
    )
    .replace("declare void @callee()", "declare void @callee(ptr addrspace(1))")
    .replace(
        "ptr elementtype(void ()) @callee, i32 0, i32 0, i32 0, i32 0",
        "ptr elementtype(void (ptr addrspace(1))) @callee, i32 1, i32 0, ptr addrspace(1) %derived, i32 0, i32 0",
    );
    let module = parse(&context, &ir);
    let error = verify_rewritten(&module, &manifest(Some((7, one_root()))))
        .expect_err("a managed call cannot borrow a moving interior address");
    assert!(error.0.contains("as a call operand"), "{error}");
}

#[test]
fn verifier_distinguishes_phi_incoming_edges_around_a_statepoint() {
    for derived in [false, true] {
        for stale in [false, true] {
            let context = Context::create();
            let definition = if derived {
                "%derived = getelementptr i8, ptr addrspace(1) %live, i64 8"
            } else {
                ""
            };
            let old = if derived { "%derived" } else { "%live" };
            let after_gc = if stale { old } else { "%relocated" };
            let before =
                format!("{definition}\n br i1 true, label %collect, label %bypass\ncollect:");
            let after = format!(
                "br label %join\nbypass:\n br label %join\njoin:\n \
                 %joined = phi ptr addrspace(1) [ {after_gc}, %collect ], [ {old}, %bypass ]\n \
                 call void @consume(ptr addrspace(1) %joined)"
            );
            let module = parse(&context, &statepoint_ir(7, &before, &after));
            let result = verify_rewritten(&module, &manifest(Some((7, one_root()))));
            if stale {
                let error = result.expect_err("the collected edge cannot pass the old pointer");
                let diagnostic = if derived {
                    "derived AS1 address crosses"
                } else {
                    "stale post-site use"
                };
                assert!(error.0.contains(diagnostic), "{error}");
            } else {
                result.expect("the bypass edge still holds its original pointer");
            }
        }
    }
}
