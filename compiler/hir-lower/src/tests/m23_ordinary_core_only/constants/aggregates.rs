use super::*;

#[test]
fn static_tuple_constants_use_images_and_keep_runtime_initializers() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/cones/aggregate-constants/src/values.scoop"
    ));
    with_input(source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        let module = output.output().export.module();
        let state = |name: &str| {
            let (_, global) = module
                .globals
                .iter()
                .find(|(_, global)| global.name == name)
                .unwrap();
            let hir::GlobalStorage::Managed { state } = &global.storage else {
                panic!("ordinary properties have managed storage")
            };
            state
        };
        assert_eq!(
            state("unit"),
            &hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::Unit,
            }
        );
        assert_eq!(
            state("single"),
            &hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::Tuple(vec![hir::HirConstantImage::Integer(
                    hir::HirIntegerConstant::Signed32(42)
                )]),
            }
        );
        let hir::HirStaticInitialState::EncodedStaticValue {
            payload: hir::HirConstantImage::Tuple(elements),
        } = state("values")
        else {
            panic!("nested tuple is a static image")
        };
        assert_eq!(
            elements[0],
            hir::HirConstantImage::Integer(hir::HirIntegerConstant::Signed8(127))
        );
        assert_eq!(
            elements[1],
            hir::HirConstantImage::Tuple(vec![
                hir::HirConstantImage::Unit,
                hir::HirConstantImage::String("embedded".into())
            ])
        );
        assert!(
            matches!(&elements[2], hir::HirConstantImage::Enum { fields, .. } if fields.is_empty())
        );
        assert!(matches!(
            state("runtimeValues"),
            hir::HirStaticInitialState::ZeroedForRuntimeUnit { .. }
        ));
    });
}
