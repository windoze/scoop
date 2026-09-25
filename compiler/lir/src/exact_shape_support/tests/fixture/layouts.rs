use scoop_identity::{
    CborIdentityRecord, ConeIdentity, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, ExactTypeKey, GeneratedEnumVariantRole, GeneratedNominalKey,
    PersistentExactTypeId, PersistentTypeId, RepresentationRole, ScanRole, SourceDeclarationKey,
    SourceDeclarationKind,
};

use crate::exact_layout::tests::Bound;
use crate::{
    EnumLayoutFieldInputV1, EnumLayoutVariantInputV1, ExactInstanceLayoutV1, ExactLayoutExportV1,
    ExactValueLayoutV1, NichePointerKind,
};

#[derive(Clone)]
pub(super) struct Shape {
    pub exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    pub value: ExactValueLayoutV1,
    pub instance: ExactInstanceLayoutV1,
}

pub(super) struct LayoutFixture {
    pub shapes: Vec<Shape>,
    pub records: Vec<ExactLayoutExportV1>,
    pub foundations: Vec<crate::OdrFreeLirFoundation>,
}

pub(super) fn build(source: &SourceDeclarationKey, wrong_step_payload: bool) -> LayoutFixture {
    let source_exact_record = source_exact(source);
    let (source_value, value_foundation) = source_value(source, source_exact_record.clone());
    let (source_instance, instance_foundation) = source_instance(source, &source_value);
    let mut shapes = vec![Shape {
        exact: source_exact_record.clone(),
        value: source_value.clone(),
        instance: source_instance,
    }];
    let mut foundations = vec![value_foundation, instance_foundation];
    let mut extra = Vec::new();

    if matches!(
        source.declaration_kind(),
        SourceDeclarationKind::Struct | SourceDeclarationKind::Enum
    ) {
        let key = GeneratedNominalKey::BoxedValue {
            payload: source_exact_record.id(),
        };
        let exact = generated_exact(&key);
        let (value, value_foundation) = managed_value(source.origin(), exact.clone());
        let (instance, instance_foundation) =
            boxed_instance(source.origin(), exact.clone(), &source_value);
        shapes.push(Shape {
            exact,
            value,
            instance,
        });
        foundations.extend([value_foundation, instance_foundation]);
    }

    let wrong_value = if wrong_step_payload {
        let exact = source_exact(&crate::exact_layout::tests::source(
            "WrongPayload",
            scoop_identity::SourceNominalKind::Struct,
            0,
        ));
        let (value, foundation) = empty_struct(source.origin(), exact);
        extra.push(value.clone().into());
        foundations.push(foundation);
        Some(value)
    } else {
        None
    };
    let step = GeneratedNominalKey::CoroutineStep {
        result: source_exact_record.id(),
    };
    let (shape, shape_foundations) = helper_shape(
        source.origin(),
        &step,
        [
            GeneratedEnumVariantRole::CoroutineStepCompleted,
            GeneratedEnumVariantRole::CoroutineStepSuspended,
        ],
        0,
        wrong_value.as_ref().unwrap_or(&source_value),
    );
    shapes.push(shape);
    foundations.extend(shape_foundations);
    let slot = GeneratedNominalKey::CoroutineSlot {
        value: source_exact_record.id(),
    };
    let (shape, shape_foundations) = helper_shape(
        source.origin(),
        &slot,
        [
            GeneratedEnumVariantRole::CoroutineSlotEmpty,
            GeneratedEnumVariantRole::CoroutineSlotValue,
        ],
        1,
        &source_value,
    );
    shapes.push(shape);
    foundations.extend(shape_foundations);

    let mut records = shapes
        .iter()
        .flat_map(|shape| {
            [
                ExactLayoutExportV1::from(shape.value.clone()),
                ExactLayoutExportV1::from(shape.instance.clone()),
            ]
        })
        .collect::<Vec<_>>();
    records.extend(extra);
    LayoutFixture {
        shapes,
        records,
        foundations,
    }
}

fn source_exact(
    source: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(source).unwrap(),
    ))
    .unwrap()
}

fn generated_exact(
    key: &GeneratedNominalKey,
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_generated_key(key).unwrap(),
    ))
    .unwrap()
}

