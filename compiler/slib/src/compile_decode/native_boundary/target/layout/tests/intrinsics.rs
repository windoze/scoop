use super::*;
use scoop_hir::{
    IntegerKind, IntrinsicTypeKind, IntrinsicTypeParameters, IntrinsicTypeTarget,
    NominalIntrinsicRepresentationV1,
};
use scoop_identity::PersistentGenericTypeId;

fn with_intrinsic(
    family: IntrinsicTypeKind,
    provider: ConeIdentity,
    check: impl FnOnce(&mut NativeBoundaryNormalizer<'_>, PersistentExactTypeId),
) {
    let count = match family.parameters() {
        IntrinsicTypeParameters::None => 0,
        IntrinsicTypeParameters::OneInvariantUnconstrained
        | IntrinsicTypeParameters::OneInvariantValue => 1,
    };
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Representation").unwrap(),
        match family.target() {
            IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
            IntrinsicTypeTarget::Class => SourceNominalKind::Class,
        },
        count,
    );
    let unit = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let key = if count == 0 {
        ExactTypeKey::Nominal(PersistentTypeId::from_source_declaration(&source).unwrap())
    } else {
        ExactTypeKey::NominalApplication {
            origin: PersistentGenericTypeId::from_source_declaration(&source).unwrap(),
            arguments: NonEmptyVec::from_first(unit.id(), []),
        }
    };
    let exact_record = CborIdentityRecord::from_key(key).unwrap();
    let exact = exact_record.id();
    let exact_types = HashMap::from([
        (exact, exact_record.into_shared_key()),
        (unit.id(), unit.into_shared_key()),
    ]);
    let definition = NativeBoundaryTypeDefinitionRecord::new(
        &source,
        &[count],
        NativeBoundaryNominalShape::Intrinsic(NominalIntrinsicRepresentationV1::new(family)),
    )
    .unwrap();
    let definitions = HashMap::from([(
        definition.owner(),
        AbiNominalDefinition::native(&definition),
    )]);
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
    let mut normalizer = NativeBoundaryNormalizer::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &mut meter,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );
    check(&mut normalizer, exact);
}

#[test]
fn scalar_abi_uses_intrinsic_family_from_either_provider() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        for kind in IntegerKind::ALL {
            with_intrinsic(
                IntrinsicTypeKind::Integer(kind),
                provider,
                |normalizer, exact| {
                    let storage = normalizer.c_storage(exact).unwrap();
                    let (signedness, bit_width) = integer_representation(kind);
                    assert_eq!(
                        storage,
                        CanonicalCStorageType::Integer {
                            exact_type: exact,
                            signedness,
                            bit_width
                        }
                    );
                    let physical = normalizer.scoop_layout(exact).unwrap();
                    assert_eq!(physical.size, u64::from(kind.width().bytes()));
                    assert_eq!(physical.shape, ScoopAbiValueShape::Scalar);
                    assert!(physical.gc_free);
                },
            );
        }
        with_intrinsic(IntrinsicTypeKind::Boolean, provider, |normalizer, exact| {
            assert_eq!(
                normalizer.c_storage(exact).unwrap(),
                CanonicalCStorageType::Boolean { exact_type: exact }
            );
            let physical = normalizer.scoop_layout(exact).unwrap();
            assert_eq!(physical.size, 1);
            assert_eq!(physical.shape, ScoopAbiValueShape::Scalar);
            assert!(physical.gc_free);
        });
    }
}

#[test]
fn intrinsic_reference_layout_and_niches_preserve_managed_storage() {
    for family in [
        IntrinsicTypeKind::String,
        IntrinsicTypeKind::Array,
        IntrinsicTypeKind::MutableArray,
    ] {
        with_intrinsic(family, ConeIdentity::SINGLE_FILE, |normalizer, exact| {
            assert!(matches!(
                normalizer.c_storage(exact),
                Err(NativeBoundaryCompileError::Target(
                    NativeBoundaryTargetError::NotCAbiSafe { .. }
                ))
            ));
            let physical = normalizer.scoop_layout(exact).unwrap();
            assert_eq!(physical.size, 8);
            assert!(!physical.gc_free);
            assert_eq!(
                normalizer.niche_pointer_kind(exact),
                Some(scoop_lir::PointerKind::Managed)
            );
        });
    }
}

#[test]
fn pointer_intrinsics_require_structural_exact_types() {
    for family in [IntrinsicTypeKind::Ptr, IntrinsicTypeKind::FunPtr] {
        with_intrinsic(family, ConeIdentity::SINGLE_FILE, |normalizer, exact| {
            assert!(matches!(
                normalizer.c_storage(exact),
                Err(NativeBoundaryCompileError::Target(
                    NativeBoundaryTargetError::InvalidSignatureShape
                ))
            ));
            assert!(matches!(
                normalizer.scoop_layout(exact),
                Err(NativeBoundaryCompileError::Target(
                    NativeBoundaryTargetError::InvalidSignatureShape
                ))
            ));
        });
    }
}

#[test]
fn a_fixed_core_scalar_identity_needs_its_actual_representation_witness() {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("UInt8").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let owner = PersistentTypeId::from_source_declaration(&source).unwrap();
    let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
    let exact = exact_record.id();
    let exact_types = HashMap::from([(exact, exact_record.into_shared_key())]);
    let definition = NativeBoundaryTypeDefinitionRecord::new(
        &source,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: vec![],
        },
    )
    .unwrap();
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    for definitions in [
        HashMap::new(),
        HashMap::from([(
            definition.owner(),
            AbiNominalDefinition::native(&definition),
        )]),
    ] {
        let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
        let mut normalizer = NativeBoundaryNormalizer::new(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            &mut meter,
            &exact_types,
            &callable_applications,
            &initialization_units,
            &definitions,
        );
        if definitions.is_empty() {
            assert!(matches!(
                normalizer.c_storage(exact),
                Err(NativeBoundaryCompileError::ClosureRequired { .. })
            ));
            assert!(matches!(
                normalizer.scoop_layout(exact),
                Err(NativeBoundaryCompileError::ClosureRequired { .. })
            ));
        } else {
            assert!(matches!(
                normalizer.c_storage(exact),
                Err(NativeBoundaryCompileError::Target(
                    NativeBoundaryTargetError::NotCAbiSafe { .. }
                ))
            ));
            assert_eq!(normalizer.scoop_layout(exact).unwrap().size, 0);
        }
    }
}
