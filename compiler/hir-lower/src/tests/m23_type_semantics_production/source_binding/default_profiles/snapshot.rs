use super::super::super::source_default_table::{declaration, owner_name};
use super::*;

#[test]
fn bound_default_profile_dump_preserves_each_parameter_and_inherited_source_domain() {
    with_inputs(DOMAINS, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let declarations = parameters
                .bind_default_declarations(inputs.production.templates(), &[], &mut meter())
                .unwrap();
            let profiles = domains
                .bind_nominal_default_target_domains(&declarations, &mut meter())
                .unwrap()
                .bind_source_profiles(inputs.production.profiles(), &mut meter())
                .unwrap();
            let export = inputs.output.output().export.module();
            let labels = export
                .source_parameter_interfaces
                .iter()
                .map(|p| (declaration(export, p.owner), owner_name(export, p.owner)))
                .collect::<std::collections::BTreeMap<_, _>>();
            let mut lines = declarations
                .declarations()
                .iter()
                .map(|d| {
                    format!(
                        "{}[{}]: {:?} inherited={} publishing={}",
                        labels[&d.key().owner()],
                        d.key().parameter_position(),
                        profiles
                            .default_access_profile(d.key(), &mut meter())
                            .unwrap(),
                        d.key().owner() != d.definition_root().declaration(),
                        if d.publishing_call_domain() == d.direct_call_domain() {
                            "provider"
                        } else {
                            "distinct"
                        },
                    )
                })
                .collect::<Vec<_>>();
            lines.sort();
            assert_eq!(
                lines.join("\n") + "\n",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-default-source-profiles/domain-binding.snap"
                ))
            );
        });
    });
}
