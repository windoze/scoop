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
            let layout_node = DigestNodeV1::new(
                DigestNodeKey::layout(layout.layout_record().id()),
                Vec::new(),
            )
            .unwrap();
            let scan_node =
                DigestNodeV1::new(DigestNodeKey::scan(layout.scan_record().id()), Vec::new())
                    .unwrap();
            for node in [layout_node, scan_node] {
                nodes.insert(node.id(), node);
            }
        }
        let image_key = DigestNodeKey::runtime_image(producer);
        let image_id = DigestNodeId::from_key(&image_key).unwrap();
        let inputs = nodes
            .values()
            .filter(|node| node.id() != image_id)
            .map(DigestInputRefV1::from_node)
            .collect();
        let image = DigestNodeV1::new(image_key, inputs).unwrap();
        nodes.insert(image_id, image);
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
            RegistrationIdentitySurfaceV1::from_foundation(&fixture.foundation).unwrap();
        fixture
    }
}
