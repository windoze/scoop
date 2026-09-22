use std::collections::BTreeSet;

use scoop_identity::{
    EnumVariantIdentityKey, GeneratedNominalKey, OptionalSignatureType, PersistentEnumVariantId,
    PersistentTypeId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::{Encoder, WireEncode};

use super::{
    ClassRepresentationFieldOwnerV1, ClassRepresentationFieldV1, DeclarationAccessSourceV1,
    EnumRepresentationFieldV1, ExactTypeGcV1, NominalCLayoutPolicyV1,
    NominalIntrinsicRepresentationV1, StructRepresentationFieldV1, wire,
};
use crate::{
    IntrinsicTypeParameters, IntrinsicTypeTarget, NominalSourceShapeV1, SignatureBinderScopeV1,
    SourceNominalId,
};

mod decode;
mod error;
mod semantics;
mod table;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use error::*;
pub use semantics::*;
pub use table::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumRepresentationVariantV1 {
    variant: PersistentEnumVariantId,
    owner: SourceNominalId,
    fields: Vec<EnumRepresentationFieldV1>,
    gc: ExactTypeGcV1,
}

impl EnumRepresentationVariantV1 {
    pub fn try_new(
        key: &EnumVariantIdentityKey,
        fields: Vec<EnumRepresentationFieldV1>,
        gc: ExactTypeGcV1,
    ) -> Result<Self, NominalRepresentationBuildError> {
        let owner = key
            .source_owner()
            .ok_or(NominalRepresentationBuildError::GeneratedSourceVariant)?;
        let variant = PersistentEnumVariantId::from_key(key)
            .map_err(NominalRepresentationBuildError::VariantIdentity)?;
        let mut seen = BTreeSet::new();
        for (index, field) in fields.iter().enumerate() {
            if field.variant() != variant {
                return Err(NominalRepresentationBuildError::FieldOwner { index });
            }
            if !seen.insert(field.field()) {
                return Err(NominalRepresentationBuildError::DuplicateField { index });
            }
        }
        Ok(Self {
            variant,
            owner,
            fields,
            gc,
        })
    }
    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
    pub fn fields(&self) -> &[EnumRepresentationFieldV1] {
        &self.fields
    }
    pub const fn gc(&self) -> ExactTypeGcV1 {
        self.gc
    }
}

impl WireEncode for EnumRepresentationVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.fields)?;
        encoder.field(3)?;
        self.gc.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalRepresentationShapeV1 {
    Struct {
        fields: Vec<StructRepresentationFieldV1>,
        c_layout_policy: NominalCLayoutPolicyV1,
    },
    Enum {
        variants: Vec<EnumRepresentationVariantV1>,
    },
    Class {
        base: OptionalSignatureType,
        declared_fields: Vec<ClassRepresentationFieldV1>,
    },
    Interface,
    Object {
        backing_class: PersistentTypeId,
        declared_fields: Vec<ClassRepresentationFieldV1>,
    },
    Intrinsic {
        representation: NominalIntrinsicRepresentationV1,
    },
}

impl NominalRepresentationShapeV1 {
    pub fn source_kind(&self) -> SourceDeclarationKind {
        match self {
            Self::Struct { .. } => SourceDeclarationKind::Struct,
            Self::Enum { .. } => SourceDeclarationKind::Enum,
            Self::Class { .. } => SourceDeclarationKind::Class,
            Self::Interface => SourceDeclarationKind::Interface,
            Self::Object { .. } => SourceDeclarationKind::Object,
            Self::Intrinsic { representation } => match representation.family().target() {
                IntrinsicTypeTarget::Struct => SourceDeclarationKind::Struct,
                IntrinsicTypeTarget::Class => SourceDeclarationKind::Class,
            },
        }
    }

