use super::*;
use hir::{DefaultSourceAccessDomainV1 as Domain, PersistentAccessConstraintV1 as Constraint};
mod expected;
mod rejection;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/lookup-domains.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/lookup-domain-combinations.scoop"
));

fn summary(domain: &Domain) -> String {
    let names = domain
        .persistent()
        .constraints()
        .iter()
        .map(|c| match c {
            Constraint::Cone(_) => "Cone",
            Constraint::File(_) => "File",
            Constraint::LexicalOwner(_) => "LexicalOwner",
            Constraint::SubclassesOf(_) => "SubclassesOf",
        })
        .collect::<Vec<_>>();
    format!(
        "[{}]; generic {}",
        names.join(", "),
        domain.generic_subclasses().values().len()
    )
}
#[test]
fn default_source_lookup_domains_equal_sealed_lookup_for_all_declaration_roles() {
    let mut roles = BTreeSet::new();
    for source in [
        SOURCE,
        COMBINATIONS,
        super::SOURCE,
        super::OUTSIDE_ROOTS,
        super::COMBINATIONS,
    ] {
        with_sources(source, |output, fixture, required, table| {
            let export = output.output().export.module();
            let expected = expected::lookups(export);
            let foundation = fixture.bind().unwrap();
            let bound = foundation
                .bind_default_access_declarations(table, required, &mut meter())
                .unwrap();
            for record in table.records() {
                let subject = record.subject();
                roles.insert(subject.kind_tag());
                let actual = bound.source_lookup_domain(subject, &mut meter()).unwrap();
                let expected =
                    Domain::from_export_hir(export, expected[&subject], &mut meter()).unwrap();
                assert_eq!(
                    actual,
                    expected,
                    "subject={subject:?}, key={:?}, access={:?}",
                    bound.source_key(subject, &mut meter()).unwrap(),
                    record.declaration_access(),
                );
                assert!(!actual.is_empty());
            }
        });
    }
    assert_eq!(roles, (1..=8).collect());
}
#[test]
fn default_source_lookup_domain_dump_preserves_file_lexical_and_concrete_class_constraints() {
    with_sources(SOURCE, |output, fixture, required, table| {
        let foundation = fixture.bind().unwrap();
        let bound = foundation
            .bind_default_access_declarations(table, required, &mut meter())
            .unwrap();
        let export = output.output().export.module();
        let mut snapshot = String::new();
        for name in [
            "exposed",
            "local",
            "fileOnly",
            "generic",
            "FileHost.inherited",
            "Host.visible",
            "Host.localMember",
            "Host.privateMember",
            "Host.protectedMember",
            "Host.Nested.Inner.layered",
        ] {
            let domain = bound
                .source_lookup_domain(function(export, name), &mut meter())
                .unwrap();
            snapshot.push_str(&format!("{name}: {}\n", summary(&domain)));
        }
        assert_eq!(
            snapshot,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/lookup-domains.snap"
            ))
        );
        let (_, property) = export
            .properties
            .iter()
            .find(|(_, p)| p.name == "global")
            .unwrap();
        let getter = Subject::PropertyAccessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let setter = Subject::PropertyAccessor(
            export.property_accessor_identities[property.capability.setter().unwrap()].id(),
        );
        assert!(
            bound
                .source_lookup_domain(getter, &mut meter())
                .unwrap()
                .is_universal()
        );
        assert_eq!(
            summary(&bound.source_lookup_domain(setter, &mut meter()).unwrap()),
            "[Cone, File]; generic 0"
        );
    });
}
#[test]
fn default_source_lookup_domains_keep_generic_outer_regions_across_static_nested_boundaries() {
    with_sources(COMBINATIONS, |output, fixture, required, table| {
        assert!(fixture.source.entries().sources.records().is_empty());
        let foundation = fixture.bind().unwrap();
        let bound = foundation
            .bind_default_access_declarations(table, required, &mut meter())
            .unwrap();
        let export = output.output().export.module();
        for (name, expected) in [
            ("GenericHost.choose", "[Cone]; generic 1"),
            ("GenericHost.Static.exposed", "[Cone]; generic 1"),
            ("GenericHost.Static.both", "[Cone, SubclassesOf]; generic 1"),
            ("GenericHost.Static.Again.duplicate", "[Cone]; generic 1"),
            ("GenericHost.Middle.Inner.deep", "[Cone]; generic 3"),
        ] {
            let domain = bound
                .source_lookup_domain(function(export, name), &mut meter())
                .unwrap();
            assert_eq!(summary(&domain), expected, "{name}");
        }
    });
}
