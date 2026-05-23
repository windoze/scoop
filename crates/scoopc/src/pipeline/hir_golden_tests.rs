use std::path::PathBuf;

use crate::hir::{ExprKind, FunDecl, Item, Stmt, StmtKind};
use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;

fn session() -> Session {
    Session::with_options(SessionOptions::new()).unwrap()
}

fn find_fun<'a>(lowered: &'a crate::hir::LoweredHir, fqn: &str) -> &'a FunDecl {
    lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == fqn => Some(fun),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected HIR fun {fqn}"))
}

#[test]
fn hir_tail_if_uses_declared_return_type_hint() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/hir_tail_if_return_expected.scoop",
        r#"
fun main(): Int {
    if (true) { 3 } else { 1 }
}
"#,
    );

    let lowered = super::lower_typed_hir_for_dump(&sess, &src)
        .expect("HIR stage should lower a tail if with declared return type");
    let main = find_fun(&lowered, "main");
    let body = main.body.as_ref().expect("main has body");
    assert_eq!(
        body.ty, main.return_ty,
        "body type should match function return type"
    );

    let [
        Stmt {
            kind: StmtKind::Expr(expr),
            ..
        },
    ] = body.stmts.as_slice()
    else {
        panic!(
            "main should contain a single tail expr stmt, got {:?}",
            body.stmts
        );
    };
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &expr.kind
    else {
        panic!("main tail should remain an if expr, got {expr:?}");
    };

    assert_eq!(
        expr.ty, main.return_ty,
        "if expr should inherit function return type"
    );
    assert_eq!(
        then_branch.ty, main.return_ty,
        "then branch should stay Int"
    );
    assert_eq!(
        else_branch.as_deref().expect("if has else branch").ty,
        main.return_ty,
        "else branch should stay Int"
    );
}

fn assert_hir_golden(source_name: &str, golden_name: &str) {
    let sess = Session::new().unwrap();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir")
        .join(source_name);
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = super::load_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir")
        .join(golden_name);
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_minimal_golden() {
    assert_hir_golden("minimal.scoop", "minimal.hir");
}

#[test]
fn hir_fixture_handle_perform_golden() {
    assert_hir_golden("handle_perform.scoop", "handle_perform.hir");
}

#[test]
fn hir_fixture_control_flow_golden() {
    assert_hir_golden("control_flow.scoop", "control_flow.hir");
}

#[test]
fn hir_fixture_member_access_golden() {
    assert_hir_golden("member_access.scoop", "member_access.hir");
}

#[test]
fn hir_fixture_closure_non_capture_golden() {
    assert_hir_golden("closure_non_capture.scoop", "closure_non_capture.hir");
}

#[test]
fn hir_fixture_closure_capture_val_golden() {
    assert_hir_golden("closure_capture_val.scoop", "closure_capture_val.hir");
}
