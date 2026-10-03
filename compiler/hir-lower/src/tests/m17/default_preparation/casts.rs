use super::*;
use hir::concrete::ExprKind;

#[test]
fn optional_and_instantiated_default_casts_keep_the_explicit_checked_type() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-executable-type-sites/casts.scoop"
    ));
    let output = lower_source(source).unwrap();
    let module = output.local.module();
    let mut kinds = Vec::new();
    module
        .visit_executable_expressions(|occurrence| {
            if let ExprKind::Cast {
                check_ty, optional, ..
            } = occurrence.expression.kind
            {
                assert_eq!(
                    module.types[check_ty].kind,
                    hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
                );
                assert_eq!(check_ty == occurrence.expression.ty, !optional);
                kinds.push(optional);
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    kinds.sort_unstable();
    assert_eq!(kinds, [false, true, true]);
}