fn source_value(
    source: &SourceDeclarationKey,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> (ExactValueLayoutV1, crate::OdrFreeLirFoundation) {
    match source.declaration_kind() {
        SourceDeclarationKind::Struct => empty_struct(source.origin(), exact),
        SourceDeclarationKind::Interface => managed_value(source.origin(), exact),
        _ => panic!("fixture source kind"),
    }
}

fn empty_struct(
    provider: ConeIdentity,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> (ExactValueLayoutV1, crate::OdrFreeLirFoundation) {
    let Bound {
        identity,
        foundation,
    } = Bound::for_provider(
        provider,
        exact,
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    let value = ExactValueLayoutV1::ordinary_struct(identity, false, &[], &foundation).unwrap();
    (value, foundation)
}

fn managed_value(
    provider: ConeIdentity,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> (ExactValueLayoutV1, crate::OdrFreeLirFoundation) {
    let Bound {
        identity,
        foundation,
    } = Bound::for_provider(
        provider,
        exact,
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    let value =
        ExactValueLayoutV1::qualified_pointer(identity, NichePointerKind::Managed, &foundation)
            .unwrap();
    (value, foundation)
}

fn source_instance(
    source: &SourceDeclarationKey,
    value: &ExactValueLayoutV1,
) -> (ExactInstanceLayoutV1, crate::OdrFreeLirFoundation) {
    let Bound {
        identity,
        foundation,
    } = Bound::for_provider(
        source.origin(),
        value.identity().exact_record().clone(),
        RepresentationRole::ManagedObject,
        ScanRole::ManagedObject,
    );
    let instance = match source.declaration_kind() {
        SourceDeclarationKind::Struct => {
            ExactInstanceLayoutV1::boxed_payload(identity, value, &foundation).unwrap()
        }
        SourceDeclarationKind::Interface => {
            ExactInstanceLayoutV1::abstract_reference(identity, &foundation).unwrap()
        }
        _ => panic!("fixture source kind"),
    };
    (instance, foundation)
}

fn boxed_instance(
    provider: ConeIdentity,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    payload: &ExactValueLayoutV1,
) -> (ExactInstanceLayoutV1, crate::OdrFreeLirFoundation) {
    let Bound {
        identity,
        foundation,
    } = Bound::for_provider(
        provider,
        exact,
        RepresentationRole::ManagedObject,
        ScanRole::ManagedObject,
    );
    let instance = ExactInstanceLayoutV1::boxed_payload(identity, payload, &foundation).unwrap();
    (instance, foundation)
}

fn helper_shape(
    provider: ConeIdentity,
    owner: &GeneratedNominalKey,
    roles: [GeneratedEnumVariantRole; 2],
    payload_index: usize,
    payload: &ExactValueLayoutV1,
) -> (Shape, [crate::OdrFreeLirFoundation; 2]) {
    let exact = generated_exact(owner);
    let variants = roles.map(|role| {
        CborIdentityRecord::from_key(EnumVariantIdentityKey::generated(owner, role).unwrap())
            .unwrap()
    });
    let field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        variants[payload_index].id(),
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let payload_field = [EnumLayoutFieldInputV1 {
        field: &field,
        value: payload,
    }];
    let empty: [EnumLayoutFieldInputV1<'_>; 0] = [];
    let fields = if payload_index == 0 {
        [&payload_field[..], &empty[..]]
    } else {
        [&empty[..], &payload_field[..]]
    };
    let inputs = [
        EnumLayoutVariantInputV1 {
            variant: &variants[0],
            fields: fields[0],
        },
        EnumLayoutVariantInputV1 {
            variant: &variants[1],
            fields: fields[1],
        },
    ];
    let Bound {
        identity,
        foundation: value_foundation,
    } = Bound::for_provider(
        provider,
        exact.clone(),
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    let value = ExactValueLayoutV1::enumeration(identity, &inputs, &value_foundation).unwrap();
    let (instance, instance_foundation) = boxed_instance(provider, exact.clone(), &value);
    (
        Shape {
            exact,
            value,
            instance,
        },
        [value_foundation, instance_foundation],
    )
}
