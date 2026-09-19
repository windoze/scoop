use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    ImmortalObjectKey, ImmortalObjectOwner, PackagePath, PersistentExactTypeId,
    PersistentPropertyId, PersistentTypeId, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

use super::*;
use crate::{
    Global, ImmortalObjectIdentity, LayoutIdentity, MaterializationRoot, StaticStorageIdentity,
};

#[test]
fn semantic_plans_bind_storage_shape_scan_and_immortal_initializers() {
    let mut globals = Arena::new();
    let immortal = globals.alloc(string_global(0, "value"));
    let first = storage_global(
        "first",
        LirType::Ptr(PointerKind::Managed),
        RefScan::References(vec![0]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: immortal,
                kind: PointerKind::Managed,
            },
        },
    );
    let first_id = storage_id(&first);
    globals.alloc(first);
    let second = storage_global(
        "second",
        LirType::Aggregate(Vec::new()),
        RefScan::None,
        LirStaticInitialState::ZeroedForRuntimeUnit,
    );
    let second_id = storage_id(&second);
    globals.alloc(second);

    let plans = StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        &globals,
        &crate::StructDefs::default(),
        &crate::EnumDefs::default(),
    )
    .unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.storages().len(), 2);
    assert!(
        plans
            .storages()
            .windows(2)
            .all(|pair| pair[0].storage() < pair[1].storage())
    );
    let first = plans
        .storages()
        .iter()
        .find(|plan| plan.storage() == first_id)
        .unwrap();
    assert_eq!(first.scan_kind(), StaticStorageScanKindV1::Recursive);
    assert_eq!((first.byte_size(), first.allocation_extent()), (8, 8));
    assert_eq!(first.required_alignment(), 8);
    assert_eq!(first.initial_state().initial_template(), &[0; 8]);
    assert_eq!(first.initial_state().immortal_relocations().len(), 1);
    assert_eq!(
        first.initial_state().immortal_relocations()[0].target(),
        match &globals[immortal].init {
            GlobalInit::StringConst { identity, .. } => identity.identity_record().id(),
            _ => unreachable!(),
        }
    );
    let second = plans
        .storages()
        .iter()
        .find(|plan| plan.storage() == second_id)
        .unwrap();
    assert_eq!(second.scan_kind(), StaticStorageScanKindV1::None);
    assert_eq!((second.byte_size(), second.allocation_extent()), (0, 1));
}

#[test]
fn encoded_integer_templates_preserve_target_bits_and_zero_padding() {
    let mut structs = crate::StructDefs::default();
    let pair = structs.alloc_scoop(
        "Pair".to_string(),
        8,
        4,
        false,
        vec![
            crate::StructField {
                ty: LirType::I8,
                layout: crate::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            crate::StructField {
                ty: LirType::I32,
                layout: crate::FieldLayout {
                    offset: 4,
                    access_align: 4,
                },
            },
        ],
    );
    let mut globals = Arena::new();
    globals.alloc(storage_global(
        "bits",
        LirType::Struct(pair),
        RefScan::None,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Struct {
                struct_id: pair,
                fields: vec![
                    LirConstantImage::Integer(crate::LirIntegerConstant::Unsigned8(0xab)),
                    LirConstantImage::Integer(crate::LirIntegerConstant::Unsigned32(0x1234_5678)),
                ],
            },
        },
    ));

    let plans = StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        &globals,
        &structs,
        &crate::EnumDefs::default(),
    )
    .unwrap();

    assert_eq!(
        plans.storages()[0].initial_state().initial_template(),
        &[0xab, 0, 0, 0, 0x78, 0x56, 0x34, 0x12]
    );
}

