use std::collections::BTreeSet;

use object::{Object, ObjectSymbol};

use super::*;

mod constants;
mod cstrings;
mod no_gc;
mod optimized;
mod parallel;
mod references;
mod switches;

#[test]
fn partitions_callables_and_type_metadata_away_from_static_storage() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let callable_bodies = module
        .functions
        .iter()
        .map(|function| function.callable_body.id())
        .collect::<BTreeSet<_>>();
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();

    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(input.foundation()).unwrap();
    let partition = ScoopLirObjectPartitionV1::from_input(&input, &surface).unwrap();

    for units in partition
        .objects()
        .iter()
        .filter(|units| units.kind() == ScoopLirObjectKindV1::NonCallable)
    {
        assert!(!units.definition_plans().is_empty());
        for definition in units.definition_plans() {
            use scoop_lir::StrongDefinitionRole::*;
            let plan = surface.plan(*definition).unwrap();
            let role = plan.definition_role();
            assert!(!matches!(
                role,
                CallableBody | CallableRegistration | SafepointRegistration
            ));
            if matches!(role, TypeDescriptor | TypeRegistration)
                && plan.primary_symbol().linkage() != scoop_lir::LinkageClass::OdrWeak
            {
                let roles = units
                    .definition_plans()
                    .iter()
                    .map(|id| {
                        let companion = surface.plan(*id).unwrap();
                        assert_eq!(companion.owner(), plan.owner());
                        companion.definition_role()
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(roles, BTreeSet::from([TypeDescriptor, TypeRegistration]));
                assert_eq!(units.definition_plans().len(), 2);
            } else if matches!(
                role,
                TypeDescriptor | TypeRegistration | Layout | ScanProgram
            ) {
                assert_eq!(units.definition_plans(), &[*definition]);
            }
        }
    }

    let actual_bodies = partition
        .objects()
        .iter()
        .filter(|object| matches!(object.kind(), ScoopLirObjectKindV1::CallableBody(_)))
        .map(|object| {
            let ScoopLirObjectKindV1::CallableBody(body) = object.kind() else {
                panic!("only the first object may be non-callable");
            };
            let plans = object
                .definition_plans()
                .iter()
                .map(|id| surface.plan(*id).unwrap())
                .collect::<Vec<_>>();
            assert!(plans.iter().any(|plan| plan.definition_role()
                == scoop_lir::StrongDefinitionRole::CallableRegistration));
            for plan in plans {
                match plan.owner().kind() {
                    scoop_lir::StrongDefinitionEntityKind::CallableBody(owner) => {
                        assert_eq!(owner, body)
                    }
                    scoop_lir::StrongDefinitionEntityKind::SafepointSite(site) => {
                        let function = input
                            .module()
                            .callable_bodies()
                            .find(|function| function.callable_body.id() == body)
                            .unwrap();
                        assert!(
                            function
                                .safepoints
                                .iter()
                                .any(|entry| entry.site_id() == site)
                        );
                    }
                    other => panic!("unrelated definition in callable object: {other:?}"),
                }
            }
            body
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bodies, callable_bodies);
}

#[test]
fn partition_is_complete_non_overlapping_and_excludes_generated_bridge_units() {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let producer_units =
        scoop_lir::ProducerUnitPartitionV1::from_foundation(input.foundation()).unwrap();

    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(input.foundation()).unwrap();
    let partition = ScoopLirObjectPartitionV1::from_input(&input, &surface).unwrap();

    let actual = partition
        .objects()
        .iter()
        .flat_map(ScoopLirObjectUnitSetV1::definition_plans)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        actual.iter().copied().collect::<BTreeSet<_>>().len(),
        actual.len(),
        "one definition plan must not occur in multiple objects"
    );
    assert_eq!(
        actual.iter().copied().collect::<BTreeSet<_>>(),
        producer_units
            .scoop_lir_definition_plans()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    );
    let generated_bridge_definitions = producer_units
        .generated_bridge_units()
        .iter()
        .flat_map(scoop_lir::GeneratedBridgeProducerUnitV1::definition_plans)
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(
        actual
            .iter()
            .all(|plan| !generated_bridge_definitions.contains(plan))
    );
}

#[test]
fn renders_only_the_callable_selected_by_each_physical_member() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let symbols = module
        .functions
        .iter()
        .map(|function| (function.callable_body.id(), function.symbol().to_string()))
        .collect::<Vec<_>>();
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();

    let rendered = render_llvm_ir_members(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        host_profile(),
    )
    .unwrap();

    for member in rendered {
        let defined = symbols
            .iter()
            .filter(|(_, symbol)| {
                member.llvm_ir().lines().any(|line| {
                    line.starts_with("define ") && line.contains(&format!("@\"{symbol}\""))
                })
            })
            .map(|(body, _)| *body)
            .collect::<Vec<_>>();
        match member.units().kind() {
            ScoopLirObjectKindV1::NonCallable => assert!(defined.is_empty()),
            ScoopLirObjectKindV1::CallableBody(body) => assert_eq!(defined, vec![body]),
        }
    }
}

