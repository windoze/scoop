use super::*;
use scoop_identity::{AccessorRole, CallableTemplateOwner, PropertyAccessorKey};

#[test]
fn signature_sites_preserve_receivers_constructors_and_exact_accessor_parameter_ranges() {
    with_hir_source(&source("declaration-combined"), |output, _| {
        let local = output.output().local.module();
        let interface = public_interface(output);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let identities = source_inventory::identity_closure_for_foundation(
            output,
            foundation.as_canonical().clone(),
        );
        let validate = |root, part| {
            foundation.validate_declaration_type_position(
                local.cone,
                Position::CallableSignature(root, part),
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
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
                        &mut BudgetMeter::new(DecodeLimits::default()),
                        &WirePath::root(),
                    )
                    .unwrap();
            } else {
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        site.position(),
                        &mut BudgetMeter::new(DecodeLimits::default()),
                        &WirePath::root(),
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