#[test]
fn semantic_plans_reject_thread_local_and_noncanonical_empty_scans() {
    let mut globals = Arena::new();
    let mut thread_local = storage_global(
        "tls",
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::ZeroedForRuntimeUnit,
    );
    let GlobalInit::Storage {
        thread_local: flag, ..
    } = &mut thread_local.init
    else {
        unreachable!()
    };
    *flag = true;
    let storage = storage_id(&thread_local);
    globals.alloc(thread_local);
    assert_eq!(
        StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            &crate::StructDefs::default(),
            &crate::EnumDefs::default(),
        ),
        Err(StrongStaticStorageSemanticPlanBuildError::ThreadLocal(
            storage
        ))
    );

    let mut globals = Arena::new();
    let empty_scan = storage_global(
        "emptyScan",
        LirType::I64,
        RefScan::References(Vec::new()),
        LirStaticInitialState::ZeroedForRuntimeUnit,
    );
    let storage = storage_id(&empty_scan);
    globals.alloc(empty_scan);
    assert_eq!(
        StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            &crate::StructDefs::default(),
            &crate::EnumDefs::default(),
        ),
        Err(StrongStaticStorageSemanticPlanBuildError::Scan {
            storage,
            actual: RefScan::References(Vec::new()),
            expected: RefScan::None,
        })
    );
}

#[test]
fn semantic_plans_reject_a_scan_that_disagrees_with_storage_type() {
    let mut globals = Arena::new();
    let wrong_scan = storage_global(
        "wrongScan",
        LirType::Ptr(PointerKind::Managed),
        RefScan::References(vec![8]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::NullPointer(PointerKind::Managed),
        },
    );
    let storage = storage_id(&wrong_scan);
    globals.alloc(wrong_scan);

    assert_eq!(
        StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            &crate::StructDefs::default(),
            &crate::EnumDefs::default(),
        ),
        Err(StrongStaticStorageSemanticPlanBuildError::Scan {
            storage,
            actual: RefScan::References(vec![8]),
            expected: RefScan::References(vec![0]),
        })
    );
}

#[test]
fn encoded_values_reject_non_immortal_nonnull_targets() {
    let mut globals = Arena::new();
    let target = globals.alloc(storage_global(
        "target",
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::ZeroedForRuntimeUnit,
    ));
    let source = storage_global(
        "source",
        LirType::Ptr(PointerKind::Managed),
        RefScan::References(vec![0]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: target,
                kind: PointerKind::Managed,
            },
        },
    );
    let storage = storage_id(&source);
    globals.alloc(source);

    assert_eq!(
        StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            &crate::StructDefs::default(),
            &crate::EnumDefs::default(),
        ),
        Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
            storage,
            kind: StaticStorageInitialValueFailureV1::NonImmortalTarget,
        })
    );
}

fn storage_global(
    name: &str,
    ty: LirType,
    scan: RefScan,
    initial_state: LirStaticInitialState,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity: static_storage_identity(name),
            layout: LayoutIdentity::managed_value(
                exact_type(name),
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            ty,
            initial_state,
            thread_local: false,
        },
    }
}

fn storage_id(global: &Global) -> scoop_identity::PersistentStaticStorageId {
    match &global.init {
        GlobalInit::Storage { identity, .. } => identity.identity_record().id(),
        _ => unreachable!(),
    }
}

fn static_storage_identity(name: &str) -> StaticStorageIdentity {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap();
    StaticStorageIdentity::property_backing(
        PropertyOwner::Property(property),
        MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

fn string_global(path_index: u32, value: &str) -> Global {
    Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: ImmortalObjectIdentity::from_key(
                ImmortalObjectKey::string_constant(
                    ImmortalObjectOwner::Property(PropertyOwner::Property(
                        PersistentPropertyId::from_source_declaration(
                            &SourceDeclarationKey::property(
                                source_site(),
                                CanonicalIdentifier::new("text").unwrap(),
                            ),
                        )
                        .unwrap(),
                    )),
                    StructuralDefinitionPath::from_first(
                        StructuralPathSegment::new(
                            StructuralDefinitionSiteRole::StringConstant,
                            path_index,
                        ),
                        [],
                    ),
                ),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            value: value.to_string(),
        },
    }
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

mod projections;
