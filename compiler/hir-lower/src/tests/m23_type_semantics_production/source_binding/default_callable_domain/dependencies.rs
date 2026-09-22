use super::super::default_type_domain::dependencies::support::artifacts;
use super::*;
use hir::DefaultCallableDeclarationV1 as Declaration;
use std::collections::BTreeMap;

#[test]
fn callable_domains_route_same_named_functions_generics_and_accessors_to_actual_artifacts() {
    let core = trusted_core();
    let (artifacts, identities) = artifacts(&core);
    let foundations = artifacts.each_ref().map(|a| {
        a.source
            .bind_to_foundation(&a.foundation, &identities, &mut meter())
            .unwrap()
    });
    let access = std::array::from_fn::<_, 3, _>(|i| {
        foundations[i]
            .bind_default_access_declarations(
                &artifacts[i].table,
                &artifacts[i].required,
                &mut meter(),
            )
            .unwrap()
    });
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let core_types = core_inputs.protocols().fundamental_types();
    let mut dependencies = [&access[1], &access[2]];
    dependencies.sort_by_key(|a| a.provider());
    let domains = Domains::new(&access[0], &dependencies, core_types, &mut meter()).unwrap();
    let missing = Domains::new(&access[0], &[], core_types, &mut meter()).unwrap();
    artifacts[0].contracts.with_bound(&foundations[0], core_types, |members, constructors| {
        let parameters = members.bind_parameter_protocols(constructors, &artifacts[0].contracts.protocols, &mut meter()).unwrap();
        let mut targets = BTreeSet::new();
        for (i, artifact) in artifacts.iter().enumerate() {
            for (kind, donor) in selected(&artifact.defaults) {
                let original = selected(&artifacts[0].defaults).remove(&kind).unwrap();
                let donor_reference = &donor.references().callables()[0];
                let Target::Callable(callee) = donor_reference.target() else { panic!("source callable") };
                let changed = replacement::call(original, callee, donor_reference.witness().target_domain().clone());
                let table = super::super::default_origins::replace(&artifacts[0].defaults, changed);
                let declarations = parameters.bind_default_declarations(&table, &[], &mut meter()).unwrap();
                domains.bind_nominal_default_callable_domains(&declarations, &mut meter()).unwrap();
                let reference = &donor.references().callables()[0];
                targets.insert(format!("{:?}", reference.target()));
                if i != 0 {
                    let provider = artifact.coordinate.identity().unwrap();
                    let error = missing.bind_nominal_default_callable_domains(&declarations, &mut meter()).unwrap_err();
                    assert!(matches!(error, Error::Target { key, error, .. } if key == original.key() && matches!(*error, hir::DefaultSourceDomainError::MissingProvider(actual) if actual == provider)));
                }
            }
        }
        assert_eq!(targets.len(), 9);
    });
}

fn selected(
    table: &hir::CanonicalDefaultSourceTemplatesV1,
) -> BTreeMap<u8, &hir::DefaultSourceTemplateV1> {
    table
        .records()
        .iter()
        .filter_map(|t| {
            let [reference] = t.references().callables() else {
                return None;
            };
            let Target::Callable(callee) = reference.target() else {
                return None;
            };
            let kind = match callee.declaration() {
                Declaration::Function(_) => 0,
                Declaration::GenericFunction(_) => 1,
                Declaration::PropertyAccessor(_) => 2,
                Declaration::Generated(_) => return None,
            };
            Some((kind, t))
        })
        .collect()
}
