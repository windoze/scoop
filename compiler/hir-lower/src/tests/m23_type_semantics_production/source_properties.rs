use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalInheritanceSourcePropertiesV1 as Table, NominalSupportPropertyPayloadV1 as Payload,
};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject};
use scoop_wire::{decode_canonical, encode};

mod contracts;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
));
const DIRECT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/property-direct.scoop"
));

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn table(output: &hir::OrdinaryHirOutput) -> Table {
    Table::from_ordinary_hir(output, &mut meter()).unwrap()
}

#[test]
fn property_sources_preserve_logical_types_setter_origins_and_dispatch_relations() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        contracts::verify(output, &table);
        assert_eq!(table.records().len(), 9);
        assert_eq!(
            contracts::render(output, &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/properties.contracts.snap"
            ))
        );
    });
}

#[test]
fn public_property_with_protected_setter_keeps_its_original_visibility() {
    with_source(DIRECT, |output, _| {
        let table = table(output);
        assert_eq!(table.records().len(), 1);
        let record = &table.records()[0];
        assert_eq!(
            record.declaration_access().declared_visibility(),
            hir::DeclaredVisibilityV1::Public
        );
        let Payload::Runtime { interface } = record.payload() else {
            panic!("runtime property");
        };
        let hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = interface.mutability()
        else {
            panic!("mutable property");
        };
        assert_eq!(
            setter_access.declared_visibility(),
            hir::DeclaredVisibilityV1::Protected
        );
        let public =
            hir::CanonicalPropertyInterfacesV1::from_export_hir(output.output().export.module())
                .unwrap();
        let public = public
            .get(scoop_identity::PropertyOwner::Property(
                record.declaration(),
            ))
            .unwrap();
        assert_eq!(
            public.capability().setter_access(),
            Some(hir::PropertySetterPublicAccessV1::Restricted)
        );
        let protected = hir::CanonicalInheritanceSourceProtectedCallablesV1::from_ordinary_hir(
            output,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(protected.records().len(), 1);
        assert_eq!(
            protected.records()[0].declaration(),
            CallableTemplateOrigin::Accessor(*setter)
        );
        assert_eq!(protected.records()[0].declaration_access(), setter_access);
    });
}

#[test]
fn property_source_inventory_covers_protected_and_dispatch_accessors_exactly() {
    for source in [
        SOURCE,
        DIRECT,
        source_dispatch::VIRTUAL,
        source_dispatch::INTERFACES,
    ] {
        with_source(source, |output, _| {
            let table = table(output);
            contracts::verify(output, &table);
            let protected = hir::CanonicalInheritanceSourceProtectedCallablesV1::from_ordinary_hir(
                output,
                &mut meter(),
            )
            .unwrap();
            let dispatched =
                hir::CanonicalInheritanceSourceCallablesV1::from_ordinary_hir(output, &mut meter())
                    .unwrap();
            let mut required = protected
                .records()
                .iter()
                .filter_map(|record| match record.declaration() {
                    CallableTemplateOrigin::Accessor(id) => Some(id),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            for record in dispatched.records() {
                match record.declaration() {
                    hir::InheritanceCallableDeclarationV1::Getter(id)
                    | hir::InheritanceCallableDeclarationV1::Setter(id) => {
                        required.insert(id);
                    }
                    hir::InheritanceCallableDeclarationV1::Function(_) => continue,
                }
            }
            let export = output.output().export.module();
            let expected = export
                .properties
                .iter()
                .filter_map(|(id, property)| {
                    let getter =
                        export.property_accessor_identities[property.capability.getter()].id();
                    let setter_required = property.capability.setter().is_some_and(|setter| {
                        required.contains(&export.property_accessor_identities[setter].id())
                    });
                    if required.contains(&getter) || setter_required {
                        let hir::HirPropertyIdentity::Ordinary(identity) =
                            &export.property_identities[id]
                        else {
                            panic!("nominal property");
                        };
                        Some(identity.id())
                    } else {
                        None
                    }
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                table
                    .records()
                    .iter()
                    .map(|record| record.declaration())
                    .collect::<BTreeSet<_>>(),
                expected
            );
        });
    }
}

#[test]
fn property_source_bytes_are_deterministic_and_replay_without_candidates() {
    let first = with_source(SOURCE, |output, _| {
        let table = table(output);
        let bytes = encode(&table).unwrap();
        let decoded: hir::DecodedCanonicalInheritanceSourcePropertiesV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let mut identities = source_inventory::identity_closure(output);
        assert_eq!(
            decoded.resolve(&mut identities, &mut meter()).unwrap(),
            table
        );
        bytes
    });
    assert_eq!(
        first,
        with_source(SOURCE, |output, _| encode(&table(output)).unwrap())
    );
}

#[test]
fn property_source_projection_obeys_shared_resources() {
    with_source(SOURCE, |output, _| {
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                Table::from_ordinary_hir(output, &mut BudgetMeter::new(limits)),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
    });
}
