use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin, DefinitionOwnerChain,
    NormalizedSourcePath, PackagePath, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::{ExportDefinitionSourceV1, PublicLookupAccessV1};

#[test]
fn expands_local_and_external_chains_with_shared_memoized_targets() {
    let fixture = GraphFixture::chain();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let expansions = fixture
        .table
        .expand_alias_closure(&fixture.authority, &mut meter, &WirePath::root().field(5))
        .unwrap();

    assert_eq!(expansions.entries().len(), 2);
    assert!(!expansions.is_empty());
    let first = expansions.get(fixture.first).unwrap();
    let second = expansions.get(fixture.second).unwrap();
    assert_eq!(first.alias(), fixture.first);
    assert_eq!(first.target(), &SignatureTypeKey::Nominal(fixture.nominal));
    assert!(std::ptr::eq(first.target(), second.target()));
    assert_eq!(meter.usage().decoded_nodes, 4);
    assert_eq!(meter.usage().decoded_edges, 3);
    assert_eq!(meter.usage().validation_work_units, 7);
}

#[test]
fn rejects_missing_targets_after_authorization() {
    let mut fixture = GraphFixture::chain();
    fixture.authority.records.remove(&fixture.third);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        fixture.table.expand_alias_closure(
            &fixture.authority,
            &mut meter,
            &WirePath::root().field(5),
        ),
        Err(TypeAliasExpansionError::MissingInterface {
            alias: fixture.third,
        })
    );
}

#[test]
fn rejects_unauthorized_edges_before_target_lookup() {
    let mut fixture = GraphFixture::chain();
    fixture.authority.records.remove(&fixture.third);
    fixture
        .authority
        .authorized
        .remove(&(fixture.bridge, fixture.third));
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        fixture.table.expand_alias_closure(
            &fixture.authority,
            &mut meter,
            &WirePath::root().field(5),
        ),
        Err(TypeAliasExpansionError::UnauthorizedTarget {
            source: fixture.bridge,
            target: fixture.third,
        })
    );
}

#[test]
fn reports_the_complete_cycle_with_repeated_terminal_node() {
    let fixture = GraphFixture::cycle();
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        fixture.table.expand_alias_closure(
            &fixture.authority,
            &mut meter,
            &WirePath::root().field(5),
        ),
        Err(TypeAliasExpansionError::Cycle {
            chain: vec![fixture.first, fixture.bridge, fixture.third, fixture.first,],
        })
    );
}

#[test]
fn enforces_semantic_depth_node_and_edge_budgets() {
    let fixture = GraphFixture::chain();
    let path = WirePath::root().field(5);

    let mut depth_meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 2,
        ..DecodeLimits::default()
    });
    let error = fixture
        .table
        .expand_alias_closure(&fixture.authority, &mut depth_meter, &path)
        .unwrap_err();
    assert_eq!(
        resource_kind(error),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticRecursion,
            limit: 2,
            observed: 3,
        }
    );

    let mut node_meter = BudgetMeter::new(DecodeLimits {
        decoded_nodes: 2,
        ..DecodeLimits::default()
    });
    let error = fixture
        .table
        .expand_alias_closure(&fixture.authority, &mut node_meter, &path)
        .unwrap_err();
    assert_eq!(
        resource_kind(error),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::DecodedNodes,
            limit: 2,
            observed: 3,
        }
    );

    let mut edge_meter = BudgetMeter::new(DecodeLimits {
        decoded_edges: 1,
        ..DecodeLimits::default()
    });
    let error = fixture
        .table
        .expand_alias_closure(&fixture.authority, &mut edge_meter, &path)
        .unwrap_err();
    assert_eq!(
        resource_kind(error),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::DecodedEdges,
            limit: 1,
            observed: 2,
        }
    );
}

