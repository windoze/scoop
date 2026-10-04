use super::*;
use scoop_identity::{NominalDeclarationOwner, SignatureTypeKey};

const SHAPES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-intrinsic-source-shapes/declarations.scoop"
));
const CONSUMERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-intrinsic-source-shapes/consumers.scoop"
));

#[test]
fn shared_nominal_producer_preserves_every_intrinsic_family() {
    let module =
        lower_core_with_additional_declarations(scoop_parser::parse(SHAPES).unwrap().declarations);
    let public = hir::CanonicalNominalInterfacesV1::from_export_hir(&module).unwrap();
    let mut expected = Vec::new();
    for (id, declaration) in module.structs.iter() {
        match &declaration.representation {
            hir::StructRepresentation::Intrinsic(intrinsic) => expected.push((
                source_owner(&module.nominal_identities[id]),
                declaration.name.as_str(),
                *intrinsic,
            )),
            hir::StructRepresentation::Declared(_) if declaration.name.starts_with("Intrinsic") => {
                let owner = source_owner(&module.nominal_identities[id]);
                let hir::NominalSourceShapeV1::Struct(shape) =
                    public.get(owner).unwrap().source_shape()
                else {
                    panic!("ordinary structs retain their actual fields and CLayout policy");
                };
                assert_eq!(shape.fields().len(), declaration.semantic_fields().len());
                assert_eq!(
                    shape.c_layout_policy(),
                    hir::NominalCLayoutPolicyV1::from_source_contract(
                        declaration.attributes.c_layout
                    )
                );
            }
            hir::StructRepresentation::Declared(_) => continue,
        }
    }
    for (id, declaration) in module.classes.iter() {
        match &declaration.representation {
            hir::ClassRepresentation::Intrinsic(intrinsic) => expected.push((
                source_owner(&module.nominal_identities[id]),
                declaration.name.as_str(),
                *intrinsic,
            )),
            hir::ClassRepresentation::Declared if declaration.name == "IntrinsicStringLike" => {
                let owner = source_owner(&module.nominal_identities[id]);
                let hir::NominalSourceShapeV1::Class(fields) =
                    public.get(owner).unwrap().source_shape()
                else {
                    panic!(
                        "ordinary classes preserve source storage instead of claiming an intrinsic family"
                    );
                };
                assert_eq!(fields.fields().len(), declaration.fields.len());
                assert_eq!(
                    fields.fields()[0].field(),
                    module.field_identities[declaration.fields[0]].id()
                );
            }
            hir::ClassRepresentation::Declared => continue,
        }
    }
    assert_eq!(expected.len(), 15);

    for (owner, name, family) in expected {
        let record = public.get(owner).unwrap();
        assert_eq!(
            record.source_shape(),
            &hir::NominalSourceShapeV1::Intrinsic(hir::NominalIntrinsicRepresentationV1::new(
                family
            ))
        );
        assert_eq!(family.source_name(), name);
        let kind = match family {
            hir::IntrinsicTypeKind::Array
            | hir::IntrinsicTypeKind::MutableArray
            | hir::IntrinsicTypeKind::String => hir::PublicNominalKindV1::Class,
            _ => hir::PublicNominalKindV1::Struct,
        };
        assert_eq!(record.kind(), kind);
        let bounds = record
            .type_parameters()
            .binders()
            .iter()
            .map(|binder| binder.bounds())
            .collect::<Vec<_>>();
        let expected = match family {
            hir::IntrinsicTypeKind::Array
            | hir::IntrinsicTypeKind::MutableArray
            | hir::IntrinsicTypeKind::FunPtr => vec![&hir::TypeParameterBoundsV1::Unconstrained],
            hir::IntrinsicTypeKind::Ptr => vec![&hir::TypeParameterBoundsV1::Value],
            _ => Vec::new(),
        };
        assert_eq!(bounds, expected);
    }
}

#[test]
fn constant_and_vararg_projection_keep_actual_nominal_references_in_combination() {
    let module = lower_core_with_additional_declarations(
        scoop_parser::parse(CONSUMERS).unwrap().declarations,
    );
    let public = hir::CanonicalNominalInterfacesV1::from_export_hir(&module).unwrap();
    let constants = hir::CanonicalExportConstValuesV1::from_export_hir(&module).unwrap();
    assert!(constants.records().len() >= 3);
    for constant in constants.records() {
        let SignatureTypeKey::Nominal(id) = constant.value_type() else {
            panic!("portable constants carry concrete nominal types");
        };
        let expected = match constant.value().kind() {
            hir::CanonicalConstValueKindV1::Char => hir::IntrinsicTypeKind::Char,
            hir::CanonicalConstValueKindV1::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
            hir::CanonicalConstValueKindV1::Boolean => hir::IntrinsicTypeKind::Boolean,
            hir::CanonicalConstValueKindV1::String => hir::IntrinsicTypeKind::String,
        };
        assert_eq!(
            public
                .get(NominalDeclarationOwner::Concrete(*id))
                .unwrap()
                .source_shape(),
            &hir::NominalSourceShapeV1::Intrinsic(hir::NominalIntrinsicRepresentationV1::new(
                expected
            ))
        );
    }
    let source = hir::CanonicalCallableSourceInterfacesV1::from_export_hir(&module).unwrap();
    let mut varargs = 0;
    for record in source.records() {
        for parameter in record.parameters().parameters() {
            let Some(element) = parameter.calling().element_type() else {
                continue;
            };
            let SignatureTypeKey::NominalApplication { origin, arguments } = parameter.value_type()
            else {
                panic!("varargs carry an explicit generic nominal application");
            };
            assert_eq!(arguments.as_slice(), std::slice::from_ref(element));
            assert_eq!(
                public
                    .get(NominalDeclarationOwner::GenericTemplate(*origin))
                    .unwrap()
                    .source_shape(),
                &hir::NominalSourceShapeV1::Intrinsic(hir::NominalIntrinsicRepresentationV1::new(
                    hir::IntrinsicTypeKind::Array
                ))
            );
            varargs += 1;
        }
    }
    assert!(varargs >= 2);
}

fn source_owner(identity: &hir::HirNominalIdentity) -> hir::SourceNominalId {
    hir::SourceNominalId::from_source_declaration(identity.source().unwrap().declaration()).unwrap()
}
