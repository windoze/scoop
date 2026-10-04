use super::*;
use crate::{IntegerKind, MirCLayoutContract};

mod release;
pub use release::MirClassReleasePolicyV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRepresentationFieldV1 {
    pub field: PersistentFieldId,
    pub value: PersistentExactTypeId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRepresentationVariantFieldV1 {
    pub field: PersistentEnumVariantFieldId,
    pub value: PersistentExactTypeId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRepresentationVariantV1 {
    pub variant: PersistentEnumVariantId,
    pub fields: Vec<MirRepresentationVariantFieldV1>,
    pub gc: MirGcKindV1,
}

/// CLayout is a source policy; target offsets, sizes and alignments are absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTypeCLayoutPolicyV1 {
    Ordinary,
    CLayout(MirCLayoutContract),
}

/// Fixed, parameter-free intrinsic declarations. Generic families require a
/// separately supported exact-application path and cannot masquerade as one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirParamFreeIntrinsicV1 {
    Unit,
    Integer(IntegerKind),
    Boolean,
    String,
    Char,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirClassKindV1 {
    Final,
    Open,
    Abstract,
}

/// Declaration order is semantic: fields and variants are never id-sorted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeRepresentationV1 {
    Intrinsic(MirParamFreeIntrinsicV1),
    Struct {
        fields: Vec<MirRepresentationFieldV1>,
        c_layout: MirTypeCLayoutPolicyV1,
        interior_mutable: bool,
    },
    Enum {
        variants: Vec<MirRepresentationVariantV1>,
    },
    Class {
        kind: MirClassKindV1,
        declared_fields: Vec<MirRepresentationFieldV1>,
        release_policy: MirClassReleasePolicyV1,
    },
    Interface,
    InlineArray {
        element: PersistentExactTypeId,
    },
    Object {
        backing: PersistentExactTypeId,
    },
    ObjectBacking {
        declared_fields: Vec<MirRepresentationFieldV1>,
    },
    BoxedValue {
        payload: MirRepresentationFieldV1,
    },
    CoroutineStep {
        variants: Vec<MirRepresentationVariantV1>,
    },
    CoroutineSlot {
        variants: Vec<MirRepresentationVariantV1>,
    },
}
impl MirTypeRepresentationV1 {
    pub const fn release_policy(&self) -> MirClassReleasePolicyV1 {
        match self {
            Self::Class { release_policy, .. } => *release_policy,
            _ => MirClassReleasePolicyV1::None,
        }
    }
    pub fn fields(&self) -> &[MirRepresentationFieldV1] {
        match self {
            Self::Struct { fields, .. } => fields,
            Self::Class {
                declared_fields, ..
            }
            | Self::ObjectBacking { declared_fields } => declared_fields,
            Self::BoxedValue { payload } => std::slice::from_ref(payload),
            Self::Intrinsic(_)
            | Self::InlineArray { .. }
            | Self::Enum { .. }
            | Self::Interface
            | Self::Object { .. }
            | Self::CoroutineStep { .. }
            | Self::CoroutineSlot { .. } => &[],
        }
    }
    pub fn variants(&self) -> &[MirRepresentationVariantV1] {
        match self {
            Self::Enum { variants }
            | Self::CoroutineStep { variants }
            | Self::CoroutineSlot { variants } => variants,
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBaseClassV1 {
    None,
    Base(PersistentExactTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBaseAndInterfacesV1 {
    pub base: MirBaseClassV1,
    /// Canonical direct-interface set; dispatch order lives in its own schema.
    pub interfaces: Vec<PersistentExactTypeId>,
}
