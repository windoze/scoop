use super::*;
use hir::concrete::ExprKind;

#[test]
fn shared_shape_sites_match_actual_boxes_unboxes_and_checked_targets() {
    for case in ["shape-standalone", "shape-combined"] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-executable-type-sites/{case}.scoop"),
            ))
            .unwrap();
        with_hir_source(&source, |output, _| {
            let interface = public_interface(output);
            let module = output.output().local.module();
            let mut actual = Vec::new();
            let mut operations = [0_usize; 4];
            module
                .visit_executable_expressions(
                    &mut BudgetMeter::new(DecodeLimits::default()),
                    |occurrence, _| {
                        let expression = occurrence.expression;
                        let (role, ty, index) = match &expression.kind {
                            ExprKind::Box(operand) => {
                                (HirExpressionTypeRoleV1::BoxedValue, operand.ty, 0)
                            }
                            ExprKind::Unbox(_) => {
                                (HirExpressionTypeRoleV1::BoxedValue, expression.ty, 1)
                            }
                            ExprKind::Cast { check_ty, .. } => {
                                (HirExpressionTypeRoleV1::TypeTest, *check_ty, 2)
                            }
                            ExprKind::IsInstance { check_ty, .. } => {
                                (HirExpressionTypeRoleV1::TypeTest, *check_ty, 3)
                            }
                            _ => return Ok::<_, std::convert::Infallible>(()),
                        };
                        operations[index] += 1;
                        actual.push((
                            occurrence.position,
                            role,
                            module.exact_type_identities.get(ty).unwrap().id(),
                        ));
                        Ok(())
                    },
                )
                .unwrap();
            assert!(operations[0] > 0 && operations[2] > 0 && operations[3] > 0);
            if case == "shape-combined" {
                assert!(operations[1] > 0);
            }
            let mut projected = Vec::new();
            for site in interface
                .external_references()
                .records()
                .iter()
                .flat_map(|reference| reference.type_sites().records())
                .filter_map(|site| site.as_expression())
                .filter(|site| {
                    matches!(
                        site.role(),
                        HirExpressionTypeRoleV1::BoxedValue | HirExpressionTypeRoleV1::TypeTest
                    )
                })
            {
                let key = (site.position(), site.role(), site.exact());
                assert!(actual.contains(&key), "{case}: {key:?}");
                assert_eq!(site.origin().definition().source().cone(), module.cone);
                assert_eq!(site.origin().evaluation().source().cone(), module.cone);
                projected.push(key);
            }
            projected.sort_unstable();
            projected.dedup();
            // The combined fixture also boxes one current-Cone nominal.
            assert_eq!(
                projected.len(),
                actual.len() - usize::from(case == "shape-combined")
            );
        });
    }
}
