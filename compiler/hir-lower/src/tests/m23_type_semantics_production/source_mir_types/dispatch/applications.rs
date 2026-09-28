use super::*;
use scoop_identity::{
    CallableDefinitionOwner, CallableTemplateOrigin, StrongCallableDefinitionOwner,
};
use scoop_mir::{
    MirCallableBridgeAuthority, MirCallableBridgeError, MirCallableOriginV1,
    MirDispatchImplementationV1 as Implementation, MirTypeBridgeCallableLookupV1,
    ParamFreeMirCallableBindingV1,
};

#[test]
fn concrete_generic_dispatch_keeps_direct_parents_and_actual_odr_bodies() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-initialization");
    for case in ["dispatch", "dispatch-combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
        with_dispatch(&source, |output, input, _, types, authority, schemas| {
            let local = output.output().local.module();
            let exact = |name: &str| {
                let (_, class) = local
                    .classes
                    .iter()
                    .find(|(_, class)| class.name == name)
                    .unwrap();
                local.exact_type_identities[class.canonical_type].id()
            };
            let base = exact("Base");
            let derived = exact("Derived");
            let base_record = types.get(base).unwrap();
            let derived_record = types.get(derived).unwrap();
            assert_eq!(base_record.base_and_interfaces().interfaces.len(), 1);
            assert!(derived_record.base_and_interfaces().interfaces.is_empty());
            assert_eq!(
                derived_record.base_and_interfaces().base,
                scoop_mir::MirBaseClassV1::Base(base)
            );
            for owner in [base, derived] {
                let schema = schemas.get(owner).unwrap();
                assert_eq!(schema.vtable().len(), 2);
                assert_eq!(
                    schema.itables().len(),
                    if case == "dispatch" { 1 } else { 2 }
                );
                assert!(
                    schema
                        .itables()
                        .iter()
                        .all(|table| table.entries().len() == 2)
                );
            }
            for (_, interface) in local
                .interfaces
                .iter()
                .filter(|(_, interface)| !interface.type_arguments.is_empty())
            {
                let owner = local.exact_type_identities[interface.canonical_type].id();
                assert_eq!(
                    types.get(owner).unwrap().base_and_interfaces().interfaces,
                    interface
                        .parents
                        .iter()
                        .map(|parent| local.exact_type_identities[*parent].id())
                        .collect::<Vec<_>>()
                );
                let schema = schemas.get(owner).unwrap();
                assert!(schema.itables().is_empty());
                assert_eq!(schema.interface_slots().unwrap().len(), 2);
            }
            let roots = input.materialization().callable_roots();
            let mut bindings = BTreeMap::new();
            let mut defaults = 0;
            let mut traps = 0;
            for schema in schemas.records().iter().filter(|schema| {
                matches!(
                    types.get(schema.owner()).unwrap().origin(),
                    scoop_mir::MirTypeOriginV1::NominalApplication(_)
                )
            }) {
                for entry in schema
                    .vtable()
                    .iter()
                    .chain(schema.itables().iter().flat_map(|table| table.entries()))
                {
                    let target = entry.implementation().target();
                    assert!(matches!(target, CallableDefinitionOwner::Odr(_)));
                    let binding = authority.callables.get(target).unwrap();
                    let root = roots
                        .iter()
                        .find(|root| root.subject() == target.into())
                        .unwrap();
                    assert_eq!(
                        binding.lowered_signature().gc_effect(),
                        input.module().functions[root.function()].gc_effect
                    );
                    let scoop_mir::MirCallableRecordRefV1::Lowered(binding) = binding else {
                        panic!("applications have complete lowered bindings")
                    };
                    bindings.insert(target, binding.clone());
                    match entry.implementation() {
                        Implementation::InterfaceDefaultTarget { .. } => defaults += 1,
                        Implementation::AbstractObligation { .. } => {
                            traps += 1;
                            let body = &input.module().functions[root.function()].body;
                            assert!(matches!(
                                body.blocks[body.entry].terminator,
                                scoop_mir::Terminator::Trap { .. }
                            ));
                        }
                        _ => {}
                    }
                }
            }
            assert!(defaults > 0);
            assert_eq!(traps > 0, case == "dispatch-combined");
            let bindings =
                CanonicalMirCallableBindingsV1::try_new(bindings.into_values().collect()).unwrap();
            let callable_authority = MirCallableBridgeAuthority {
                identities: authority.identities,
                foundation: input.foundation(),
                types: authority.types,
            };
            let merged =
                MirTypeBridgeCallableIndexV1::try_new(&[&bindings, &bindings], &[]).unwrap();
            assert_eq!(merged.record_count(), bindings.entries().len());
            reject_wrong_application_target(callable_authority, &bindings);

            let snapshot = fixtures.join(format!("{case}.hir.snap"));
            let actual = hir::dump(&output.output().export);
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, &actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
        });
    }
}

fn reject_wrong_application_target(
    authority: MirCallableBridgeAuthority<'_>,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    let binding = &bindings.entries()[0];
    let MirCallableOriginV1::Application(application) = *binding.origin() else {
        panic!("source methods keep their application origin")
    };
    let application = authority
        .identities
        .canonical_key::<_, scoop_identity::CallableApplicationKey>(application)
        .unwrap();
    let strong = match application.origin() {
        CallableTemplateOrigin::Function(id) => StrongCallableDefinitionOwner::Function(id),
        CallableTemplateOrigin::Accessor(id) => StrongCallableDefinitionOwner::PropertyAccessor(id),
        _ => panic!("dispatch roots refer to methods or accessors"),
    };
    for wrong_target in [strong.into(), bindings.entries()[1].implementation()] {
        assert!(matches!(
            ParamFreeMirCallableBindingV1::try_new(
                authority,
                binding.origin().clone(),
                wrong_target,
                binding.semantic_signature().clone(),
                binding.lowered_signature().clone(),
                *binding.lowering_role(),
            ),
            Err(MirCallableBridgeError::OriginMismatch)
        ));
    }
}
