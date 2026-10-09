use super::*;

#[test]
fn open_default_equality_keeps_source_applications_and_exact_executable_bindings() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/derived-generic-defaults.scoop"
    ));
    with_source(source, |output, mir| {
        assert!(!mir.functions.is_empty());
        let module = output.output().export.module();
        let inputs = || hir::HirFunctionIdentityInputs {
            functions: &module.functions,
            lambdas: &module.lambdas,
            anonymous_functions: &module.anonymous_functions,
            local_functions: &module.local_functions,
            property_getters: &module.property_getters,
            property_setters: &module.property_setters,
            property_accessor_identities: &module.property_accessor_identities,
            initialization_units: &module.initialization_units,
            initialization_unit_identities: &module.initialization_unit_identities,
            derived_equality_applications: &module.derived_equality_applications,
            structs: &module.structs,
            enums: &module.enums,
            type_identities: &module.type_identities,
            struct_constructors: &module.struct_constructors,
            class_constructors: &module.class_constructors,
            constructor_identities: &module.constructor_identities,
            enum_member_identities: &module.enum_member_identities,
        };
        let identities = || {
            module
                .function_identities
                .iter()
                .map(|(_, id)| id.clone())
                .collect::<Vec<_>>()
        };
        hir::HirFunctionIdentities::checked(inputs(), identities()).unwrap();
        let mut open = 0;
        let mut exact = 0;
        let mut open_only = 0;
        for (function, identity) in module.function_identities.iter() {
            let hir::HirFunctionIdentity::DerivedEquality(records) = identity else {
                continue;
            };
            let applications = module
                .derived_equality_applications
                .iter()
                .filter(|(_, a)| a.function == function)
                .collect::<Vec<_>>();
            for (id, application) in &applications {
                match &module.type_identities[application.owner_ty] {
                    hir::HirTypeIdentity::Open(parameters) => {
                        assert!(!parameters.parameters().is_empty());
                        assert!(records.iter().all(|r| r.application() != *id));
                        open += 1;
                    }
                    hir::HirTypeIdentity::Exact(owner) => {
                        let record = records.iter().find(|r| r.application() == *id).unwrap();
                        assert_eq!(
                            record.record().key(),
                            &scoop_identity::GeneratedCallableKey::DerivedEquality {
                                exact_owner: owner.id()
                            }
                        );
                        exact += 1;
                    }
                }
            }
            let index = function.into_raw().into_u32() as usize;
            if records.is_empty() && !applications.is_empty() {
                open_only += 1;
                let concrete_owner = module
                    .types
                    .iter()
                    .find_map(|(id, _)| module.type_identities[id].exact())
                    .unwrap()
                    .id();
                let mut wrong = identities();
                wrong[index] = hir::HirFunctionIdentity::derived_equality(vec![
                    hir::HirDerivedEqualityFunctionIdentity::new(applications[0].0, concrete_owner)
                        .unwrap(),
                ]);
                assert!(matches!(
                    hir::HirFunctionIdentities::checked(inputs(), wrong),
                    Err(hir::HirFunctionIdentityError::DerivedEqualityIdentity { .. })
                ));
            } else if !records.is_empty() {
                let mut missing = identities();
                missing[index] = hir::HirFunctionIdentity::derived_equality(vec![]);
                assert!(matches!(
                    hir::HirFunctionIdentities::checked(inputs(), missing),
                    Err(hir::HirFunctionIdentityError::DerivedEqualityIdentity { .. })
                ));
            }
        }
        // Open defaults carry bound comparisons into their callers; only the
        // directly compared closed owner needs an exact derived callable.
        assert_eq!((open, exact, open_only), (3, 1, 2));
    });
}