#[test]
fn bounds_cycle_diagnostic_text_before_allocating_the_chain() {
    let fixture = GraphFixture::cycle();
    let path = WirePath::root().field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: 267,
        ..DecodeLimits::default()
    });

    let error = fixture
        .table
        .expand_alias_closure(&fixture.authority, &mut meter, &path)
        .unwrap_err();
    assert_eq!(
        resource_kind(error),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticLeafBytes,
            limit: 267,
            observed: 268,
        }
    );
}

fn resource_kind(error: TypeAliasExpansionError) -> WireErrorKind {
    match error {
        TypeAliasExpansionError::Resource(error) => error.kind().clone(),
        error => panic!("expected resource error, found {error}"),
    }
}

struct GraphFixture {
    first: PersistentTypeAliasId,
    second: PersistentTypeAliasId,
    bridge: PersistentTypeAliasId,
    third: PersistentTypeAliasId,
    nominal: PersistentTypeId,
    table: CanonicalTypeAliasInterfacesV1,
    authority: TestAuthority,
}

impl GraphFixture {
    fn chain() -> Self {
        let first = alias("First");
        let second = alias("Second");
        let bridge = alias("Bridge");
        let third = alias("Third");
        let nominal = nominal("Target");
        let table = CanonicalTypeAliasInterfacesV1::try_new(vec![
            alias_record(first, TypeAliasTargetV1::Alias(bridge)),
            alias_record(second, TypeAliasTargetV1::Alias(bridge)),
        ])
        .unwrap();
        let authority = TestAuthority {
            records: BTreeMap::from([
                (
                    bridge,
                    alias_record(bridge, TypeAliasTargetV1::Alias(third)),
                ),
                (
                    third,
                    alias_record(
                        third,
                        TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(nominal)),
                    ),
                ),
            ]),
            authorized: BTreeSet::from([(first, bridge), (second, bridge), (bridge, third)]),
        };
        Self {
            first,
            second,
            bridge,
            third,
            nominal,
            table,
            authority,
        }
    }

    fn cycle() -> Self {
        let first = alias("CycleA");
        let second = alias("UnusedRoot");
        let bridge = alias("CycleB");
        let third = alias("CycleC");
        let nominal = nominal("UnusedTarget");
        let table = CanonicalTypeAliasInterfacesV1::try_new(vec![alias_record(
            first,
            TypeAliasTargetV1::Alias(bridge),
        )])
        .unwrap();
        let authority = TestAuthority {
            records: BTreeMap::from([
                (
                    bridge,
                    alias_record(bridge, TypeAliasTargetV1::Alias(third)),
                ),
                (third, alias_record(third, TypeAliasTargetV1::Alias(first))),
            ]),
            authorized: BTreeSet::from([(first, bridge), (bridge, third), (third, first)]),
        };
        Self {
            first,
            second,
            bridge,
            third,
            nominal,
            table,
            authority,
        }
    }
}

#[derive(Default)]
struct TestAuthority {
    records: BTreeMap<PersistentTypeAliasId, TypeAliasInterfaceRecordV1>,
    authorized: BTreeSet<(PersistentTypeAliasId, PersistentTypeAliasId)>,
}

impl TypeAliasClosureAuthority for TestAuthority {
    fn external_type_alias(
        &self,
        alias: PersistentTypeAliasId,
    ) -> Option<&TypeAliasInterfaceRecordV1> {
        self.records.get(&alias)
    }

    fn is_type_alias_edge_authorized(
        &self,
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    ) -> bool {
        self.authorized.contains(&(source, target))
    }
}

fn alias_record(
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
) -> TypeAliasInterfaceRecordV1 {
    TypeAliasInterfaceRecordV1::try_new(
        alias,
        target,
        PublicLookupAccessV1::DirectOnly,
        definition_origin(),
    )
    .unwrap()
}

fn definition_origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Aliases.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn alias(name: &str) -> PersistentTypeAliasId {
    PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        site(),
        identifier(name),
    ))
    .unwrap()
}

fn nominal(name: &str) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site(),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
