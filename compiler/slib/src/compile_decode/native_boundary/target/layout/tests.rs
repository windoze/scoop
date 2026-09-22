use scoop_hir::NativeBoundaryFieldDefinition;
use scoop_identity::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, FieldIdentityKey, PackagePath, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::*;

#[test]
fn rejects_unit_as_a_c_object_even_without_consulting_a_witness() {
    let owner = scoop_identity::CoreBuiltinNominal::Unit
        .identity_record()
        .id();
    let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
    let exact = record.id();
    let exact_types = HashMap::from([(exact, record.into_shared_key())]);
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    let definitions = HashMap::new();
    let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
    let mut normalizer = NativeBoundaryNormalizer::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &mut meter,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );

    assert!(matches!(
        normalizer.c_storage(exact),
        Err(NativeBoundaryCompileError::Target(
            NativeBoundaryTargetError::NotCAbiSafe { exact: actual }
        )) if actual == exact
    ));
}

#[test]
fn recomputes_packed_and_overaligned_c_struct_layout() {
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("PackedPair").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let owner = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let first =
        FieldIdentityKey::source_declared(&declaration, CanonicalIdentifier::new("first").unwrap())
            .unwrap();
    let second = FieldIdentityKey::source_declared(
        &declaration,
        CanonicalIdentifier::new("second").unwrap(),
    )
    .unwrap();
    let scalar = |name, kind| {
        let source = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        NativeBoundaryTypeDefinitionRecord::new(
            &source,
            &[0],
            NativeBoundaryNominalShape::Intrinsic(
                scoop_hir::NominalIntrinsicRepresentationV1::new(
                    scoop_hir::IntrinsicTypeKind::Integer(kind),
                ),
            ),
        )
        .unwrap()
    };
    let u8 = scalar("SmallField", scoop_hir::IntegerKind::UNSIGNED_8);
    let u64 = scalar("LargeField", scoop_hir::IntegerKind::UNSIGNED_64);
    let NativeBoundaryNominalOwner::Concrete(u8_owner) = u8.owner() else {
        panic!("concrete scalar");
    };
    let NativeBoundaryNominalOwner::Concrete(u64_owner) = u64.owner() else {
        panic!("concrete scalar");
    };
    let definition = NativeBoundaryTypeDefinitionRecord::new(
        &declaration,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::CLayout {
                aligned: CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes8),
                packed: CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes1),
            },
            fields: vec![
                NativeBoundaryFieldDefinition::new(&first, SignatureTypeKey::Nominal(u8_owner))
                    .unwrap(),
                NativeBoundaryFieldDefinition::new(&second, SignatureTypeKey::Nominal(u64_owner))
                    .unwrap(),
            ],
        },
    )
    .unwrap();
    let exact = |owner| {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner))
            .unwrap()
            .id()
    };
    let owner_exact = exact(owner);
    let exact_types = HashMap::from([
        (owner_exact, Arc::new(ExactTypeKey::Nominal(owner))),
        (exact(u8_owner), Arc::new(ExactTypeKey::Nominal(u8_owner))),
        (exact(u64_owner), Arc::new(ExactTypeKey::Nominal(u64_owner))),
    ]);
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    let definitions = HashMap::from([
        (definition.owner(), &definition),
        (u8.owner(), &u8),
        (u64.owner(), &u64),
    ]);
    let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
    let mut normalizer = NativeBoundaryNormalizer::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &mut meter,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );

    let CanonicalCStorageType::Struct { layout, .. } = normalizer.c_storage(owner_exact).unwrap()
    else {
        panic!("a C-layout struct must normalize to struct storage");
    };
    let layout = normalizer.expected_layouts.get(&layout).unwrap().layout();

    assert_eq!(layout.byte_size(), 16);
    assert_eq!(layout.alignment().get(), 8);
    assert_eq!(
        layout
            .fields()
            .iter()
            .copied()
            .map(CanonicalCAbiLayoutField::offset)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn scoop_layout_walk_has_inclusive_semantic_depth_boundaries() {
    let unit = scoop_identity::CoreBuiltinNominal::Unit
        .identity_record()
        .id();
    let leaf = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit)).unwrap();
    let middle =
        CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(leaf.id(), [])))
            .unwrap();
    let root = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
        middle.id(),
        [],
    )))
    .unwrap();
    let root_id = root.id();
    let exact_types = HashMap::from([
        (leaf.id(), leaf.into_shared_key()),
        (middle.id(), middle.into_shared_key()),
        (root_id, root.into_shared_key()),
    ]);
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    let definitions = HashMap::new();

    for (limit, accepted) in [(2, false), (3, true), (4, true)] {
        let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits {
            semantic_recursion: limit,
            ..scoop_wire::DecodeLimits::default()
        });
        let result = {
            let mut normalizer = NativeBoundaryNormalizer::new(
                scoop_lir::LirTargetProfile::DARWIN_AARCH64,
                &mut meter,
                &exact_types,
                &callable_applications,
                &initialization_units,
                &definitions,
            );
            normalizer.scoop_layout(root_id)
        };
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(NativeBoundaryCompileError::Resource(ref error))
                    if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                        resource: scoop_wire::ResourceKind::SemanticRecursion,
                        limit: 2,
                        observed: 3,
                    }
            ));
        }
    }
}

mod declared_structs;
mod intrinsics;
