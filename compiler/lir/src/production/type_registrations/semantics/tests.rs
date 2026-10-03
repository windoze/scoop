use crate::TypeInstanceShapeV1;
use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, ExactTypeKey, PackagePath,
    PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    ArrayElementStorageV1, DispatchEntry, ItableRecord, LayoutIdentity, MaterializationRoot,
    NoGcRuntimeFunction, RuntimeFunction, RuntimeTypeMappingRecord, TypeDescriptorIdentity,
    TypeInstanceKindV1, VtableRecord,
};

#[test]
fn projects_complete_descriptor_semantics_in_exact_type_order() {
    let interface_exact = exact_type("Readable");
    let owner_exact = exact_type("Document");
    let mut descriptors = Arena::new();
    let interface_identity = identity(interface_exact);
    let interface = descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Readable".to_string(),
        instance_layout: instance_layout(interface_exact),
        instance_shape: TypeInstanceShapeV1::abstract_ref(),
        inline_scan: TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: VtableRecord::new(&interface_identity, Vec::new()).unwrap(),
        itables: Vec::new(),
        identity: interface_identity,
    });
    let owner_identity = identity(owner_exact);
    let runtime_slot = DispatchEntry {
        callable: CallableRef::Runtime(RuntimeFunction::NoGc(NoGcRuntimeFunction::Trap)),
    };
    descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Document".to_string(),
        instance_layout: instance_layout(owner_exact),
        instance_shape: TypeInstanceShapeV1::inline_array(
            LirTargetProfile::DARWIN_AARCH64,
            ArrayElementStorageV1::zero_sized(8).unwrap(),
        )
        .unwrap(),
        inline_scan: TypeDescriptorInlineScanV1::Null,
        parent: Some(TypeDescriptorRef::Local(interface)),
        vtable: VtableRecord::new(&owner_identity, vec![runtime_slot]).unwrap(),
        itables: vec![
            ItableRecord::new(
                &owner_identity,
                interface_exact,
                TypeDescriptorRef::Local(interface),
                vec![runtime_slot],
            )
            .unwrap(),
        ],
        identity: owner_identity,
    });

    let plans = StrongTypeDescriptorSemanticPlanSetV1::from_components(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        DescriptorSemanticInputs {
            descriptors: &descriptors,
            external_descriptors: &Arena::new(),
            layouts: &Arena::new(),
            arrays: &Arena::new(),
            functions: &[],
            external_callables: &Arena::new(),
        },
    )
    .unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.descriptors().len(), 2);
    assert!(
        plans
            .descriptors()
            .windows(2)
            .all(|pair| pair[0].exact_type() < pair[1].exact_type())
    );
    let owner = plans
        .descriptors()
        .iter()
        .find(|plan| plan.exact_type() == owner_exact)
        .unwrap();
    assert_eq!(owner.diagnostic_name(), "Document");
    assert_eq!(
        owner.instance_layout(),
        instance_layout(owner_exact).layout_record().id()
    );
    assert_eq!(
        owner.instance_scan(),
        instance_layout(owner_exact).scan_record().id()
    );
    assert_eq!(
        owner.instance_shape().instance_kind(),
        TypeInstanceKindV1::InlineArray
    );
    assert_eq!(
        owner.parent(),
        Some(StrongTypeDescriptorRefV1::Local(interface_exact))
    );
    assert_eq!(
        owner.vtable().slots(),
        [StrongTypeDispatchCallableRefV1::Runtime(
            RuntimeFunction::NoGc(NoGcRuntimeFunction::Trap,)
        )]
    );
    assert_eq!(owner.itables().len(), 1);
    assert_eq!(
        owner.itables()[0].interface(),
        StrongTypeDescriptorRefV1::Local(interface_exact)
    );
}

#[test]
fn rejects_duplicate_exact_types_and_foreign_instance_layouts() {
    let exact = exact_type("Duplicate");
    let mut descriptors = Arena::new();
    for name in ["first", "second"] {
        let identity = identity(exact);
        descriptors.alloc(TypeDescriptor {
            release_policy: Default::default(),
            relations: Default::default(),
            diagnostic_name: name.to_string(),
            instance_layout: instance_layout(exact),
            instance_shape: TypeInstanceShapeV1::abstract_ref(),
            inline_scan: TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable: VtableRecord::new(&identity, Vec::new()).unwrap(),
            itables: Vec::new(),
            identity,
        });
    }
    assert_eq!(
        StrongTypeDescriptorSemanticPlanSetV1::from_components(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            DescriptorSemanticInputs {
                descriptors: &descriptors,
                external_descriptors: &Arena::new(),
                layouts: &Arena::new(),
                arrays: &Arena::new(),
                functions: &[],
                external_callables: &Arena::new(),
            },
        ),
        Err(StrongTypeDescriptorSemanticPlanBuildError::DuplicateExactType(exact))
    );

    let owner = exact_type("Owner");
    let identity = identity(owner);
    let mut descriptors = Arena::new();
    descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Owner".to_string(),
        instance_layout: instance_layout(exact_type("Foreign")),
        instance_shape: TypeInstanceShapeV1::abstract_ref(),
        inline_scan: TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: VtableRecord::new(&identity, Vec::new()).unwrap(),
        itables: Vec::new(),
        identity,
    });
    assert_eq!(
        StrongTypeDescriptorSemanticPlanSetV1::from_components(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            DescriptorSemanticInputs {
                descriptors: &descriptors,
                external_descriptors: &Arena::new(),
                layouts: &Arena::new(),
                arrays: &Arena::new(),
                functions: &[],
                external_callables: &Arena::new(),
            },
        ),
        Err(StrongTypeDescriptorSemanticPlanBuildError::InstanceLayoutMismatch(owner))
    );
}

fn identity(exact_type: PersistentExactTypeId) -> TypeDescriptorIdentity {
    TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(exact_type).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

fn instance_layout(exact_type: PersistentExactTypeId) -> LayoutIdentity {
    LayoutIdentity::managed_object(
        exact_type,
        LirTargetProfile::DARWIN_AARCH64,
        MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        scoop_identity::SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}
