use super::source_dispatch::with_hir_source;
use super::source_inventory::identity_closure;
use super::*;
use hir::{
    DecodedDefaultSourceAccessDomainV1 as DecodedDomain,
    DecodedDefaultSourceAccessWitnessV1 as DecodedWitness,
};
use hir::{DefaultSourceAccessDomainV1 as Domain, DefaultSourceAccessWitnessV1 as Witness};
use scoop_wire::{decode_canonical, encode};

mod budgets;
mod rejection;
mod wire;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access.scoop"
));
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn function(export: &hir::ExportHir, name: &str) -> hir::ExportParameterOwner {
    hir::ExportParameterOwner::Function(
        export
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0,
    )
}
fn class(export: &hir::ExportHir, name: &str) -> hir::ClassId {
    export
        .classes
        .iter()
        .find(|(_, c)| c.name == name)
        .unwrap()
        .0
}
fn witnesses(
    references: &hir::ExportDefaultReferences,
) -> impl Iterator<Item = &hir::ExportDefaultAccessWitness> {
    references
        .callables
        .iter()
        .map(|r| &r.witness)
        .chain(references.constructors.iter().map(|r| &r.witness))
        .chain(references.types.iter().map(|r| &r.witness))
        .chain(references.globals.iter().map(|r| &r.witness))
        .chain(references.singleton_values.iter().map(|r| &r.witness))
        .chain(references.fields.iter().map(|r| &r.witness))
}
fn domain_summary(domain: &Domain) -> String {
    if domain.is_empty() {
        return "empty".into();
    }
    let regions = domain
        .persistent()
        .constraints()
        .iter()
        .map(|constraint| match constraint {
            hir::PersistentAccessConstraintV1::Cone(_) => "cone",
            hir::PersistentAccessConstraintV1::File(_) => "file",
            hir::PersistentAccessConstraintV1::LexicalOwner(_) => "lexical",
            hir::PersistentAccessConstraintV1::SubclassesOf(_) => "subclasses",
        })
        .collect::<Vec<_>>()
        .join("+");
    format!(
        "{};generic={}",
        if regions.is_empty() { "all" } else { &regions },
        domain.generic_subclasses().values().len()
    )
}

#[test]
fn actual_default_witnesses_round_trip_all_source_regions_without_public_lookup() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let mut identities = identity_closure(output);
        let mut summary = BTreeSet::new();
        for name in [
            "fileDefault",
            "publicDefault",
            "LocalBase.choose",
            "LocalBase.hidden",
            "LocalChild.choose",
            "Generic.choose",
            "Generic.Static.literal",
        ] {
            let body = hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
                output,
                function(export, name),
                0,
                &mut meter(),
            )
            .unwrap();
            let mut count = 0;
            for source in witnesses(body.source_references()) {
                let snapshot = Witness::from_export_hir(export, source, &mut meter()).unwrap();
                assert_eq!(snapshot.owner(), body.definition_root().declaration());
                let bytes = encode(&snapshot).unwrap();
                let decoded: DecodedWitness =
                    decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                assert_eq!(encode(&decoded).unwrap(), bytes);
                let restored = decoded.resolve(&mut identities, &mut meter()).unwrap();
                assert_eq!(restored, snapshot);
                assert_eq!(encode(&restored).unwrap(), bytes);
                let slot = match snapshot.slot_call_domain() {
                    hir::OptionalDefaultSourceSlotDomainV1::Absent => "absent".into(),
                    hir::OptionalDefaultSourceSlotDomainV1::Present(domain) => {
                        domain_summary(domain)
                    }
                };
                summary.insert(format!(
                    "{name}: direct={}, slot={slot}, target={}",
                    domain_summary(snapshot.direct_call_domain()),
                    domain_summary(snapshot.target_domain())
                ));
                if name == "Generic.choose" {
                    assert_eq!(
                        snapshot
                            .direct_call_domain()
                            .generic_subclasses()
                            .values()
                            .len(),
                        1
                    );
                    assert!(!snapshot.direct_call_domain().is_universal());
                }
                if name == "Generic.Static.literal" {
                    assert!(
                        snapshot
                            .direct_call_domain()
                            .generic_subclasses()
                            .is_empty()
                    );
                }
                if name == "LocalChild.choose" {
                    assert_ne!(snapshot.owner(), body.owner());
                }
                count += 1;
            }
            assert!(count > 0, "{name} must retain actual references");
        }
        assert_eq!(
            summary.into_iter().collect::<Vec<_>>().join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/access.snap"
            ))
        );
    });
}

#[test]
fn mixed_source_domains_partition_only_actual_generic_class_constraints() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let generic = class(export, "Generic");
        let other = class(export, "Other");
        let concrete = class(export, "LocalBase");
        let source = hir::AccessDomain::from_constraints([
            hir::AccessConstraint::SubclassesOf(other),
            hir::AccessConstraint::SubclassesOf(concrete),
            hir::AccessConstraint::LexicalOwner(hir::VisibilityOwner::Class(generic)),
            hir::AccessConstraint::SubclassesOf(generic),
            hir::AccessConstraint::Cone(export.cone),
            hir::AccessConstraint::File(export.source_files[0].identity.clone()),
        ]);
        let domain = Domain::from_export_hir(export, &source, &mut meter()).unwrap();
        assert_eq!(domain.generic_subclasses().values().len(), 2);
        assert_eq!(domain.persistent().constraints().len(), 4);
        let decoded: DecodedDomain =
            decode_canonical(&encode(&domain).unwrap(), DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded
                .resolve(&mut identity_closure(output), &mut meter())
                .unwrap(),
            domain
        );
        assert_eq!(
            Domain::from_export_hir(export, &hir::AccessDomain::empty(), &mut meter()).unwrap(),
            Domain::empty()
        );
        assert_eq!(
            Domain::from_export_hir(export, &hir::AccessDomain::universal(), &mut meter()).unwrap(),
            Domain::universal()
        );
    });
}
