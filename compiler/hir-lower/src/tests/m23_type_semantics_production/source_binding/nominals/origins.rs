use super::*;
use scoop_identity::{
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, SourceSpan,
};

#[test]
fn nominal_binding_requires_variant_subject_and_valid_source_points() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let choice = named(&fixture, &table, "Choice");
        let hir::NominalSourceShapeV1::Enum(shape) = choice.source_shape() else {
            panic!("enum")
        };
        let subject = DefinitionOriginSubject::EnumVariant(shape.variants()[0].variant());
        for missing in [true, false] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            let mut origins = output
                .output()
                .export
                .export_definition_origins
                .records()
                .iter()
                .chain(
                    output
                        .output()
                        .local
                        .local_value_identities
                        .definition_origins()
                        .records(),
                )
                .filter(|record| record.subject() != subject)
                .cloned()
                .collect::<Vec<_>>();
            if !missing {
                let origin = fixture
                    .foundation
                    .definition_origin(subject)
                    .unwrap()
                    .origin();
                let context = fixture
                    .foundation
                    .source_context_key(origin.context())
                    .unwrap();
                let invalid = DefinitionOrigin::new(
                    origin.source().clone(),
                    SourceSpan::new(1_000_000, 1_000_001).unwrap(),
                    context,
                )
                .unwrap();
                origins.push(DefinitionOriginRecord::new(subject, invalid));
            }
            canonical.set_definition_origins(origins).unwrap();
            let modified = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&modified, &fixture.identities, &mut meter())
                .unwrap();
            let error = foundation
                .bind_nominal_sources(&table, &mut meter())
                .unwrap_err();
            assert!(
                matches!((missing, &error),
                (true, Error::Origin(actual)) if *actual == subject)
                    || matches!(
                        (missing, &error),
                        (
                            false,
                            Error::Foundation(hir::TypeFoundationBindingError::MissingSourcePoint(
                                1_000_000
                            ))
                        )
                    ),
                "{error:?}"
            );
        }
    });
}
