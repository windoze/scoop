use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, IdentityLayer, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    ObjectDefinitionPlanOwner, OdrMemberDiscriminator, OdrMemberId, OdrMemberKey, OdrMemberRole,
    PendingIdentityValidation, PersistentCallableBodyId, RequestedConeKind,
};
use scoop_wire::{WirePath, decode_canonical, encode};

use super::*;

mod objects;
mod reader;

#[test]
fn actual_generic_library_emits_shared_odr_objects() {
    let target = resolved_target().expect("generic machine lowering requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);
    let directory = crate::workspace_root().join("tests/fixtures/m23-generic-body-consumption");
    let fixture =
        |name| std::fs::read_to_string(directory.join(format!("machine-{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-machine-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &fixture("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    std::fs::rename(
        provider_root.join("src"),
        provider_root.join("unused-source"),
    )
    .unwrap();

    let mut outputs = Vec::new();
    for (case, name, expected_bodies) in [
        ("standalone", "generic-machine-standalone", 1),
        ("control-flow", "generic-machine-control-flow", 4),
        ("combined", "generic-machine-first", 7),
        ("combined", "generic-machine-second", 7),
    ] {
        let coordinate = ConeCoordinate::new("dev.example", name, "0.1.0").unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &fixture(case));
        write_dependency_manifest(&root, name, &[&provider_coordinate]);
        let loaded = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .load_preflight()
        .unwrap();
        let request = loaded.validate().unwrap();
        let parsed = request.parse_current_sources().unwrap();
        let hir = parsed
            .lower_hir(RequestedConeKind::Library, request.protocols())
            .unwrap();
        let closure = request.dependencies().semantic();
        let selected = closure
            .project_dependency_callables_to_mir(&hir.hir)
            .unwrap();
        let mir = hir.machine_input().lower_selected_mir(selected).unwrap();
        assert!(
            mir.strong
                .materialization()
                .initialization_units()
                .is_empty()
        );
        let selected = closure
            .project_dependency_callables_to_lir(mir.strong.selected_callables())
            .unwrap();
        let dependencies = closure.layout_dependencies().collect::<Vec<_>>();
        let selected_layout = production::select_lir_dependencies(
            &mir.strong,
            [],
            &dependencies,
            target.lir_target(),
        )
        .unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(coordinate.identity().unwrap())
            .unwrap();
        hir.foundation.register_identities(&mut pending).unwrap();
        mir.strong
            .foundation()
            .register_identities(&mut pending)
            .unwrap();
        let mut coordinates = vec![coordinate];
        for (coordinate, identities) in closure.identity_inputs() {
            pending
                .register_external_graph_authorities(identities)
                .unwrap();
            coordinates.push(coordinate.clone());
        }
        let identities = pending.finish().unwrap();
        let diagnostics =
            scoop_identity::ExactTypeDiagnosticCatalog::try_new(&identities, &coordinates).unwrap();
        let (lir, _) = super::super::super::machine::lower_selected_lir(
            &mir.strong,
            &mir.public,
            &selected,
            target.lir_target(),
            &selected_layout,
            &diagnostics,
        )
        .unwrap();
        let bytes = encode(lir.foundation()).unwrap();
        let decoded = decode_canonical::<scoop_lir::DecodedLirFoundation>(&bytes).unwrap();
        let mut pending = PendingIdentityValidation::from_graph(identities);
        decoded.register_identities(&mut pending).unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        let validated = decoded
            .validate(lir.module().cone, &mut identities)
            .unwrap();
        assert_eq!(encode(&validated).unwrap(), bytes);
        let production = lir
            .build_production_section_v2(
                coordinates[0].clone(),
                &[ConeIdentity::CORE, provider_coordinate.identity().unwrap()],
                scoop_lir::EntryProductionSourceV1::Library,
                &[],
            )
            .unwrap();
        let objects = scoop_codegen::emit_object_set_v2(
            &lir,
            production.clone(),
            sysroot.path(),
            scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(objects.members().len(), lir.module().functions.len() + 1);
        let native_bodies = objects::check(&objects);
        reader::check(
            &lir,
            &production,
            &coordinates[0],
            &[ConeIdentity::CORE, provider_coordinate.identity().unwrap()],
            &dependencies,
            &selected_layout,
        );
        let generated = scoop_codegen::emit_c_bridge_object_set(
            &lir,
            sysroot.path(),
            target.c_bridge_toolchain(),
        )
        .unwrap();
        objects::verify(objects, &generated, expected_bodies);
        let replayed = scoop_lir::replay_digest_finalization_plan_v2(
            lir.foundation(),
            production.registration_production(),
            &scoop_lir::EntryProductionSourceV1::Library,
        )
        .unwrap();
        assert_eq!(
            encode(&replayed).unwrap(),
            encode(production.digest_finalization_plan()).unwrap()
        );
        let members = identities
            .records::<OdrMemberId, OdrMemberKey>(IdentityLayer::Lir, &WirePath::root())
            .unwrap();
        let plans = identities
            .records::<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>(
                IdentityLayer::Lir,
                &WirePath::root(),
            )
            .unwrap();
        let atoms = identities
            .records::<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>(
                IdentityLayer::Lir,
                &WirePath::root(),
            )
            .unwrap();
        let all_members = identities
            .closure_records::<OdrMemberId, OdrMemberKey>(&WirePath::root())
            .unwrap();
        let mut bodies = BTreeMap::<PersistentCallableBodyId, Vec<Vec<u8>>>::new();
        for function in &lir.module().functions {
            let body = &function.callable_body;
            let key = body.definition_plan_key(lir.module().cone);
            let plan = plans.iter().find(|plan| *plan.key() == key).unwrap();
            let ObjectDefinitionPlanOwner::Odr { member } = key.owner() else {
                assert_eq!(body.symbol_request().linkage(), LinkageClass::ConeStrong);
                continue;
            };
            assert_eq!(body.symbol_request().linkage(), LinkageClass::OdrWeak);
            assert!(!members.iter().any(|record| record.id() == member));
            let source = all_members
                .iter()
                .find(|record| record.id() == member)
                .unwrap();
            let registration = members
                .iter()
                .find(|record| {
                    record.key().role() == OdrMemberRole::RegistrationRecord
                        && record.key().discriminator()
                            == &OdrMemberDiscriminator::CallableBody(body.id())
                })
                .expect("every ODR body has a registration member");
            let source_group = source.key().group();
            assert_eq!(registration.key().group(), source_group);
            assert!(
                plans
                    .iter()
                    .any(|plan| { *plan.key() == ObjectDefinitionPlanKey::odr(registration.id()) })
            );
            let mut records = vec![
                native_bodies.get(&body.id()).unwrap().clone(),
                encode(body.identity_record()).unwrap(),
                encode(plan).unwrap(),
                encode(registration).unwrap(),
                encode(production.canonical_definitions().plan(plan.id()).unwrap()).unwrap(),
            ];
            for id in [member, registration.id()] {
                let node = production
                    .digest_finalization_plan()
                    .nodes()
                    .iter()
                    .find(|node| {
                        node.key().owner_and_role()
                            == scoop_identity::DigestOwnerAndRoleKey::OdrMemberDefinition(id)
                    })
                    .unwrap();
                assert!(
                    node.direct_inputs()
                        .iter()
                        .any(|input| input.kind() == scoop_identity::DigestKind::LirDefinition)
                );
                assert!(
                    node.direct_inputs()
                        .iter()
                        .any(|input| input.kind() == scoop_identity::DigestKind::ObjectDefinition)
                );
                records.push(encode(node).unwrap());
            }
            for atom in atoms.iter().filter(|atom| atom.key().plan() == plan.id()) {
                records.push(encode(atom).unwrap());
            }
            for site in function.safepoints.iter() {
                let registration = members
                    .iter()
                    .find(|record| {
                        record.key().role() == OdrMemberRole::RegistrationRecord
                            && record.key().discriminator()
                                == &OdrMemberDiscriminator::SafepointSite(site.site_id())
                    })
                    .expect("every ODR safepoint has a registration member");
                assert_eq!(registration.key().group(), source_group);
                assert!(plans.iter().any(|plan| {
                    *plan.key() == ObjectDefinitionPlanKey::odr(registration.id())
                }));
                records.push(encode(registration).unwrap());
            }
            bodies.insert(body.id(), records);
        }
        assert_eq!(bodies.len(), expected_bodies);
        if case != "standalone" {
            assert!(
                atoms
                    .iter()
                    .any(|atom| { atom.key().role() == DefinitionAtomRole::Lsda })
            );
        }
        if name != "generic-machine-second" {
            for (stage, dump) in [
                ("hir", scoop_hir::dump(&hir.hir.output().export)),
                ("mir", scoop_mir::dump(mir.strong.module())),
                ("lir", scoop_lir::dump(lir.module())),
            ] {
                let path = directory.join(format!("machine-{case}.{stage}.snap"));
                if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                    std::fs::write(&path, &dump).unwrap();
                }
                assert_eq!(dump, std::fs::read_to_string(path).unwrap());
            }
        }
        outputs.push(bodies);
    }
    for (first, second, expected_shared) in [(0, 1, 1), (1, 2, 4), (2, 3, 4)] {
        let mut shared = 0;
        for (body, records) in &outputs[first] {
            if let Some(other) = outputs[second].get(body) {
                assert!(
                    records == other,
                    "shared ODR body {body} differs between consumers {first} and {second}"
                );
                shared += 1;
            }
        }
        assert_eq!(shared, expected_shared);
    }
}
