use super::*;

#[test]
fn indirect_default_targets_reject_field_role_confusion() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        for (target, _, _) in expected::targets(output.output().export.module()) {
            let forged = match target {
                Target::ClassField(id) => Target::StructField(id),
                Target::StructField(id) => Target::ClassField(id),
                _ => continue,
            };
            assert!(
                matches!(foundation.default_indirect_access_subject(forged), Err(Error::Role(actual)) if actual == forged)
            );
        }
    });
}

#[test]
fn indirect_default_targets_require_actual_artifact_keys_not_only_known_identities() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            for (target, _, _) in expected::targets(output.output().export.module()) {
                let mut canonical = fixture.foundation.as_canonical().clone();
                match target {
                    Target::StructField(_) | Target::ClassField(_) => {
                        canonical.set_fields(vec![]).unwrap()
                    }
                    Target::EnumVariant(_) => canonical.set_enum_variants(vec![]).unwrap(),
                    Target::Singleton(_) => canonical.set_object_values(vec![]).unwrap(),
                }
                let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
                let foundation = fixture
                    .source
                    .bind_to_foundation(&incomplete, &fixture.identities)
                    .unwrap();
                assert!(
                    matches!(foundation.default_indirect_access_subject(target), Err(Error::MissingTarget(actual)) if actual == target)
                );
            }
        });
    }
}

#[test]
fn indirect_default_targets_require_the_actual_access_declaration_key() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            for (target, subject, _) in expected::targets(output.output().export.module()) {
                let mut canonical = fixture.foundation.as_canonical().clone();
                match subject {
                    Subject::Type(_) => canonical.set_types(vec![]).unwrap(),
                    Subject::GenericType(_) => canonical.set_generic_types(vec![]).unwrap(),
                    Subject::Property(_) => canonical.set_properties(vec![]).unwrap(),
                    _ => panic!("source access declaration"),
                }
                let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
                let foundation = fixture
                    .source
                    .bind_to_foundation(&incomplete, &fixture.identities)
                    .unwrap();
                assert!(
                    matches!(foundation.default_indirect_access_subject(target), Err(Error::MissingDeclaration(actual)) if actual == subject)
                );
            }
        });
    }
}
