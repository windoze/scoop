use super::*;
use scoop_identity::CoreBuiltinNominal;

#[test]
fn declared_any_uses_managed_scoop_storage_and_rejects_c_storage() {
    let key = ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id());
    let exact = PersistentExactTypeId::from_key(&key).unwrap();
    with_keys(vec![key], &[any_definition()], |normalizer| {
        let layout = normalizer.scoop_layout(exact).unwrap();
        assert_eq!(
            (layout.size, layout.alignment, layout.gc_free),
            (8, 8, false)
        );
        assert!(matches!(normalizer.scoop_storage(exact).unwrap(), storage
            if storage.exact_type() == exact && storage.byte_size() == 8));
        assert!(matches!(normalizer.scoop_storage(exact).unwrap(), storage
            if storage.exact_type() == exact && storage.byte_size() == 8));
        assert_eq!(
            normalizer.niche_pointer_kind(exact),
            Some(scoop_lir::PointerKind::Managed)
        );
        assert!(matches!(normalizer.c_storage(exact),
            Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::NotCAbiSafe { exact: rejected }))
                if rejected == exact));
    });
}

#[test]
fn declared_any_in_a_tuple_preserves_aggregate_passing_and_managed_storage() {
    let any = ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id());
    let unit = ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
    let tuple = ExactTypeKey::Tuple(
        scoop_identity::NonEmptyVec::new(vec![
            PersistentExactTypeId::from_key(&any).unwrap(),
            PersistentExactTypeId::from_key(&unit).unwrap(),
        ])
        .unwrap(),
    );
    let exact = PersistentExactTypeId::from_key(&tuple).unwrap();
    with_keys(vec![any, unit, tuple], &[any_definition()], |normalizer| {
        let layout = normalizer.scoop_layout(exact).unwrap();
        assert_eq!(
            (layout.size, layout.alignment, layout.gc_free),
            (8, 8, false)
        );
        assert!(matches!(normalizer.scoop_storage(exact).unwrap(), storage
            if storage.exact_type() == exact && storage.byte_size() == 8));
        assert!(matches!(normalizer.scoop_storage(exact).unwrap(), storage
            if storage.exact_type() == exact && storage.byte_size() == 8));
    });
}

#[test]
fn any_requires_its_exact_key_and_source_declaration() {
    let any = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Any.identity_record().id(),
    ))
    .unwrap();
    with_keys(vec![], &[], |normalizer| {
        assert!(matches!(normalizer.scoop_storage(any),
            Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::MissingExactType { exact }))
                if exact == any));
    });
    let declared_owner = CoreBuiltinNominal::Any.identity_record().id();
    with_keys(
        vec![ExactTypeKey::Nominal(declared_owner)],
        &[],
        |normalizer| {
            assert_eq!(normalizer.niche_pointer_kind(any), None);
            assert!(matches!(normalizer.scoop_storage(any),
            Err(NativeBoundaryCompileError::ClosureRequired { owner: NativeBoundaryNominalOwner::Concrete(rejected) })
                if rejected == declared_owner));
        },
    );
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Any").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let owner = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let key = ExactTypeKey::Nominal(owner);
    let exact = PersistentExactTypeId::from_key(&key).unwrap();
    assert_ne!(any, exact);
    with_keys(vec![key], &[], |normalizer| {
        assert_eq!(normalizer.niche_pointer_kind(exact), None);
        assert!(matches!(normalizer.scoop_storage(exact),
            Err(NativeBoundaryCompileError::ClosureRequired { owner: NativeBoundaryNominalOwner::Concrete(rejected) })
                if rejected == owner));
    });
}

fn with_keys<T>(
    keys: Vec<ExactTypeKey>,
    records: &[NativeBoundaryTypeDefinitionRecord],
    run: impl FnOnce(&mut NativeBoundaryNormalizer<'_>) -> T,
) -> T {
    let exact_types = keys
        .into_iter()
        .map(|key| {
            let record = CborIdentityRecord::from_key(key).unwrap();
            (record.id(), record.into_shared_key())
        })
        .collect();
    let callable_applications = HashMap::new();
    let initialization_units = HashMap::new();
    let definitions = records
        .iter()
        .map(|record| (record.owner(), AbiNominalDefinition::native(record, false)))
        .collect();

    let mut normalizer = NativeBoundaryNormalizer::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );
    run(&mut normalizer)
}

pub(super) fn any_definition() -> NativeBoundaryTypeDefinitionRecord {
    NativeBoundaryTypeDefinitionRecord::new(
        &CoreBuiltinNominal::Any.declaration_key(),
        &[0],
        NativeBoundaryNominalShape::Intrinsic(scoop_hir::NominalIntrinsicRepresentationV1::new(
            scoop_hir::IntrinsicTypeKind::Any,
        )),
    )
    .unwrap()
}
