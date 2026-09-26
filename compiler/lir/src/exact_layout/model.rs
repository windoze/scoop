use std::sync::Arc;

use scoop_identity::{PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentScanId};

use super::*;
use crate::{
    ArrayElementStorageV1, ClassStorageLayoutV1, EnumStorageGeometryV1, FieldStorageV1,
    IntegerKind, NichePointerKind, NonZeroPow2, PlacedFieldStorageV1, StrongShapeDefinitionRefV1,
    TupleStorageLayoutV1, TypeInstanceShapeV1, ValueLayoutConstituentV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactLayoutExportV1(pub(super) LayoutBody);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LayoutBody {
    Value(Arc<ExactValueLayoutV1>),
    Instance(Arc<ExactInstanceLayoutV1>),
}

#[derive(Clone, Copy, Debug)]
pub enum ExactLayoutBodyKindV1<'a> {
    Value(&'a ExactValueLayoutV1),
    Instance(&'a ExactInstanceLayoutV1),
}

impl ExactLayoutExportV1 {
    pub fn value_handle(&self) -> Option<Arc<ExactValueLayoutV1>> {
        match &self.0 {
            LayoutBody::Value(value) => Some(Arc::clone(value)),
            LayoutBody::Instance(_) => None,
        }
    }
    pub fn instance_handle(&self) -> Option<Arc<ExactInstanceLayoutV1>> {
        match &self.0 {
            LayoutBody::Value(_) => None,
            LayoutBody::Instance(instance) => Some(Arc::clone(instance)),
        }
    }
    pub fn identity(&self) -> &ExactLayoutIdentityV1 {
        match &self.0 {
            LayoutBody::Value(value) => &value.identity,
            LayoutBody::Instance(instance) => &instance.identity,
        }
    }
    pub fn kind(&self) -> ExactLayoutBodyKindV1<'_> {
        match &self.0 {
            LayoutBody::Value(value) => ExactLayoutBodyKindV1::Value(value),
            LayoutBody::Instance(instance) => ExactLayoutBodyKindV1::Instance(instance),
        }
    }
    pub fn scan(&self) -> PersistentScanId {
        match &self.0 {
            LayoutBody::Value(value) => value.scan.id,
            LayoutBody::Instance(instance) => instance.scan.id,
        }
    }
    pub fn scan_definition(&self) -> StrongShapeDefinitionRefV1 {
        match &self.0 {
            LayoutBody::Value(value) => value.scan.physical,
            LayoutBody::Instance(instance) => instance.scan.physical,
        }
    }
}

impl From<ExactValueLayoutV1> for ExactLayoutExportV1 {
    fn from(value: ExactValueLayoutV1) -> Self {
        Self(LayoutBody::Value(Arc::new(value)))
    }
}
impl From<ExactInstanceLayoutV1> for ExactLayoutExportV1 {
    fn from(value: ExactInstanceLayoutV1) -> Self {
        Self(LayoutBody::Instance(Arc::new(value)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactValueLayoutV1 {
    pub(super) identity: ExactLayoutIdentityV1,
    pub(super) value: ValueLayoutConstituentV1,
    pub(super) representation: ExactRepresentationLayoutV1,
    pub(super) scan: ScanBinding,
}

impl ExactValueLayoutV1 {
    pub const fn identity(&self) -> &ExactLayoutIdentityV1 {
        &self.identity
    }
    pub const fn value(&self) -> &ValueLayoutConstituentV1 {
        &self.value
    }
    pub const fn representation(&self) -> &ExactRepresentationLayoutV1 {
        &self.representation
    }
    pub const fn scan(&self) -> PersistentScanId {
        self.scan.id
    }
    pub const fn scan_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.scan.physical
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactInstanceLayoutV1 {
    pub(super) identity: ExactLayoutIdentityV1,
    pub(super) shape: TypeInstanceShapeV1,
    pub(super) representation: InstanceRepresentationV1,
    pub(super) scan: ScanBinding,
}

impl ExactInstanceLayoutV1 {
    pub const fn identity(&self) -> &ExactLayoutIdentityV1 {
        &self.identity
    }
    pub const fn shape(&self) -> &TypeInstanceShapeV1 {
        &self.shape
    }
    pub const fn representation(&self) -> &InstanceRepresentationV1 {
        &self.representation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarRepresentationKindV1 {
    Integer(IntegerKind),
    Boolean,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicValueFamilyV1 {
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRepresentationLayoutV1(pub(super) ValueRepresentation);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ValueRepresentation {
    Scalar(ScalarRepresentationKindV1),
    QualifiedPointer(NichePointerKind),
    Struct(StructRepresentationLayoutV1),
    Tuple(TupleStorageLayoutV1),
    TaggedEnum(TaggedEnumRepresentationLayoutV1),
    NicheEnum(NicheEnumRepresentationLayoutV1),
    IntrinsicValue(IntrinsicValueFamilyV1),
}

#[derive(Clone, Copy, Debug)]
pub enum ExactRepresentationKindV1<'a> {
    Scalar(ScalarRepresentationKindV1),
    QualifiedPointer(NichePointerKind),
    Struct(&'a StructRepresentationLayoutV1),
    Tuple(&'a TupleStorageLayoutV1),
    TaggedEnum(&'a TaggedEnumRepresentationLayoutV1),
    NicheEnum(&'a NicheEnumRepresentationLayoutV1),
    IntrinsicValue(IntrinsicValueFamilyV1),
}

impl ExactRepresentationLayoutV1 {
    pub fn kind(&self) -> ExactRepresentationKindV1<'_> {
        match &self.0 {
            ValueRepresentation::Scalar(kind) => ExactRepresentationKindV1::Scalar(*kind),
            ValueRepresentation::QualifiedPointer(kind) => {
                ExactRepresentationKindV1::QualifiedPointer(*kind)
            }
            ValueRepresentation::Struct(value) => ExactRepresentationKindV1::Struct(value),
            ValueRepresentation::Tuple(value) => ExactRepresentationKindV1::Tuple(value),
            ValueRepresentation::TaggedEnum(value) => ExactRepresentationKindV1::TaggedEnum(value),
            ValueRepresentation::NicheEnum(value) => ExactRepresentationKindV1::NicheEnum(value),
            ValueRepresentation::IntrinsicValue(kind) => {
                ExactRepresentationKindV1::IntrinsicValue(*kind)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructRepresentationLayoutV1 {
    pub(super) policy: StructLayoutPolicyV1,
    pub(super) interior_mutable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructLayoutPolicyV1 {
    Ordinary(crate::AggregateStorageLayoutV1),
    CLayout(crate::CLayoutStorageReplayV1),
}
impl StructRepresentationLayoutV1 {
    pub const fn policy(&self) -> &StructLayoutPolicyV1 {
        &self.policy
    }
    pub const fn interior_mutable(&self) -> bool {
        self.interior_mutable
    }
    pub fn fields(&self) -> &[PlacedFieldStorageV1] {
        match &self.policy {
            StructLayoutPolicyV1::Ordinary(layout) => layout.fields(),
            StructLayoutPolicyV1::CLayout(layout) => layout.aggregate().fields(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantFieldLayoutV1 {
    pub(super) field: PersistentEnumVariantFieldId,
    pub(super) storage: FieldStorageV1,
    pub(super) access_alignment: NonZeroPow2,
}
impl EnumVariantFieldLayoutV1 {
    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }
    pub const fn storage(&self) -> &FieldStorageV1 {
        &self.storage
    }
    pub const fn access_alignment(&self) -> NonZeroPow2 {
        self.access_alignment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantLayoutV1 {
    pub(super) variant: PersistentEnumVariantId,
    pub(super) fields: Vec<EnumVariantFieldLayoutV1>,
}
impl EnumVariantLayoutV1 {
    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }
    pub fn fields(&self) -> &[EnumVariantFieldLayoutV1] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaggedEnumRepresentationLayoutV1 {
    pub(super) geometry: EnumStorageGeometryV1,
    pub(super) variants: Vec<EnumVariantLayoutV1>,
}
impl TaggedEnumRepresentationLayoutV1 {
    pub const fn geometry(&self) -> &EnumStorageGeometryV1 {
        &self.geometry
    }
    pub fn variants(&self) -> &[EnumVariantLayoutV1] {
        &self.variants
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NicheEnumRepresentationLayoutV1 {
    pub(super) pointer_kind: NichePointerKind,
    pub(super) variants: Vec<EnumVariantLayoutV1>,
    pub(super) payload_variant: PersistentEnumVariantId,
}
impl NicheEnumRepresentationLayoutV1 {
    pub const fn pointer_kind(&self) -> NichePointerKind {
        self.pointer_kind
    }
    pub fn variants(&self) -> &[EnumVariantLayoutV1] {
        &self.variants
    }
    pub const fn payload_variant(&self) -> PersistentEnumVariantId {
        self.payload_variant
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceRepresentationV1(pub(super) InstanceRepresentation);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InstanceRepresentation {
    ClassObject(ClassStorageLayoutV1),
    BoxedPayload(ValueLayoutConstituentV1),
    InlineBytes,
    InlineArray {
        element: ValueLayoutConstituentV1,
        storage: ArrayElementStorageV1,
    },
    AbstractReference,
}

#[derive(Clone, Copy, Debug)]
pub enum InstanceRepresentationKindV1<'a> {
    ClassObject(&'a ClassStorageLayoutV1),
    BoxedPayload(&'a ValueLayoutConstituentV1),
    InlineBytes,
    InlineArray {
        element: &'a ValueLayoutConstituentV1,
        storage: &'a ArrayElementStorageV1,
    },
    AbstractReference,
}
impl InstanceRepresentationV1 {
    pub fn kind(&self) -> InstanceRepresentationKindV1<'_> {
        match &self.0 {
            InstanceRepresentation::ClassObject(value) => {
                InstanceRepresentationKindV1::ClassObject(value)
            }
            InstanceRepresentation::BoxedPayload(value) => {
                InstanceRepresentationKindV1::BoxedPayload(value)
            }
            InstanceRepresentation::InlineBytes => InstanceRepresentationKindV1::InlineBytes,
            InstanceRepresentation::InlineArray { element, storage } => {
                InstanceRepresentationKindV1::InlineArray { element, storage }
            }
            InstanceRepresentation::AbstractReference => {
                InstanceRepresentationKindV1::AbstractReference
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScanBinding {
    pub(super) id: PersistentScanId,
    pub(super) physical: StrongShapeDefinitionRefV1,
}
