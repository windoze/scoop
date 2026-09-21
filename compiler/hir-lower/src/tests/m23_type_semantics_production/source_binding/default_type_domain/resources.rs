use super::*;

#[test]
fn default_type_domains_replay_with_one_shared_resource_budget() {
    with_local(|core, fixture, table, required, templates| {
        let foundation = fixture.bind().unwrap();
        let declarations = foundation
            .bind_default_access_declarations(table, required, &mut meter())
            .unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let core_types = inputs.protocols().fundamental_types();
        let domains = Domains::new(
            &declarations,
            &[],
            &core.foundation,
            core_types,
            &mut meter(),
        )
        .unwrap();
        for (name, template) in templates {
            let scope = scope(name);
            let mut measured = meter();
            domains
                .type_source_domain(template.result(), &scope, &mut measured)
                .unwrap();
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units * 2 - 1,
                ..DecodeLimits::default()
            });
            domains
                .type_source_domain(template.result(), &scope, &mut shared)
                .unwrap();
            assert!(
                matches!(
                    domains.type_source_domain(template.result(), &scope, &mut shared),
                    Err(Error::Resource(_))
                ),
                "{name}"
            );
        }
        let ty = templates
            .iter()
            .find(|(name, _)| *name == "Enclosing.combined")
            .unwrap()
            .1
            .result();
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                matches!(
                    domains.type_source_domain(
                        ty,
                        &scope("Enclosing.combined"),
                        &mut BudgetMeter::new(limits)
                    ),
                    Err(Error::Resource(_))
                ),
                "{limits:?}"
            );
        }
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                matches!(
                    Domains::new(
                        &declarations,
                        &[],
                        &core.foundation,
                        core_types,
                        &mut BudgetMeter::new(limits)
                    ),
                    Err(Error::Resource(_))
                ),
                "{limits:?}"
            );
        }
    });
}
