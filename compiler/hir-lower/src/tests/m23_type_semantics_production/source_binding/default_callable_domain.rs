use super::*;
use hir::{
    DefaultSourceCallableDomainBindingError as Error, DefaultSourceDomainsV1 as Domains,
    DefaultSourceReferenceRecordV1 as Reference, ExportDefaultCallableTargetV1 as Target,
};
mod dependencies;
mod rejection;
mod repeated;
mod replacement;

pub(super) mod support;
use support::with_inputs;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/callable-domain-binding.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/callable-domain-binding-combinations.scoop"
));

#[test]
fn callable_domains_replay_declarations_nested_targets_equality_and_inherited_defaults() {
    let mut snapshot = String::new();
    for (name, source) in [("standalone", SOURCE), ("combinations", COMBINATIONS)] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters.bind_default_declarations(&inputs.templates, &[]).unwrap();
                let proof = domains.bind_nominal_default_callable_domains(&declarations).unwrap();
                assert!(std::ptr::eq(proof.declarations(), &declarations));
                let mut counts = [0; 10];
                let mut constrained = 0;
                for declaration in proof.declarations().declarations() {
                    counts[0] += 1;
                    counts[1] += usize::from(declaration.key().owner() != declaration.definition_root().declaration());
                    for occurrence in declaration.references().occurrences() {
                        let Reference::Callable(record) = occurrence.source() else { continue };
                        counts[kind(record.target())] += 1;
                        constrained += usize::from(record.witness().target_domain() != &hir::DefaultSourceAccessDomainV1::universal());
                    }
                }
                snapshot.push_str(&format!("{name}: defaults {}; inherited {}; callable {}; bound {}; equality {}; local {}; lambda {}; anonymous {}; reference {}; address {}; constrained {}\n",
                    counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7], counts[8], counts[9], constrained));
            });
        });
    }
    assert_eq!(
        snapshot,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/callable-domain-binding.snap"
        ))
    );
}

fn kind(target: &Target) -> usize {
    match target {
        Target::Callable(_) => 2,
        Target::Bound(_) => 3,
        Target::DerivedEquality { .. } => 4,
        Target::LocalFunction { .. } => 5,
        Target::Lambda { .. } => 6,
        Target::AnonymousFunction { .. } => 7,
        Target::CallableReference { .. } => 8,
        Target::FunctionAddress { .. } => 9,
    }
}