    fn validate_owner(
        &self,
        owner: PersistentTypeId,
    ) -> Result<(), NominalRepresentationBuildError> {
        let source_owner = SourceNominalId::Concrete(owner);
        let scope = SignatureBinderScopeV1::for_declaration(0, None);
        let validate_type = |ty: &SignatureTypeKey| {
            scope
                .validate(ty)
                .map_err(NominalRepresentationBuildError::Binder)
        };
        match self {
            Self::Struct {
                fields,
                c_layout_policy,
            } => {
                if fields.is_empty()
                    && matches!(c_layout_policy, NominalCLayoutPolicyV1::CLayout { .. })
                {
                    return Err(NominalRepresentationBuildError::EmptyCLayout);
                }
                let mut seen = BTreeSet::new();
                for (index, field) in fields.iter().enumerate() {
                    if field.owner() != source_owner {
                        return Err(NominalRepresentationBuildError::FieldOwner { index });
                    }
                    if !seen.insert(field.field()) {
                        return Err(NominalRepresentationBuildError::DuplicateField { index });
                    }
                    validate_type(field.value_type())?;
                }
            }
            Self::Enum { variants } => {
                let mut seen = BTreeSet::new();
                for (index, variant) in variants.iter().enumerate() {
                    if variant.owner() != source_owner {
                        return Err(NominalRepresentationBuildError::VariantOwner { index });
                    }
                    if !seen.insert(variant.variant()) {
                        return Err(NominalRepresentationBuildError::DuplicateVariant { index });
                    }
                    for field in variant.fields() {
                        validate_type(field.value_type())?;
                    }
                }
            }
            Self::Class {
                base,
                declared_fields,
            } => {
                if let OptionalSignatureType::Present(base) = base {
                    if !matches!(
                        base.as_ref(),
                        SignatureTypeKey::Nominal(_) | SignatureTypeKey::NominalApplication { .. }
                    ) {
                        return Err(NominalRepresentationBuildError::NonNominalBase);
                    }
                    validate_type(base)?;
                }
                validate_class_fields(
                    declared_fields,
                    ClassRepresentationFieldOwnerV1::SourceClass(source_owner),
                    &scope,
                )?;
            }
            Self::Object {
                backing_class,
                declared_fields,
            } => {
                let expected = PersistentTypeId::from_generated_key(
                    &GeneratedNominalKey::ObjectBackingClass { object: owner },
                )
                .map_err(NominalRepresentationBuildError::BackingIdentity)?;
                if *backing_class != expected {
                    return Err(NominalRepresentationBuildError::ObjectBackingClass);
                }
                validate_class_fields(
                    declared_fields,
                    ClassRepresentationFieldOwnerV1::ObjectBackingClass(expected),
                    &scope,
                )?;
            }
            Self::Intrinsic { representation } => {
                if representation.family().parameters() != IntrinsicTypeParameters::None {
                    return Err(NominalRepresentationBuildError::GenericIntrinsicFamily);
                }
            }
            Self::Interface => return Ok(()),
        }
        Ok(())
    }
}

fn validate_class_fields(
    fields: &[ClassRepresentationFieldV1],
    owner: ClassRepresentationFieldOwnerV1,
    scope: &SignatureBinderScopeV1,
) -> Result<(), NominalRepresentationBuildError> {
    let mut seen = BTreeSet::new();
    for (index, field) in fields.iter().enumerate() {
        if field.owner() != owner {
            return Err(NominalRepresentationBuildError::FieldOwner { index });
        }
        if !seen.insert(field.field()) {
            return Err(NominalRepresentationBuildError::DuplicateField { index });
        }
        scope
            .validate(field.value_type())
            .map_err(NominalRepresentationBuildError::Binder)?;
    }
    Ok(())
}

