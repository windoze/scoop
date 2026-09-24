use super::*;
use hir::{
    HirCallableTypePositionV1 as Part, HirDependencyTypePositionV1 as Position,
    HirDependencyTypeSiteV1 as Site,
};
use source_dispatch::with_hir_source;

mod signatures;
mod storage;

fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-executable-type-sites")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn unread_parameters_keep_complete_signature_and_local_value_type_sites() {
    with_hir_source(&source("declaration-standalone"), |output, _| {
        let interface = public_interface(output);
        let local = output.output().local.module();
        let (function_id, function) = local
            .functions
            .iter()
            .find(|(_, function)| function.name == "onlySignature")
            .unwrap();
        let parameter = &function.params[0];
        let value = local
            .local_value_identities
            .function_local(function_id, parameter.local)
            .id();
        let exact = local.exact_type_identities.get(parameter.ty).unwrap().id();
        let sites: Vec<_> = interface
            .external_references()
            .records()
            .iter()
            .flat_map(|reference| reference.type_sites().records())
            .collect();
        assert_eq!(sites.len(), 3);
        assert!(sites.iter().all(|site| site.as_expression().is_none()));
        assert!(sites.iter().any(|site| matches!(site, Site::CallableSignature { root, position: Part::Parameter(0), exact: ty } if *root == function.materialization && *ty == exact)));
        assert!(sites.iter().any(|site| matches!(site, Site::LocalValue { local, exact: ty } if *local == value && *ty == exact)));
        let foundation = hir::OdrFreeHirFoundation::try_new(
            hir::CanonicalHirFoundation::from_dependency_output(output).unwrap(),
        )
        .unwrap();
        for site in sites {
            foundation
                .validate_declaration_type_position(
                    local.cone,
                    site.position(),
                    &mut BudgetMeter::new(DecodeLimits::default()),
                    &WirePath::root(),
                )
                .unwrap();
        }
    });
}

#[test]
fn declaration_type_sites_reject_missing_identities_wrong_providers_and_invalid_positions() {
    with_hir_source(&source("declaration-standalone"), |output, _| {
        let local = output.output().local.module();
        let function = local
            .functions
            .iter()
            .find(|(_, function)| function.name == "onlySignature")
            .unwrap()
            .1;
        let root = function.materialization;
        let foundation = hir::OdrFreeHirFoundation::try_new(
            hir::CanonicalHirFoundation::from_dependency_output(output).unwrap(),
        )
        .unwrap();
        let validate = |current, position, meter: &mut BudgetMeter| {
            foundation.validate_declaration_type_position(
                current,
                position,
                meter,
                &WirePath::root(),
            )
        };
        for part in [Part::Parameter(99), Part::Receiver] {
            let invalid = Position::CallableSignature(root, part);
            assert!(matches!(
                validate(
                    local.cone,
                    invalid,
                    &mut BudgetMeter::new(DecodeLimits::default())
                ),
                Err(hir::DeclarationTypeSiteValidationError::SignaturePosition(
                    ..
                ))
            ));
        }
        let position = Position::CallableSignature(root, Part::Parameter(0));
        assert!(matches!(
            validate(
                ConeIdentity::CORE,
                position,
                &mut BudgetMeter::new(DecodeLimits::default())
            ),
            Err(hir::DeclarationTypeSiteValidationError::Provider { .. })
        ));
        let missing =
            scoop_identity::PersistentLocalValueId::from_key(&scoop_identity::LocalValueKey::new(
                root,
                scoop_identity::LocalValueSelector::Parameter {
                    declaration_index: 99,
                },
            ))
            .unwrap();
        assert!(matches!(
            validate(
                local.cone,
                Position::LocalValue(missing),
                &mut BudgetMeter::new(DecodeLimits::default())
            ),
            Err(hir::DeclarationTypeSiteValidationError::MissingIdentity { .. })
        ));
        let mut baseline = BudgetMeter::new(DecodeLimits::default());
        validate(local.cone, position, &mut baseline).unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: baseline.usage().validation_work_units,
            ..DecodeLimits::default()
        });
        validate(local.cone, position, &mut shared).unwrap();
        assert!(validate(local.cone, position, &mut shared).is_err());
        assert!(
            validate(
                local.cone,
                position,
                &mut BudgetMeter::new(DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                })
            )
            .is_err()
        );
    });
}
