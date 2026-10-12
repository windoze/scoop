use super::*;
use scoop_identity::{AccessorRole, CallableTemplateOwner, PropertyAccessorKey};

#[test]
fn signature_sites_preserve_receivers_constructors_and_exact_accessor_parameter_ranges() {
    with_hir_source(&source("declaration-combined"), |output, core| {
        let local = output.output().local.module();
        let interface = public_interface(output);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
            )
            .unwrap();

        let identities =
            source_inventory::identity_closure_for_foundation(output, foundation.clone());
        let validate = |root, part| {
            foundation.validate_declaration_type_position(
                local.cone,
                Position::CallableSignature(root, part),
                &[core.source_foundation.as_ref()],
            )
        };
        for site in interface
            .external_references()
            .records()
            .iter()
            .flat_map(|reference| reference.type_sites().records())
        {
            if let Some(expression) = site.as_expression() {
                foundation
                    .validate_executable_evaluation_origin(
                        local.cone,
                        expression.position().root,
                        expression.origin().evaluation(),
                        &foundation,
                    )
                    .unwrap();
            } else {
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        site.position(),
                        &[core.source_foundation.as_ref()],
                    )
                    .unwrap();
            }
        }
        let mut roles = [0; 2];
        for (_, function) in local.functions.iter() {
            let root = function.materialization;
            assert_eq!(
                validate(root, Part::Receiver).is_ok(),
                function.receiver.value_type().is_some(),
                "{}",
                function.name
            );
            if let CallableTemplateOwner::Accessor(id) = root.template() {
                let role = identities
                    .canonical_key::<_, PropertyAccessorKey>(id)
                    .unwrap()
                    .role();
                let setter = role == AccessorRole::Setter;
                roles[usize::from(setter)] += 1;
                assert_eq!(validate(root, Part::Parameter(0)).is_ok(), setter);
                assert!(matches!(
                    validate(root, Part::Parameter(1)),
                    Err(hir::DeclarationTypeSiteValidationError::SignaturePosition(
                        ..
                    ))
                ));
            }
        }
        assert!(roles.iter().all(|count| *count > 0));
        for (_, constructor) in local.class_constructors.iter() {
            assert!(matches!(
                validate(constructor.materialization, Part::Receiver),
                Err(hir::DeclarationTypeSiteValidationError::SignaturePosition(
                    ..
                ))
            ));
            validate(constructor.materialization, Part::Result).unwrap();
        }
    });
}

#[test]
fn constructor_receiver_type_sites_require_an_actual_outer_or_capture_input() {
    with_hir_source(
        r#"
        class Outer<T>(val value: T) {
            inner class Inner<U>(val value: U)
        }
        public fun checkContexts(): Int {
            val captured = 1
            class Local(val value: Int) {
                fun read(): Int = value + captured
            }
            val outer = Outer("outer")
            val inner = outer.Inner(41)
            return inner.value + Local(41).read()
        }
    "#,
        |output, core| {
            use scoop_identity::LocalValueSelector;
            let local = output.output().local.module();
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            let mut kinds = [false; 3];
            for constructor in local.class_constructors.values() {
                let context =
                    constructor
                        .parameters
                        .iter()
                        .find_map(|parameter| match parameter.selector {
                            LocalValueSelector::EnclosingInstance => Some(1),
                            LocalValueSelector::CapturedEnvironment => Some(2),
                            _ => None,
                        });
                kinds[context.unwrap_or(0)] = true;
                let result = foundation.validate_declaration_type_position(
                    local.cone,
                    Position::CallableSignature(constructor.materialization, Part::Receiver),
                    &[core.source_foundation.as_ref()],
                );
                if context.is_some() {
                    result.unwrap();
                } else {
                    assert!(matches!(
                        result,
                        Err(hir::DeclarationTypeSiteValidationError::SignaturePosition(
                            ..
                        ))
                    ));
                }
            }
            assert_eq!(kinds, [true; 3]);
        },
    );
}