impl WireEncode for NominalRepresentationShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Struct {
                fields,
                c_layout_policy,
            } => {
                wire::tag(encoder, 3, 1)?;
                encoder.field(1)?;
                wire::sequence(encoder, fields)?;
                encoder.field(2)?;
                c_layout_policy.encode(encoder)
            }
            Self::Enum { variants } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                wire::sequence(encoder, variants)
            }
            Self::Class {
                base,
                declared_fields,
            } => {
                wire::tag(encoder, 3, 3)?;
                encoder.field(1)?;
                base.encode(encoder)?;
                encoder.field(2)?;
                wire::sequence(encoder, declared_fields)
            }
            Self::Interface => wire::tag(encoder, 1, 4),
            Self::Object {
                backing_class,
                declared_fields,
            } => {
                wire::tag(encoder, 3, 5)?;
                encoder.field(1)?;
                backing_class.encode(encoder)?;
                encoder.field(2)?;
                wire::sequence(encoder, declared_fields)
            }
            Self::Intrinsic { representation } => {
                wire::tag(encoder, 2, 6)?;
                encoder.field(1)?;
                representation.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalRepresentationSupportV1 {
    owner: PersistentTypeId,
    declaration_access: DeclarationAccessSourceV1,
    shape: NominalRepresentationShapeV1,
}

impl NominalRepresentationSupportV1 {
    pub fn try_new(
        key: &SourceDeclarationKey,
        declaration_access: DeclarationAccessSourceV1,
        shape: NominalRepresentationShapeV1,
    ) -> Result<Self, NominalRepresentationBuildError> {
        if key.duplicate_signature().type_parameter_count() != 0 {
            return Err(NominalRepresentationBuildError::GenericTemplate);
        }
        let owner = PersistentTypeId::from_source_declaration(key)
            .map_err(NominalRepresentationBuildError::OwnerIdentity)?;
        if key.declaration_kind() != shape.source_kind() {
            return Err(NominalRepresentationBuildError::SourceKind);
        }
        shape.validate_owner(owner)?;
        Ok(Self {
            owner,
            declaration_access,
            shape,
        })
    }
    pub const fn owner(&self) -> PersistentTypeId {
        self.owner
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn shape(&self) -> &NominalRepresentationShapeV1 {
        &self.shape
    }

    /// Layout support must reproduce the public source field/variant sequences
    /// and the declared CLayout policy exactly.
    pub fn validate_public_source_shape(
        &self,
        source: &NominalSourceShapeV1,
    ) -> Result<(), NominalRepresentationBuildError> {
        let agrees = match (&self.shape, source) {
            (
                NominalRepresentationShapeV1::Struct {
                    fields,
                    c_layout_policy,
                },
                NominalSourceShapeV1::Struct(source),
            ) => {
                *c_layout_policy == source.c_layout_policy()
                    && fields.len() == source.fields().len()
                    && fields.iter().zip(source.fields()).all(|(field, source)| {
                        field.field() == source.field() && field.value_type() == source.value_type()
                    })
            }
            (
                NominalRepresentationShapeV1::Enum { variants },
                NominalSourceShapeV1::Enum(source),
            ) => {
                variants.len() == source.variants().len()
                    && variants
                        .iter()
                        .zip(source.variants())
                        .all(|(variant, source)| {
                            variant.variant() == source.variant()
                                && variant.fields().len() == source.fields().len()
                                && variant.fields().iter().zip(source.fields()).all(
                                    |(field, source)| {
                                        field.field() == source.field()
                                            && field.value_type() == source.value_type()
                                    },
                                )
                        })
            }
            (NominalRepresentationShapeV1::Class { .. }, NominalSourceShapeV1::Class)
            | (NominalRepresentationShapeV1::Interface, NominalSourceShapeV1::Interface)
            | (NominalRepresentationShapeV1::Object { .. }, NominalSourceShapeV1::Object(_)) => {
                true
            }
            (
                NominalRepresentationShapeV1::Intrinsic { representation },
                NominalSourceShapeV1::Intrinsic(source),
            ) => representation == source,
            _ => false,
        };
        if agrees {
            Ok(())
        } else {
            Err(NominalRepresentationBuildError::PublicSourceShape)
        }
    }
}

impl WireEncode for NominalRepresentationSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.shape.encode(encoder)
    }
}
