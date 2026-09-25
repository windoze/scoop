use super::*;

#[test]
fn shared_type_uses_reject_missing_extra_and_misdirected_inheritance() {
    let mut source = Artifact::new(coordinate("provider"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let other = source.nominal("Other", SourceNominalKind::Class, &[]);
    let provider = source.load(&[]);
    let mut source = Artifact::new(coordinate("consumer"));
    let derived = source.nominal("Derived", SourceNominalKind::Class, &[base]);
    let unused = source.nominal("Unused", SourceNominalKind::Class, &[]);
    let consumer = source.load(&[&provider]);
    let original = consumer.uses(&[&provider]).unwrap();
    let inheritance = original
        .records()
        .iter()
        .position(|record| matches!(record.usage(), SelectedTypeUseV1::Inheritance { .. }))
        .unwrap();
    let mut missing = original.records().to_vec();
    missing.remove(inheritance);
    reject(&consumer, &provider, missing);
    for (target_provider, child, edge) in [
        (
            provider.provider(),
            unused,
            SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
        ),
        (
            provider.provider(),
            derived,
            SelectedDirectInheritanceEdgeV1::Interface { exact: exact(base) },
        ),
        (
            provider.provider(),
            derived,
            SelectedDirectInheritanceEdgeV1::ClassBase {
                exact: exact(other),
            },
        ),
        (
            consumer.provider(),
            derived,
            SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
        ),
    ] {
        let mut changed = original.records().to_vec();
        changed[inheritance] = SelectedExternalTypeUseV1::new(
            target_provider,
            SelectedTypeUseV1::Inheritance {
                derived: exact(child),
                edge,
            },
        );
        reject(&consumer, &provider, changed);
    }
    let mut extra = original.records().to_vec();
    extra.push(SelectedExternalTypeUseV1::new(
        provider.provider(),
        SelectedTypeUseV1::Inheritance {
            derived: exact(unused),
            edge: SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
        },
    ));
    reject(&consumer, &provider, extra);
    assert!(
        matches!(consumer.uses(&[]), Err(Error::MissingProvider(actual)) if actual == provider.provider())
    );
    assert!(
        matches!(consumer.uses(&[&provider, &provider]), Err(Error::DuplicateProvider(actual)) if actual == provider.provider())
    );
}

#[test]
fn shared_type_uses_do_not_treat_a_value_or_builtin_as_an_inheritance_target() {
    let mut source = Artifact::new(coordinate("provider"));
    let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
    let provider = source.load(&[]);
    let mut source = Artifact::new(coordinate("consumer"));
    let derived = source.nominal("Derived", SourceNominalKind::Class, &[value]);
    let consumer = source.load(&[&provider]);
    assert!(
        matches!(consumer.uses(&[&provider]), Err(Error::InheritanceEdges(actual)) if actual == exact(derived))
    );
    let builtins = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let mut source = Artifact::new(coordinate("consumer"));
        source.nominal(
            "Derived",
            SourceNominalKind::Class,
            &[builtin.identity_record().id()],
        );
        let consumer = source.load(&[&builtins]);
        assert!(
            matches!(consumer.uses(&[&builtins]), Err(Error::MissingNominal(actual)) if actual == builtin.identity_record().id())
        );
    }
}

fn reject(consumer: &Loaded, provider: &Loaded, records: Vec<SelectedExternalTypeUseV1>) {
    assert!(matches!(
        consumer.validate(&selected(records), &[provider]),
        Err(Error::TypeUseInventory)
    ));
}
