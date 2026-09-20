use super::super::source_dispatch::{INTERFACES, VIRTUAL, with_source};
use super::*;
use hir::{InheritanceDispatchBindingError as Error, InheritanceSlotSchemaSemanticAuthority};

mod inventories;
mod ownership;
mod slots;

const CALLABLES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/callables.scoop"
));
const SELECTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/selections.scoop"
));

#[derive(Clone)]
pub(super) struct Sources {
    pub(super) inventory: hir::CanonicalSourceInheritanceInventoriesV1,
    pub(super) interfaces: hir::CanonicalInterfaceSourceDispatchesV1,
    pub(super) selections: hir::CanonicalInheritanceSourceSlotSelectionsV1,
    pub(super) callables: hir::CanonicalInheritanceSourceCallablesV1,
}

impl Sources {
    pub(super) fn from_output(output: &hir::OrdinaryHirOutput<'_>, fixture: &mut Fixture) -> Self {
        macro_rules! restore {
            ($canonical:ty, $decoded:ty) => {{
                let source = <$canonical>::from_ordinary_hir(output, &mut meter()).unwrap();
                let bytes = encode(&source).unwrap();
                let decoded: $decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                let restored = decoded
                    .resolve(&mut fixture.identities, &mut meter())
                    .unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        Self {
            inventory: restore!(
                hir::CanonicalSourceInheritanceInventoriesV1,
                hir::DecodedCanonicalSourceInheritanceInventoriesV1
            ),
            interfaces: restore!(
                hir::CanonicalInterfaceSourceDispatchesV1,
                hir::DecodedCanonicalInterfaceSourceDispatchesV1
            ),
            selections: restore!(
                hir::CanonicalInheritanceSourceSlotSelectionsV1,
                hir::DecodedCanonicalInheritanceSourceSlotSelectionsV1
            ),
            callables: restore!(
                hir::CanonicalInheritanceSourceCallablesV1,
                hir::DecodedCanonicalInheritanceSourceCallablesV1
            ),
        }
    }

    pub(super) fn bind<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
        meter: &mut BudgetMeter,
    ) -> Result<hir::BoundInheritanceDispatchSourcesV1<'a, 'f>, Error> {
        foundation.bind_inheritance_dispatch_sources(
            &self.inventory,
            &self.interfaces,
            &self.selections,
            &self.callables,
            meter,
        )
    }
}

#[test]
fn byte_restored_dispatch_sources_replay_against_their_own_foundation() {
    for source in [VIRTUAL, INTERFACES, CALLABLES, SELECTIONS] {
        with_source(source, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let bound = sources.bind(&foundation, &mut meter()).unwrap();
            let entries = fixture.source.entries();
            let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
                entries.local_inheritance_edges.records().iter(),
                entries.source_roots.values().iter().copied(),
                &foundation,
                &mut meter(),
            )
            .unwrap();
            assert_eq!(bound.inventory(), &sources.inventory);
            for owner in sources.inventory.owners().values() {
                let checked = graph
                    .validate_slot_schemas(*owner, &bound, &mut meter())
                    .unwrap();
                assert_eq!(
                    checked.schemas(),
                    sources.inventory.get(*owner).unwrap().slot_schemas()
                );
            }
            for record in sources.callables.records() {
                let facts = bound.callable(record.declaration()).unwrap();
                assert_eq!(facts.signature, record.signature());
                assert_eq!(facts.modality, record.modality());
                assert_eq!(facts.declaration_access, record.declaration_access());
            }
            for record in sources.selections.records() {
                assert_eq!(
                    bound.selection(record.owner(), record.slot()).unwrap(),
                    record.selection()
                );
            }
        });
    }
}

#[test]
fn dispatch_binding_is_budgeted_before_it_publishes_source_authority() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                sources.bind(&foundation, &mut BudgetMeter::new(limits)),
                Err(Error::Resource(_))
            ));
        }
    });
}