#[test]
fn emitted_object_set_owns_complete_members() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let expected_bodies = module.functions.len();
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();

    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        host_profile(),
        std::num::NonZeroUsize::MIN,
    )
    .unwrap();

    assert_eq!(
        emitted
            .members
            .iter()
            .filter(|member| matches!(
                member.kind,
                EmittedConeObjectMemberKindV1::CallableBody { .. }
            ))
            .count(),
        expected_bodies
    );
    assert_eq!(emitted.target(), input.module().meta.target_profile);
    assert_eq!(
        emitted.target_selection,
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1
    );
    let projected = ScoopLirObjectPartitionV1::from_foundation(
        &input,
        &emitted.foundation,
        emitted.production.canonical_definitions(),
    )
    .unwrap();
    assert_eq!(projected, emitted.partition);
    assert_eq!(emitted.members.len(), projected.objects().len());
    assert!(
        emitted
            .members
            .iter()
            .all(|member| !member.bytes.is_empty())
    );
}

#[test]
fn emitted_callable_members_materialize_every_planned_atom_boundary() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        host_profile(),
        std::num::NonZeroUsize::MIN,
    )
    .unwrap();
    let normalization = input
        .module()
        .meta
        .target_profile
        .contract()
        .native_symbol_normalization();
    let mut callable_count = 0;

    for member in &emitted.members {
        let EmittedConeObjectMemberKindV1::CallableBody { body, .. } = member.kind else {
            continue;
        };
        callable_count += 1;
        let plan = member
            .units
            .definition_plans()
            .iter()
            .filter_map(|id| emitted.production.canonical_definitions().plan(*id))
            .find(|plan| plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody)
            .unwrap();
        assert_eq!(
            plan.owner().kind(),
            scoop_lir::StrongDefinitionEntityKind::CallableBody(body)
        );
        let bytes = member.bytes.clone();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        for definition in member.units.definition_plans() {
            let definition = emitted
                .production
                .canonical_definitions()
                .plan(*definition)
                .unwrap();
            let name = normalization
                .compiler_generated_object_symbol(definition.primary_symbol().symbol().as_str());
            assert!(
                object
                    .symbol_by_name(&name)
                    .is_some_and(|symbol| symbol.is_definition()),
                "missing colocated definition {name}"
            );
        }
        let primary_name =
            normalization.compiler_generated_object_symbol(plan.primary_symbol().symbol().as_str());
        let primary = object.symbol_by_name(&primary_name).unwrap();

        for boundary in plan.atom_boundaries() {
            let start_name =
                normalization.compiler_generated_object_symbol(boundary.start().symbol().as_str());
            let end_name =
                normalization.compiler_generated_object_symbol(boundary.end().symbol().as_str());
            let start = object.symbol_by_name(&start_name).unwrap();
            let end = object.symbol_by_name(&end_name).unwrap();
            assert!(
                start.is_definition(),
                "missing definition for `{start_name}`"
            );
            assert!(end.is_definition(), "missing definition for `{end_name}`");
            assert_eq!(start.section_index(), end.section_index());
            assert!(start.address() < end.address());
            if boundary.atom() == plan.primary_atom() {
                assert_eq!(primary.section_index(), start.section_index());
                assert_eq!(primary.address(), start.address());
            }
        }
    }
    assert_eq!(callable_count, input.module().functions.len());
}
