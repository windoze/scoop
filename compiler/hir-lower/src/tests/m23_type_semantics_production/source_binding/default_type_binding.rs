use super::*;
use hir::{
    DefaultBodyReferenceAttachmentV1 as Attachment, DefaultBodyReferenceMetadataV1 as Metadata,
    DefaultSourceReferenceRecordV1 as Reference, DefaultSourceTypeDomainBindingError as Error,
    DefaultSourceTypeDomainsV1 as Domains,
};
mod rejection;
mod resources;
mod support;
use support::with_inputs;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-domain-binding.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-domain-binding-combinations.scoop"
));

#[test]
fn default_type_binding_joins_artifact_occurrences_and_original_provider_scopes() {
    let mut snapshot = String::new();
    for (name, source) in [("direct", SOURCE), ("combinations", COMBINATIONS)] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let bound = parameters
                    .bind_default_declarations(&inputs.templates, &[], &mut meter())
                    .unwrap();
                let proof = domains
                    .bind_nominal_default_type_domains(&bound, &mut meter())
                    .unwrap();
                assert!(std::ptr::eq(proof.declarations(), &bound));
                let counts = inspect(&proof, domains);
                snapshot.push_str(&format!(
                    "{name}: templates {}; inherited {}; types {}; local signatures {}; generic constraints {}\n",
                    counts[0], counts[1], counts[2], counts[3], counts[4]
                ));
            });
        });
    }
    assert_eq!(
        snapshot,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/type-domain-binding.snap"
        ))
    );
}

fn inspect(
    proof: &hir::BoundNominalDefaultTypeDomainsV1<'_, '_, '_, '_, '_, '_>,
    domains: &Domains<'_, '_, '_, '_>,
) -> [usize; 5] {
    let mut counts = [0; 5];
    for contract in proof.declarations().declarations() {
        counts[0] += 1;
        if contract.key().owner() != contract.definition_root().declaration() {
            counts[1] += 1;
            assert_ne!(contract.owner_binders(), contract.provider_binders());
        }
        for occurrence in contract.references().occurrences() {
            let Reference::Type(reference) = occurrence.source() else {
                continue;
            };
            counts[2] += 1;
            counts[4] += reference
                .witness()
                .target_domain()
                .generic_subclasses()
                .values()
                .len();
            if let Attachment::Metadata(Metadata::LocalFunction(function)) =
                occurrence.body().attachment
            {
                counts[3] += 1;
                if matches!(
                    function.declaration(),
                    scoop_identity::CallableTemplateOrigin::GenericFunction(_)
                ) {
                    // A local signature must not borrow the provider frame as its own.
                    let result = domains.type_source_domain(
                        reference.target(),
                        &contract.provider_binders().signature_scope(),
                        &mut meter(),
                    );
                    assert!(matches!(
                        result,
                        Err(hir::DefaultSourceTypeDomainError::Binder(_))
                    ));
                }
            }
        }
    }
    assert!(counts[0] > 0 && counts[2] > 0 && counts[3] > 0);
    counts
}
