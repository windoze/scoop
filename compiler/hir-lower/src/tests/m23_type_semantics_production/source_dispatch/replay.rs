use super::*;
use hir::*;
use scoop_identity::*;
use std::sync::Arc;

mod authority;

#[derive(Default)]
struct Replay {
    sources: CanonicalInterfaceSourceDispatchesV1,
    schemas: BTreeMap<PersistentExactTypeId, CanonicalInheritanceSlotSchemasV1>,
    exacts: BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    nominals: BTreeMap<SourceNominalId, Arc<SourceDeclarationKey>>,
    access: BTreeMap<SourceNominalId, DeclarationAccessSourceV1>,
    origins: BTreeMap<SourceNominalId, ExportDefinitionSourceV1>,
    slots: BTreeMap<PersistentDispatchSlotId, Arc<DispatchSlotKey>>,
    functions: BTreeMap<PersistentFunctionId, Arc<SourceDeclarationKey>>,
    accessors: BTreeMap<PersistentPropertyAccessorId, Arc<PropertyAccessorKey>>,
    properties: BTreeMap<PersistentPropertyId, Arc<SourceDeclarationKey>>,
}

#[test]
fn real_interface_source_bytes_replay_exact_slots_and_reject_an_unjustified_suppression() {
    with_source(INTERFACES, |output, _| {
        let mut identities = super::super::source_inventory::identity_closure(output);
        let source =
            CanonicalInterfaceSourceDispatchesV1::from_dependency_hir(output, &mut meter())
                .unwrap();
        let decoded: DecodedCanonicalInterfaceSourceDispatchesV1 =
            decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
        let restored = decoded.resolve(&mut identities, &mut meter()).unwrap();
        assert_eq!(restored, source);
        assert_eq!(restored.records().len(), 6);
        let inventory = project(output);
        let mut replay = Replay {
            sources: restored,
            ..Replay::default()
        };
        let export = output.output().export.module();
        let mut edges = Vec::new();
        for (id, declaration) in export.interfaces.iter() {
            let ty = export.interface_applications[declaration.self_application].canonical_type;
            let exact = export.type_identities[ty].exact().unwrap().id();
            let nominal = export.nominal_identities[id]
                .source()
                .unwrap()
                .concrete_id()
                .unwrap();
            let source_id = SourceNominalId::Concrete(nominal);
            let key: Arc<SourceDeclarationKey> = identities.canonical_key(nominal).unwrap();
            assert!(key.owners().owners().is_empty());
            replay.nominals.insert(source_id, key);
            replay
                .exacts
                .insert(exact, identities.canonical_key(exact).unwrap());
            let origin = ExportDefinitionSourceV1::new(
                export
                    .export_definition_origins
                    .get(DefinitionOriginSubject::Type(nominal))
                    .unwrap()
                    .origin()
                    .clone(),
            );
            replay.access.insert(
                source_id,
                DeclarationAccessSourceV1::try_new(
                    declaration.access.declared.into(),
                    vec![],
                    origin.clone(),
                )
                .unwrap(),
            );
            replay.origins.insert(source_id, origin);
            replay
                .schemas
                .insert(exact, inventory.get(exact).unwrap().slot_schemas().clone());
            let parents = declaration
                .parents
                .iter()
                .map(|parent| {
                    export.type_identities[export.interface_applications[*parent].canonical_type]
                        .exact()
                        .unwrap()
                        .id()
                })
                .collect();
            edges.push(
                NominalInheritanceEdgesV1::try_new(
                    exact,
                    NominalInheritanceModalityV1::Interface,
                    DirectClassBaseV1::NoClassBase,
                    parents,
                )
                .unwrap(),
            );
        }
        for source in replay.sources.records() {
            for member in source.members() {
                let key: Arc<DispatchSlotKey> = identities.canonical_key(member.slot()).unwrap();
                match key.owner() {
                    DispatchDeclarationOwner::Function(function) => {
                        replay
                            .functions
                            .insert(function, identities.canonical_key(function).unwrap());
                    }
                    DispatchDeclarationOwner::Accessor(accessor) => {
                        let accessor_key: Arc<PropertyAccessorKey> =
                            identities.canonical_key(accessor).unwrap();
                        let scoop_identity::PropertyOwner::Property(property) =
                            accessor_key.owner()
                        else {
                            panic!("interface accessor has an ordinary property owner")
                        };
                        replay
                            .properties
                            .insert(property, identities.canonical_key(property).unwrap());
                        replay.accessors.insert(accessor, accessor_key);
                    }
                }
                replay.slots.insert(member.slot(), key);
            }
        }
        {
            let graph =
                CheckedNominalInheritanceGraphV1::validate(edges.iter(), &replay, &mut meter())
                    .unwrap();
            for record in replay.sources.records() {
                graph
                    .validate_slot_schemas(record.owner(), &replay, &mut meter())
                    .unwrap();
            }
        }
        let diamond = owner(output, "Diamond");
        let record = replay.sources.get(diamond).unwrap();
        assert!(
            record
                .members()
                .iter()
                .any(|member| !member.overrides().values().is_empty())
        );
        let changed = InterfaceSourceDispatchV1::try_new(
            diamond,
            record.parents().to_vec(),
            record
                .members()
                .iter()
                .map(|member| {
                    InterfaceSourceMemberV1::new(member.slot(), CanonicalPersistentIdsV1::empty())
                })
                .collect(),
            &mut meter(),
        )
        .unwrap();
        let records = replay
            .sources
            .records()
            .iter()
            .map(|record| {
                if record.owner() == diamond {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect();
        replay.sources =
            CanonicalInterfaceSourceDispatchesV1::try_new(records, &mut meter()).unwrap();
        let graph = CheckedNominalInheritanceGraphV1::validate(edges.iter(), &replay, &mut meter())
            .unwrap();
        assert!(
            matches!(graph.validate_slot_schemas(diamond, &replay, &mut meter()), Err(InheritanceSlotSchemaSemanticError::InheritedSlots(exact)) if exact == diamond)
        );
    });
}

#[test]
fn real_interface_source_transport_is_independent_of_unrelated_arena_ids() {
    let source = with_source(INTERFACES, |output, _| {
        encode(
            &CanonicalInterfaceSourceDispatchesV1::from_dependency_hir(output, &mut meter())
                .unwrap(),
        )
        .unwrap()
    });
    let shifted = with_source(
        &format!("fun unrelated(): Int = 0\n{INTERFACES}"),
        |output, _| {
            encode(
                &CanonicalInterfaceSourceDispatchesV1::from_dependency_hir(output, &mut meter())
                    .unwrap(),
            )
            .unwrap()
        },
    );
    assert_eq!(source, shifted);
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
