use super::*;
use hir::{
    DefaultSourceDomainsV1 as Domains, DefaultSourceReferenceRecordV1 as Reference,
    DefaultSourceValueDomainBindingError as Error, DefaultSourceValueTargetV1 as Target,
};
mod dependencies;
mod rejection;

pub(super) mod support;
use support::with_inputs;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/value-domain-binding.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/value-domain-binding-combinations.scoop"
));

fn target(reference: Reference<'_>) -> Option<Target<'_>> {
    match reference {
        Reference::Constructor(r) => Some(Target::Constructor(r.target())),
        Reference::Global(r) => Some(Target::Global(*r.target())),
        Reference::Singleton(r) => Some(Target::Singleton(*r.target())),
        Reference::Field(r) => Some(Target::Field(r.target())),
        Reference::Callable(_) | Reference::Type(_) => None,
    }
}

#[test]
fn default_value_domains_join_actual_constructor_storage_and_inherited_occurrences() {
    let mut snapshot = String::new();
    for (name, source) in [("standalone", SOURCE), ("combinations", COMBINATIONS)] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters
                    .bind_default_declarations(&inputs.templates, &[])
                    .unwrap();
                let proof = domains.bind_nominal_default_value_domains(&declarations).unwrap();
                assert!(std::ptr::eq(proof.declarations(), &declarations));
                let mut counts = [0; 7];
                for declaration in proof.declarations().declarations() {
                    counts[0] += 1;
                    counts[1] += usize::from(declaration.key().owner() != declaration.definition_root().declaration());
                    for occurrence in declaration.references().occurrences() {
                        let record = occurrence.source();
                        let Some(target) = target(record) else { continue };
                        let index = match target {
                            Target::Constructor(_) => 2,
                            Target::Global(_) => 3,
                            Target::Singleton(_) => 4,
                            Target::Field(_) => 5,
                        };
                        counts[6] += usize::from(!record.witness().target_domain().generic_subclasses().is_empty());
                        counts[index] += 1;
                        let actual = domains.value_source_domain(target).unwrap();
                        assert_eq!(&actual, record.witness().target_domain());
                    }
                }
                snapshot.push_str(&format!("{name}: defaults {}; inherited {}; constructors {}; globals {}; singletons {}; fields {}; generic occurrences {}\n", counts[0],counts[1],counts[2],counts[3],counts[4],counts[5],counts[6]));
            });
        });
    }
    assert_eq!(
        snapshot,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/value-domain-binding.snap"
        ))
    );
}
