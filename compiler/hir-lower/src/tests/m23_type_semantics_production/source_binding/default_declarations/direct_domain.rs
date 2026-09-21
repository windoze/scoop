use super::*;
use hir::{DefaultSourceAccessDomainV1 as Domain, PersistentAccessConstraintV1 as Constraint};
mod corruption;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/direct-call-domains.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/direct-call-domain-combinations.scoop"
));

#[test]
fn default_direct_domains_replay_visibility_owners_variants_and_reference_free_bodies() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let bound = parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for name in [
                "DirectDomainHost.exposed",
                "DirectDomainHost.local",
                "DirectDomainHost.hidden",
                "DirectDomainHost.inherited",
                "DirectDomainHost.Entry.Local.duplicate",
                "FileDomainBase.fileValue",
            ] {
                let contract = bound
                    .declaration(key(output, name, 0), &mut meter())
                    .unwrap();
                snapshot.push_str(&format!(
                    "{name}: {}\n",
                    summary(contract.direct_call_domain())
                ));
                assert!(
                    contract.references().occurrences().iter().all(|o| o
                        .source()
                        .witness()
                        .direct_call_domain()
                        == contract.direct_call_domain())
                );
            }
            let variant = bound
                .declarations()
                .iter()
                .find(|d| {
                    matches!(
                        d.key().owner(),
                        CallableTemplateOrigin::VariantConstructor(_)
                    )
                })
                .unwrap();
            assert_eq!(
                summary(variant.direct_call_domain()),
                "[LexicalOwner, SubclassesOf]; generic 0"
            );
            let constructor = bound
                .declarations()
                .iter()
                .find(|d| matches!(d.key().owner(), CallableTemplateOrigin::Constructor(_)))
                .unwrap();
            assert!(constructor.references().occurrences().is_empty());
            assert_eq!(summary(constructor.direct_call_domain()), "[]; generic 1");
            snapshot.push_str(&format!(
                "private enum variant: {}\nreference-free constructor: {}\n",
                summary(variant.direct_call_domain()),
                summary(constructor.direct_call_domain())
            ));
            assert_eq!(
                snapshot,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-source-defaults/direct-call-domains.snap"
                ))
            );
        });
    });
}

#[test]
fn default_direct_domains_keep_original_provider_and_full_static_nested_visibility() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let bound = parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            let base = bound
                .declaration(key(output, "DirectDomainBase.inherited", 0), &mut meter())
                .unwrap();
            let child = bound
                .declaration(key(output, "DirectDomainChild.inherited", 0), &mut meter())
                .unwrap();
            assert_ne!(base.key().owner(), child.key().owner());
            assert_eq!(base.direct_call_domain(), child.direct_call_domain());
            assert_eq!(
                summary(child.direct_call_domain()),
                "[SubclassesOf]; generic 0"
            );
            for (name, expected) in [
                ("DirectGenericDomain.own", "[]; generic 1"),
                ("DirectGenericDomain.Static.exposed", "[]; generic 1"),
                (
                    "DirectGenericDomain.Static.both",
                    "[SubclassesOf]; generic 1",
                ),
            ] {
                let contract = bound
                    .declaration(key(output, name, 0), &mut meter())
                    .unwrap();
                assert_eq!(summary(contract.direct_call_domain()), expected);
                if name.contains(".Static.") {
                    assert_eq!(contract.provider_binders().binder_arity(), 0);
                }
            }
        });
    });
}
fn summary(domain: &Domain) -> String {
    let kinds = domain
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
        kinds.join(", "),
        domain.generic_subclasses().values().len()
    )
}
