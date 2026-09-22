use super::*;
use crate::{
    DecodedNativeBoundaryNominalShape, IntegerKind, IntrinsicTypeKind, IntrinsicTypeParameters,
    IntrinsicTypeTarget, NominalIntrinsicRepresentationV1,
};

fn source(family: IntrinsicTypeKind, count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("ActualIntrinsic").unwrap(),
        match family.target() {
            IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
            IntrinsicTypeTarget::Class => SourceNominalKind::Class,
        },
        count,
    )
}

#[test]
fn all_intrinsic_families_roundtrip_with_their_actual_declaration_owner() {
    let families = IntegerKind::ALL
        .into_iter()
        .map(IntrinsicTypeKind::Integer)
        .chain([
            IntrinsicTypeKind::Boolean,
            IntrinsicTypeKind::String,
            IntrinsicTypeKind::Array,
            IntrinsicTypeKind::MutableArray,
            IntrinsicTypeKind::Ptr,
            IntrinsicTypeKind::FunPtr,
        ]);
    for family in families {
        let count = match family.parameters() {
            IntrinsicTypeParameters::None => 0,
            IntrinsicTypeParameters::OneInvariantUnconstrained
            | IntrinsicTypeParameters::OneInvariantValue => 1,
        };
        let source = source(family, count);
        let mut resolver = Resolver {
            types: vec![],
            generic_types: vec![],
            fields: vec![],
            variants: vec![],
            variant_fields: vec![],
        };
        if count == 0 {
            resolver.types.push((
                PersistentTypeId::from_source_declaration(&source).unwrap(),
                source.clone(),
            ));
        } else {
            resolver.generic_types.push((
                PersistentGenericTypeId::from_source_declaration(&source).unwrap(),
                source.clone(),
            ));
        }
        let record = NativeBoundaryTypeDefinitionRecord::new(
            &source,
            &[count],
            NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(family)),
        )
        .unwrap();
        let bytes = encode(&record).unwrap();
        let decoded: DecodedNativeBoundaryTypeDefinitionRecord =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut resolver).unwrap(), record);
    }
}

#[test]
fn intrinsic_shape_has_a_distinct_fixed_wire_tag() {
    let boolean = NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(
        IntrinsicTypeKind::Boolean,
    ));
    assert_eq!(encode(&boolean).unwrap(), [0xa2, 0, 4, 1, 0xa1, 0, 2]);
    let unknown = decode_canonical::<DecodedNativeBoundaryNominalShape>(
        &[0xa1, 0, 5],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        unknown.kind(),
        &scoop_wire::WireErrorKind::UnknownTag { tag: 5 }
    );
}

#[test]
fn intrinsic_shape_rejects_wrong_declaration_kind_and_arity() {
    let boolean = NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(
        IntrinsicTypeKind::Boolean,
    ));
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &source(IntrinsicTypeKind::String, 0),
            &[0],
            boolean.clone()
        ),
        Err(NativeBoundaryDefinitionError::ShapeKindMismatch)
    );
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &source(IntrinsicTypeKind::Boolean, 1),
            &[1],
            boolean
        ),
        Err(NativeBoundaryDefinitionError::IntrinsicArityMismatch {
            expected: 0,
            actual: 1
        })
    );
    let array = NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(
        IntrinsicTypeKind::Array,
    ));
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(&source(IntrinsicTypeKind::Array, 0), &[0], array),
        Err(NativeBoundaryDefinitionError::IntrinsicArityMismatch {
            expected: 1,
            actual: 0
        })
    );
}

#[test]
fn reader_rechecks_intrinsic_kind_against_the_resolved_declaration() {
    let source = source(IntrinsicTypeKind::String, 0);
    let record = NativeBoundaryTypeDefinitionRecord::new(
        &source,
        &[0],
        NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(
            IntrinsicTypeKind::String,
        )),
    )
    .unwrap();
    let mut bytes = encode(&record).unwrap();
    let shape_end = bytes.len() - 5;
    assert_eq!(bytes[shape_end], 3);
    bytes[shape_end] = 2;
    let decoded: DecodedNativeBoundaryTypeDefinitionRecord =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut resolver = Resolver {
        types: vec![(
            PersistentTypeId::from_source_declaration(&source).unwrap(),
            source,
        )],
        generic_types: vec![],
        fields: vec![],
        variants: vec![],
        variant_fields: vec![],
    };
    let error = decoded.resolve(&mut resolver).unwrap_err();
    assert_eq!(
        error.to_string(),
        NativeBoundaryDefinitionError::ShapeKindMismatch.to_string()
    );
}
