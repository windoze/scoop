use super::*;
use hir::{
    CanonicalDefaultSourceProfilesV1 as Profiles, DefaultSourceProfileBindingError as Error,
    ProtectedDefaultSourceProfileSemanticAuthority,
    ProtectedDefaultWitnessSourceProfileV1 as Profile,
};
mod rejection;

mod snapshot;
mod support;
use support::with_inputs;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-source-profiles/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-source-profiles/combined.scoop"
));
const DOMAINS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-source-profiles/domain-binding.scoop"
));

#[test]
fn source_profiles_bind_real_owners_inheritance_and_all_six_target_domains() {
    for source in [STANDALONE, COMBINED, DOMAINS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters
                    .bind_default_declarations(inputs.production.templates(), &[])
                    .unwrap();
                let targets = domains
                    .bind_nominal_default_target_domains(&declarations)
                    .unwrap();
                assert!(std::ptr::eq(
                    targets.types().declarations(),
                    targets.values().declarations()
                ));
                assert!(std::ptr::eq(
                    targets.types().declarations(),
                    targets.callables().declarations()
                ));
                let profiles = targets
                    .bind_source_profiles(inputs.production.profiles())
                    .unwrap();
                assert!(std::ptr::eq(
                    profiles.domains().declarations(),
                    &declarations
                ));
                for record in profiles.profiles().records() {
                    assert_eq!(
                        profiles.default_access_profile(record.key()).unwrap(),
                        record.profile()
                    );
                }
            })
        });
    }
}

#[test]
fn inherited_defaults_preserve_distinct_publishing_and_provider_call_domains() {
    with_inputs(DOMAINS, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let declarations = parameters
                .bind_default_declarations(inputs.production.templates(), &[])
                .unwrap();
            let inherited = declarations
                .declarations()
                .iter()
                .filter(|r| r.key().owner() != r.definition_root().declaration())
                .collect::<Vec<_>>();
            assert_eq!(inherited.len(), 1);
            assert_ne!(
                inherited[0].publishing_call_domain(),
                inherited[0].direct_call_domain()
            );
            assert_eq!(
                inherited[0].publishing_call_domain(),
                &hir::DefaultSourceAccessDomainV1::universal()
            );
            assert_eq!(
                inherited[0].provider_slot_domain(),
                Some(inherited[0].direct_call_domain())
            );
            let targets = domains
                .bind_nominal_default_target_domains(&declarations)
                .unwrap();
            targets
                .bind_source_profiles(inputs.production.profiles())
                .unwrap();
        })
    });
}
