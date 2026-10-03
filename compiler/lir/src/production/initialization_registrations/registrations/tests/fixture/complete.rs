//! Complete callable/storage plans for the combined registration fixture.

use std::collections::BTreeMap;

use super::*;

impl Fixture {
    pub(in super::super) fn complete_surface(options: Options) -> Self {
        let mut fixture = Self::new(options);
        let producer = fixture.foundation.producer();
        let mut canonical = fixture.foundation.as_canonical().clone();
        let mut definitions = fixture.foundation.definition_plans().to_vec();
        let mut atoms = fixture.foundation.definition_atoms().to_vec();
        let mut symbols = fixture.foundation.symbol_requests().to_vec();
        let mut nodes = fixture
            .digests
            .nodes()
            .iter()
            .map(|node| (node.id(), node.clone()))
            .collect::<BTreeMap<_, _>>();
        let (globals, _, _) = static_globals(fixture.unit, property("value"));
        let mut layouts = Vec::new();
        let mut scans = Vec::new();
        for (_, global) in globals.iter() {
            let GlobalInit::Storage {
                identity, layout, ..
            } = &global.init
            else {
                panic!("the complete fixture has only storage globals");
            };
            let layout = layout.local().expect("the fixture defines local layouts");
            layouts.push(layout.layout_record().clone());
            scans.push(layout.scan_record().clone());
            let storage = identity.identity_record().id();
            let registration = definition_artifacts(
                producer,
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            );
            let storage_definition = definition_artifacts(
                producer,
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::StaticStorage,
            );
            let layout_definition = definition_artifacts(
                producer,
                StrongDefinitionEntity::layout(layout.layout_record().id()),
                StrongDefinitionRole::Layout,
            );
            let scan_definition = definition_artifacts(
                producer,
                StrongDefinitionEntity::scan(layout.scan_record().id()),
                StrongDefinitionRole::ScanProgram,
            );
            for added in [&storage_definition, &layout_definition, &scan_definition] {
                definitions.push(added.definition.clone());
                atoms.push(added.primary.clone());
            }
            symbols.extend([
                symbol(PersistentSymbolKey::Layout(layout.layout_record().id())),
                symbol(PersistentSymbolKey::ScanProgram(layout.scan_record().id())),
            ]);
            let registration_object = leaf(registration.primary.id());
            let storage_object = leaf(storage_definition.primary.id());
            let layout_node = with_patch(
                DigestNodeKey::layout(layout.layout_record().id()),
                Vec::new(),
                registration.definition.id(),
                DigestSemanticFieldRole::Layout,
            );
            let scan_node = with_patch(
                DigestNodeKey::scan(layout.scan_record().id()),
                Vec::new(),
                registration.definition.id(),
                DigestSemanticFieldRole::Scan,
            );
            let registration_node = with_patch(
                DigestNodeKey::strong_registration(registration.definition.id()),
                [
                    &registration_object,
                    &storage_object,
                    &layout_node,
                    &scan_node,
                ]
                .into_iter()
                .map(DigestInputRefV1::from_node)
                .collect(),
                registration.definition.id(),
                DigestSemanticFieldRole::RegistrationDefinition,
            );
            for node in [
                registration_object,
                storage_object,
                layout_node,
                scan_node,
                registration_node,
            ] {
                nodes.insert(node.id(), node);
            }
        }
        for body in fixture.foundation.callable_bodies() {
            let artifacts = callable_artifacts(producer, body.clone());
            let body_key = DigestNodeKey::object_definition(artifacts.body_definition.primary.id());
            let body_id = DigestNodeId::from_key(&body_key).unwrap();
            let original = &nodes[&body_id];
            let mut patches = original
                .patch_intents()
                .iter()
                .map(|patch| *patch.key())
                .collect::<Vec<_>>();
            patches.push(DigestPatchIntentKey::new(
                body_id,
                artifacts.registration.definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::CallableBodyDefinition,
            ));
            let body_node =
                DigestNodeV1::new(body_key, original.direct_inputs().to_vec(), patches).unwrap();
            let registration_object = leaf(artifacts.registration.primary.id());
            let registration_node = with_patch(
                DigestNodeKey::strong_registration(artifacts.registration.definition.id()),
                [&registration_object, &body_node]
                    .into_iter()
                    .map(DigestInputRefV1::from_node)
                    .collect(),
                artifacts.registration.definition.id(),
                DigestSemanticFieldRole::RegistrationDefinition,
            );
            for node in [body_node, registration_object, registration_node] {
                nodes.insert(node.id(), node);
            }
        }
        canonical.set_layouts(layouts).unwrap();
        canonical.set_scans(scans).unwrap();
        canonical.set_definition_plans(definitions).unwrap();
        canonical.set_definition_atoms(atoms).unwrap();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        fixture.foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
        fixture.digests =
            DigestFinalizationPlanV1::new(nodes.into_values().collect(), &fixture.foundation)
                .unwrap();
        fixture.identities =
            RegistrationIdentitySurfaceV1::from_foundation(&fixture.foundation, &fixture.digests)
                .unwrap();
        fixture
    }
}

fn leaf(atom: scoop_identity::ObjectDefinitionAtomId) -> DigestNodeV1 {
    DigestNodeV1::new(
        DigestNodeKey::object_definition(atom),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

fn with_patch(
    key: DigestNodeKey,
    inputs: Vec<DigestInputRefV1>,
    registration: scoop_identity::ObjectDefinitionPlanId,
    field: DigestSemanticFieldRole,
) -> DigestNodeV1 {
    let source = DigestNodeId::from_key(&key).unwrap();
    DigestNodeV1::new(
        key,
        inputs,
        vec![DigestPatchIntentKey::new(
            source,
            registration,
            DefinitionAtomRole::Primary,
            field,
        )],
    )
    .unwrap()
}
