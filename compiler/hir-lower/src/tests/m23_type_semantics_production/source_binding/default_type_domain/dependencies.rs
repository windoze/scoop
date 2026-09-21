use super::*;
mod support;

#[test]
fn default_type_domains_route_each_nominal_to_its_actual_artifact() {
    let core = trusted_core();
    let (artifacts, identities) = support::artifacts(&core);
    let foundations = artifacts.each_ref().map(|a| {
        a.source
            .bind_to_foundation(&a.foundation, &identities, &mut meter())
            .unwrap()
    });
    let declarations = std::array::from_fn::<_, 3, _>(|i| {
        foundations[i]
            .bind_default_access_declarations(
                &artifacts[i].table,
                &artifacts[i].required,
                &mut meter(),
            )
            .unwrap()
    });
    let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let core_types = inputs.protocols().fundamental_types();
    let mut dependencies = [&declarations[1], &declarations[2]];
    dependencies.sort_by_key(|d| d.provider());
    let domains = Domains::new(&declarations[0], &dependencies, core_types, &mut meter()).unwrap();
    for (artifact, declaration) in artifacts.iter().zip(&declarations) {
        let actual = domains
            .type_source_domain(&artifact.ty, &scope("nominal"), &mut meter())
            .unwrap();
        let subject = *artifact.required.first().unwrap();
        assert_eq!(
            actual,
            declaration
                .source_lookup_domain(subject, &mut meter())
                .unwrap()
        );
        assert!(actual.persistent().constraints().contains(
            &hir::PersistentAccessConstraintV1::Cone(declaration.provider())
        ));
    }
    let ty = Type::Tuple(
        scoop_identity::NonEmptyVec::new(artifacts.iter().map(|a| a.ty.clone()).collect()).unwrap(),
    );
    let actual = domains
        .type_source_domain(&ty, &scope("tuple"), &mut meter())
        .unwrap();
    assert_eq!(actual.persistent().constraints().len(), 6);
    assert!(actual.generic_subclasses().values().is_empty());
    let missing = Domains::new(&declarations[0], &[], core_types, &mut meter()).unwrap();
    assert!(
        matches!(missing.type_source_domain(&ty, &scope("tuple"), &mut meter()), Err(Error::MissingProvider(provider)) if provider == declarations[1].provider())
    );
    for wrong in [
        vec![&declarations[0]],
        vec![dependencies[0], dependencies[0]],
        vec![dependencies[1], dependencies[0]],
    ] {
        assert!(matches!(
            Domains::new(&declarations[0], &wrong, core_types, &mut meter()),
            Err(Error::DependencyOrder(_))
        ));
    }
    let (_, other_ids) = support::artifacts(&core);
    let other = artifacts[1]
        .source
        .bind_to_foundation(&artifacts[1].foundation, &other_ids, &mut meter())
        .unwrap();
    let other = other
        .bind_default_access_declarations(&artifacts[1].table, &artifacts[1].required, &mut meter())
        .unwrap();
    assert!(matches!(
        Domains::new(&declarations[0], &[&other], core_types, &mut meter()),
        Err(Error::IdentityGraph(_))
    ));
}
