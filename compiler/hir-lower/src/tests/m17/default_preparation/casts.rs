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
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    for (stage, dump) in [
        ("hir", source_blocks(&hir::dump(&output.export))),
        ("mir", source_blocks(&scoop_mir::dump(&mir))),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tests/fixtures/m23-executable-type-sites/casts.{stage}.snap"
        ));
        if std::env::var_os("SCOOP_UPDATE_CAST_TARGETS").is_some() {
            std::fs::write(&path, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());
    }
}
