use super::*;
use scoop_identity::{ConeCoordinate, DefinitionOriginSubject, FieldIdentityView};

#[test]
fn object_property_fields_require_their_source_origins_in_foundation_bytes() {
    super::super::source_dispatch::with_hir_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/object-origins.scoop"
        )),
        |output, _| {
            let canonical =
                hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
            let export = output.output().export.module();
            let fields = export
                .class_fields
                .iter()
                .filter_map(|(field, _)| {
                    let identity = &export.field_identities[field];
                    let FieldIdentityView::Generated { key, .. } = identity.key().view() else {
                        return None;
                    };
                    key.object_backing_property()
                        .map(|property| (identity.id(), property))
                })
                .collect::<Vec<_>>();
            assert_eq!(fields.len(), 3);
            let coordinate = ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap();
            let validate = |foundation: &hir::CanonicalHirFoundation| {
                let decoded: hir::DecodedHirFoundation =
                    decode_canonical(&encode(foundation).unwrap()).unwrap();
                decoded.validate_with_dependency_sources(&coordinate, &mut identity_closure(output))
            };
            let restored =
                hir::OdrFreeHirFoundation::from_validated(validate(&canonical).unwrap()).unwrap();
            for (field, property) in &fields {
                let field = restored
                    .definition_origin(DefinitionOriginSubject::Field(*field))
                    .unwrap();
                let property = restored
                    .definition_origin(DefinitionOriginSubject::Property(*property))
                    .unwrap();
                assert_eq!(field.origin().source(), property.origin().source());
                assert_eq!(field.origin().span(), property.origin().span());
            }
            let missing = DefinitionOriginSubject::Field(fields[0].0);
            let mut incomplete = canonical.clone();
            incomplete
                .set_definition_origins(
                    export
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
                        .filter(|record| record.subject() != missing)
                        .cloned()
                        .collect(),
                )
                .unwrap();
            let Err(error) = validate(&incomplete) else {
                panic!("missing object field origin was accepted");
            };
            assert!(
                matches!(error, hir::HirFoundationValidationError::Origin(hir::DefinitionOriginValidationError::MissingSubject { subject }) if subject == missing),
                "{error:?}"
            );
        },
    );
}
