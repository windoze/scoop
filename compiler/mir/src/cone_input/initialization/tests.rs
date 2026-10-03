use super::*;
use crate::*;

mod support;
use support::*;

#[test]
fn sealer_retains_exact_roles_from_hir_owned_source_materializations() {
    let module = fixture();
    assert!(module.meta.generated_callables.is_empty());
    let identity = module.initialization_units[unit_id()].identity.id();
    let ensure = module.initialization_units[unit_id()].ensure;
    let initializer = module.initialization_units[unit_id()].initializer;
    let input = seal(module).unwrap();
    let root = input.materialization().initialization_roots()[0];
    assert_eq!(root.unit(), unit_id());
    assert_eq!(root.identity(), identity);
    assert_eq!(root.ensure().function(), ensure);
    assert_eq!(root.initializer().function(), initializer);
    assert_ne!(root.ensure().subject(), root.initializer().subject());
    assert_eq!(
        input.materialization().initialization_units(),
        &[root.unit()]
    );
}

#[test]
fn swapped_roles_or_an_ordinary_function_cannot_supply_an_ensure() {
    for ordinary in [false, true] {
        let mut module = fixture();
        let wrong = if ordinary {
            module.top_level[0]
        } else {
            module.initialization_units[unit_id()].initializer
        };
        module.initialization_units[unit_id()].ensure = wrong;
        assert!(matches!(
            seal(module),
            Err(ConeMirInputError::Initialization(
                StrongInitializationUnitError::WrongRole {
                    role: InitializationCallableRole::Ensure,
                    ..
                }
            ))
        ));
    }
}

#[test]
fn unit_identity_is_part_of_the_generated_callable_contract() {
    let mut module = fixture();
    module.initialization_units[unit_id()].identity = unit_identity("other");
    assert!(matches!(
        seal(module),
        Err(ConeMirInputError::Initialization(
            StrongInitializationUnitError::WrongRole {
                role: InitializationCallableRole::Initializer,
                ..
            }
        ))
    ));
}

#[test]
fn each_role_must_be_an_emitted_body_not_only_a_foundation_signature() {
    let mut module = fixture();
    let ensure = module.initialization_units[unit_id()].ensure;
    module.top_level.retain(|function| *function != ensure);
    assert!(
        matches!(seal(module), Err(ConeMirInputError::Initialization(
        StrongInitializationUnitError::MissingBody { function, .. }
    )) if function == ensure)
    );
}

#[test]
fn physical_gc_parameters_and_result_are_checked_before_sealing() {
    for change in 0..3 {
        let mut module = fixture();
        let ensure = module.initialization_units[unit_id()].ensure;
        let body = &mut module.functions[ensure];
        match change {
            0 => body.gc_effect = GcEffect::NoGc,
            1 => {
                let local = body.body.locals.alloc(Local {
                    name: "unexpected".into(),
                    ty: Type::Unit,
                    mutable: false,
                });
                body.params.push(Param {
                    name: "unexpected".into(),
                    ty: Type::Unit,
                    local,
                });
            }
            2 => body.return_ty = Type::String,
            _ => unreachable!(),
        }
        assert!(
            matches!(seal(module), Err(ConeMirInputError::Initialization(
            StrongInitializationUnitError::Signature { function, .. }
        )) if function == ensure)
        );
    }
}

#[test]
fn logical_signature_checks_execution_receiver_parameters_and_unit_result() {
    use scoop_identity::ExactCallableSignature;
    for change in 0..4 {
        let mut module = fixture();
        let ensure = module.initialization_units[unit_id()].ensure;
        let exact = module
            .meta
            .source_exact_types
            .get(&Type::Unit)
            .unwrap()
            .identity_record()
            .id();
        let old = module
            .meta
            .source_callable_materializations
            .get(ensure)
            .unwrap();
        let signature = ExactCallableSignature::new(
            if change == 0 {
                Effect::Suspend
            } else {
                Effect::Ordinary
            },
            if change == 1 { Some(exact) } else { None },
            if change == 2 { vec![exact] } else { vec![] },
            if change == 3 {
                module
                    .meta
                    .source_exact_types
                    .get(&Type::String)
                    .unwrap()
                    .identity_record()
                    .id()
            } else {
                exact
            },
        );
        let changed =
            SourceCallableMaterialization::new(ensure, old.materialization(), signature, None)
                .unwrap();
        replace_source(&mut module, changed);
        assert!(
            matches!(seal(module), Err(ConeMirInputError::Initialization(
            StrongInitializationUnitError::Signature { function, .. }
        )) if function == ensure)
        );
    }
}

#[test]
fn duplicate_unit_identity_is_rejected_before_building_a_second_root() {
    let mut module = fixture();
    let first = &module.initialization_units[unit_id()];
    let duplicate = InitializationUnit {
        identity: first.identity.clone(),
        display_name: "duplicate".into(),
        schedule: first.schedule,
        kind: first.kind,
        initializer: first.initializer,
        ensure: first.ensure,
        failure_root: first.failure_root,
        dependencies: vec![],
        cycle_thrower: first.cycle_thrower,
    };
    module.initialization_units.alloc(duplicate);
    assert!(matches!(
        seal(module),
        Err(ConeMirInputError::Initialization(
            StrongInitializationUnitError::DuplicateUnit { .. }
        ))
    ));
}

#[test]
fn generic_unit_rejects_ordinary_role_functions() {
    use scoop_identity::{
        CborIdentityRecord, NonEmptyVec, PersistentExtensionPropertyId, SignatureTypeKey,
    };
    let mut module = fixture();
    let declaration = SourceDeclarationKey::extension_property(
        scoop_identity::SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap(),
        scoop_identity::CanonicalIdentifier::new("generic").unwrap(),
        1,
        SignatureTypeKey::Binder { depth: 0, index: 0 },
    );
    let property = PersistentExtensionPropertyId::from_source_declaration(&declaration).unwrap();
    let exact = module
        .meta
        .source_exact_types
        .get(&Type::Unit)
        .unwrap()
        .identity_record()
        .id();
    module.initialization_units[unit_id()].identity = CborIdentityRecord::from_key(
        InitializationUnitKey::GenericDelegatedExtensionApplication {
            property,
            receiver_arguments: NonEmptyVec::new(vec![exact]).unwrap(),
        },
    )
    .unwrap();
    assert!(matches!(
        seal(module),
        Err(ConeMirInputError::Initialization(
            StrongInitializationUnitError::WrongRole { .. }
        ))
    ));
}
