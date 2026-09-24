//! Complete shared defaults bind the restricted projection after wire decoding.

use super::*;
use hir::{
    CanonicalBooleanV1, CanonicalProtectedDefaultTemplatesV1, ExportDefaultBodyV1,
    ProtectedDefaultAccessWitnessViewV1, ProtectedDefaultReferenceSetV1,
    ProtectedDefaultTemplateV1,
};
use std::fmt::Write;

mod body;
mod references;
mod witnesses;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    for case in ["source-defaults", "default-combinations"] {
        let root = sysroot.join(case);
        write_manifest_cone(
            &root,
            "dev.example",
            case,
            "library",
            &std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap(),
        );
        let provider = lower(sysroot, target, &root, vec![], &[core]);
        let checked = provider.check(&[core]).unwrap();
        checked
            .with_inheritance_graph(&[core], &mut meter(), |_, _| ())
            .unwrap();
        body::check(checked, core);
        references::check(checked, core);
        witnesses::check(checked, core);
        let mut snapshot = String::new();
        let mut counts = [0; 6];
        let mut generic = 0;
        let mut slots = 0;
        for template in checked.section().protected_defaults().records() {
            writeln!(snapshot, "default {:?}", template.key()).unwrap();
            let r = template.references();
            for (total, count) in counts.iter_mut().zip([
                r.callables().len(),
                r.constructors().len(),
                r.types().len(),
                r.globals().len(),
                r.singleton_values().len(),
                r.fields().len(),
            ]) {
                *total += count;
            }
            for reference in r.callables() {
                write!(
                    snapshot,
                    "  callable {:?} uses={:?}",
                    reference.target(),
                    reference.uses().values()
                )
                .unwrap();
                match reference.witness().view() {
                    ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { .. } => {
                        generic += 1;
                        writeln!(snapshot, " generic-source").unwrap();
                    }
                    ProtectedDefaultAccessWitnessViewV1::ParamFree(witness) => {
                        slots += witness.slot_call_domains().records().len();
                        writeln!(
                            snapshot,
                            " direct={} slots={} target={}",
                            witness.direct_call_domain().domain().constraints().len(),
                            witness.slot_call_domains().records().len(),
                            witness.target_domain().domain().constraints().len()
                        )
                        .unwrap();
                    }
                }
            }
            writeln!(
                snapshot,
                "  constructors={} types={} globals={} singletons={} fields={}",
                r.constructors().len(),
                r.types().len(),
                r.globals().len(),
                r.singleton_values().len(),
                r.fields().len()
            )
            .unwrap();
        }
        assert!(slots > 0);
        if case == "default-combinations" {
            assert!(generic > 0);
            assert!(counts.into_iter().all(|count| count > 0), "{counts:?}");
        }
        let golden = fixtures.join(format!("{case}.snap"));
        if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
            std::fs::write(&golden, &snapshot).unwrap();
        }
        assert_eq!(snapshot, std::fs::read_to_string(golden).unwrap());
    }
}

fn replace(
    source: &ProtectedDefaultTemplateV1,
    body: ExportDefaultBodyV1,
    references: ProtectedDefaultReferenceSetV1,
    allows_suspend: CanonicalBooleanV1,
) -> ProtectedDefaultTemplateV1 {
    ProtectedDefaultTemplateV1::try_new(
        source.key(),
        source.definition_root(),
        source.definition_path().clone(),
        source.locals().clone(),
        body,
        source.result().clone(),
        allows_suspend,
        source.type_parameters().clone(),
        source.receiver().clone(),
        source.value_parameters().clone(),
        references,
        source.definition_origin().clone(),
    )
    .unwrap()
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    replacement: ProtectedDefaultTemplateV1,
) -> Error {
    let source = checked.section();
    let mut defaults = source.protected_defaults().records().to_vec();
    let index = defaults
        .iter()
        .position(|template| template.key() == replacement.key())
        .unwrap();
    defaults[index] = replacement;
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        source.inheritance().clone(),
        source.protected_declarations().clone(),
        source.protected_source_interfaces().clone(),
        CanonicalProtectedDefaultTemplatesV1::try_new(defaults).unwrap(),
        source.definition_sources().clone(),
        source.selected().clone(),
    );
    reject_section(checked, core, &candidate)
}

fn reject_section(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    candidate: &CrossConeTypeSemanticsSectionV1,
) -> Error {
    candidate
        .validate_shared_foundation(checked.metadata(), &[core], &mut meter())
        .unwrap()
        .with_inheritance_graph(&[core], &mut meter(), |_, _| ())
        .expect_err("restricted defaults must agree with shared declarations and body occurrences")
}
