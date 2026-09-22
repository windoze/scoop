use super::*;
use scoop_identity::{CanonicalIdentifier, SignatureTypeKey};

fn metadata(
    source: &Record,
    binders: hir::CanonicalBinderListV1,
    supertypes: hir::CanonicalSignatureTypesV1,
) -> Record {
    Record::try_new(
        source.owner(),
        source.modality(),
        binders,
        supertypes,
        source.constructors().clone(),
        source.members().clone(),
        source.children().clone(),
        source.source_shape().clone(),
    )
    .unwrap()
}

#[test]
fn nominal_binding_replays_source_kind_arity_bounds_and_supertype_rules() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let pair = named(&fixture, &table, "Pair");
        let marker = named(&fixture, &table, "Marker");
        let hir::SourceNominalId::Concrete(marker_id) = marker.owner() else {
            panic!("concrete interface")
        };
        let wrong_kind = Record::try_new(
            marker.owner(),
            hir::NominalInheritanceModalityV1::Final,
            marker.type_parameters().clone(),
            marker.supertypes().clone(),
            marker.constructors().clone(),
            marker.members().clone(),
            marker.children().clone(),
            hir::NominalSourceShapeV1::Class,
        )
        .unwrap();
        let extra_binder = hir::TypeParameterBinderV1::new(
            CanonicalIdentifier::new("U").unwrap(),
            hir::TypeParameterBoundsV1::Unconstrained,
        );
        let mut binders = pair.type_parameters().binders().to_vec();
        binders.push(extra_binder);
        let wrong_arity = metadata(
            pair,
            hir::CanonicalBinderListV1::try_new(binders).unwrap(),
            pair.supertypes().clone(),
        );
        let wrong_class_bound = hir::TypeParameterBoundsV1::Nominal(
            hir::NominalTypeParameterBoundsV1::try_new(
                Some(SignatureTypeKey::Nominal(marker_id)),
                hir::CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            )
            .unwrap(),
        );
        let wrong_bounds = metadata(
            pair,
            hir::CanonicalBinderListV1::try_new(vec![hir::TypeParameterBinderV1::new(
                CanonicalIdentifier::new("T").unwrap(),
                wrong_class_bound,
            )])
            .unwrap(),
            pair.supertypes().clone(),
        );
        let wrong_supertype = metadata(
            pair,
            pair.type_parameters().clone(),
            hir::CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::Binder {
                depth: 0,
                index: 0,
            }])
            .unwrap(),
        );
        let defaults = named(&fixture, &table, "Defaults");
        let class_types =
            ["Base", "Defaults"].map(|name| match named(&fixture, &table, name).owner() {
                hir::SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
                _ => panic!("concrete class"),
            });
        let multiple_bases = metadata(
            defaults,
            defaults.type_parameters().clone(),
            hir::CanonicalSignatureTypesV1::try_new(class_types.to_vec()).unwrap(),
        );
        for record in [
            wrong_kind,
            wrong_arity,
            wrong_bounds,
            wrong_supertype,
            multiple_bases,
        ] {
            let modified = replace(&table, record);
            let result = foundation.bind_nominal_sources(&modified, &mut meter());
            assert!(matches!(result, Err(Error::Contract { .. })), "{result:?}");
        }
    });
}

#[test]
fn nominal_binding_limits_signature_depth_before_recursive_shape_replay() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let pair = named(&fixture, &table, "Pair");
        let hir::NominalSourceShapeV1::Struct(shape) = pair.source_shape() else {
            panic!("struct")
        };
        let mut fields = shape.fields().to_vec();
        let mut nested = fields[0].value_type().clone();
        for _ in 0..32 {
            nested = SignatureTypeKey::RawPointer(Box::new(nested));
        }
        fields[0] = hir::StructSourceFieldV1::new(fields[0].field(), nested);
        let source = rebuild(
            pair,
            pair.constructors().clone(),
            pair.members().clone(),
            pair.children().clone(),
            hir::NominalSourceShapeV1::Struct(
                hir::StructSourceShapeV1::try_new(fields, hir::NominalCLayoutPolicyV1::Ordinary)
                    .unwrap(),
            ),
        );
        let limits = DecodeLimits {
            semantic_recursion: 8,
            ..DecodeLimits::default()
        };
        let modified = replace(&table, source);
        let result = foundation.bind_nominal_sources(&modified, &mut BudgetMeter::new(limits));
        assert!(matches!(result, Err(Error::Resource(_))), "{result:?}");
    });
}
