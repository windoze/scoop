use std::collections::BTreeSet;

use object::{Object, ObjectSymbol};

use super::*;

mod no_gc;

#[test]
fn partitions_each_callable_body_away_from_non_callable_definitions() {
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

    assert_eq!(partition.objects().len(), callable_bodies.len() + 1);
    let non_callable = &partition.objects()[0];
    assert_eq!(non_callable.kind(), ScoopLirObjectKindV1::NonCallable);
    assert!(!non_callable.definition_plans().is_empty());
    assert!(non_callable.definition_plans().iter().all(|definition| {
        surface
            .plans()
            .iter()
            .find(|plan| plan.definition_plan() == *definition)
            .is_some_and(|plan| {
                plan.definition_role() != scoop_lir::StrongDefinitionRole::CallableBody
            })
    }));

    let actual_bodies = partition.objects()[1..]
        .iter()
        .map(|object| {
            assert_eq!(object.definition_plans().len(), 1);
            let ScoopLirObjectKindV1::CallableBody(body) = object.kind() else {
                panic!("only the first object may be non-callable");
            };
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
fn emitted_object_set_owns_verified_temporary_members() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let expected_members = module.functions.len() + 1;
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let parent = tempfile::tempdir().unwrap();

    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        parent.path(),
        host_profile(),
    )
    .unwrap();

    assert_eq!(emitted.members().len(), expected_members);
    assert_eq!(emitted.target(), input.module().meta.target_profile);
    assert_eq!(
        emitted.target_selection(),
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
    );
    assert_eq!(emitted.foundation(), input.foundation());
    assert!(emitted.members().iter().all(|member| {
        std::fs::metadata(member.path())
            .is_ok_and(|metadata| metadata.len() > 0 && metadata.permissions().readonly())
    }));
    let mut non_callable_members = 0;
    for member in emitted.members() {
        match member.kind() {
            EmittedConeObjectMemberKindV1::NonCallable {
                runtime_metadata,
                digest_patches,
            } => {
                non_callable_members += 1;
                assert_eq!(
                    digest_patches.len(),
                    runtime_metadata.patch_locations().len()
                );
                assert!(!digest_patches.is_empty());
                let bytes = std::fs::read(member.path()).unwrap();
                for (materialization, location) in digest_patches
                    .iter()
                    .zip(runtime_metadata.patch_locations())
                {
                    assert_eq!(materialization.location(), *location);
                    let start = usize::try_from(materialization.checked_object_offset()).unwrap();
                    let end = start + usize::from(location.width_bytes());
                    assert!(bytes[start..end].iter().all(|byte| *byte == 0));
                }
                let mut tampered = bytes;
                let tampered_offset =
                    usize::try_from(digest_patches[0].checked_object_offset()).unwrap();
                tampered[tampered_offset] = 1;
                let tampered_path = parent.path().join("tampered-patch.o");
                std::fs::write(&tampered_path, tampered).unwrap();
                let error =
                    crate::object_materialization::resolve_digest_patch_materializations_v1(
                        &tampered_path,
                        input.module().meta.target_profile,
                        emitted.production().canonical_definitions(),
                        runtime_metadata,
                    )
                    .unwrap_err();
                assert!(error.0.contains("not provisionally zero"), "{error}");
            }
            EmittedConeObjectMemberKindV1::CallableBody { .. } => {}
        }
    }
    assert_eq!(non_callable_members, 1);
    let backing = emitted.temporary_directory().to_path_buf();
    assert!(backing.starts_with(parent.path()));
    drop(emitted);
    assert!(!backing.exists());
}

#[test]
fn emitted_callable_members_materialize_every_planned_atom_boundary() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        parent.path(),
        host_profile(),
    )
    .unwrap();
    let normalization = input
        .module()
        .meta
        .target_profile
        .contract()
        .native_symbol_normalization();
    let mut callable_count = 0;

    for member in emitted.members() {
        let EmittedConeObjectMemberKindV1::CallableBody { body } = member.kind() else {
            continue;
        };
        callable_count += 1;
        let [definition] = member.units().definition_plans() else {
            panic!("each callable member must own exactly one definition");
        };
        let plan = emitted
            .production()
            .canonical_definitions()
            .plan(*definition)
            .unwrap();
        assert_eq!(
            plan.owner().kind(),
            scoop_lir::StrongDefinitionEntityKind::CallableBody(body)
        );
        let bytes = std::fs::read(member.path()).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
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

        let duplicate_path = parent.path().join(format!("duplicate-{body}.o"));
        std::fs::write(&duplicate_path, bytes).unwrap();
        let error = crate::callable_atom_boundaries::materialize_v1(
            &duplicate_path,
            input.module().meta.target_profile,
            plan,
            body,
        )
        .unwrap_err();
        assert!(error.0.contains("already exists"), "{error}");
    }
    assert_eq!(callable_count, input.module().functions.len());
}
