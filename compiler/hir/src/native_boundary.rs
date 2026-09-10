//! Source-shape witnesses used only to close native boundary validation.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    CLayoutOverride, EnumVariantFieldKey, EnumVariantIdentityError, EnumVariantIdentityKey,
    FieldIdentityError, FieldIdentityKey, NominalDeclarationOwner, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentGenericTypeId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationIdentityError, SourceDeclarationKey,
};
use scoop_wire::{Encoder, HashError, WireEncode};

mod decode;
pub use decode::{
    DecodedNativeBoundaryCLayoutPolicy, DecodedNativeBoundaryFieldDefinition,
    DecodedNativeBoundaryNominalOwner, DecodedNativeBoundaryNominalShape,
    DecodedNativeBoundaryTypeDefinitionRecord, DecodedNativeBoundaryVariantDefinition,
    DecodedNativeBoundaryVariantFieldDefinition, NativeBoundaryResolutionError,
    NativeBoundaryResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryNominalOwner {
    Concrete(PersistentTypeId),
    GenericTemplate(PersistentGenericTypeId),
}

impl NativeBoundaryNominalOwner {
    fn from_declaration(
        declaration: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        match NominalDeclarationOwner::from_source_declaration(declaration)? {
            NominalDeclarationOwner::Concrete(id) => Ok(Self::Concrete(id)),
            NominalDeclarationOwner::GenericTemplate(id) => Ok(Self::GenericTemplate(id)),
        }
    }

    pub const fn declaration_owner(self) -> NominalDeclarationOwner {
        match self {
            Self::Concrete(id) => NominalDeclarationOwner::Concrete(id),
            Self::GenericTemplate(id) => NominalDeclarationOwner::GenericTemplate(id),
        }
    }
}

impl WireEncode for NativeBoundaryNominalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_value_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryFieldDefinition {
    field: PersistentFieldId,
    owner: NominalDeclarationOwner,
    ty: SignatureTypeKey,
}

impl NativeBoundaryFieldDefinition {
    pub fn new(
        key: &FieldIdentityKey,
        ty: SignatureTypeKey,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = key
            .source_owner()
            .ok_or(NativeBoundaryDefinitionError::ExpectedSourceField)?;
        let field = PersistentFieldId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::FieldIdentity)?;
        Ok(Self { field, owner, ty })
    }

    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }

    pub const fn owner(&self) -> NominalDeclarationOwner {
        self.owner
    }

    pub const fn ty(&self) -> &SignatureTypeKey {
        &self.ty
    }
}

impl WireEncode for NativeBoundaryFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryVariantFieldDefinition {
    field: PersistentEnumVariantFieldId,
    variant: PersistentEnumVariantId,
    ty: SignatureTypeKey,
}

impl NativeBoundaryVariantFieldDefinition {
    pub fn new(
        key: &EnumVariantFieldKey,
        ty: SignatureTypeKey,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let field = PersistentEnumVariantFieldId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::Hash)?;
        Ok(Self {
            field,
            variant: key.variant(),
            ty,
        })
    }

    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn ty(&self) -> &SignatureTypeKey {
        &self.ty
    }
}

impl WireEncode for NativeBoundaryVariantFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryVariantDefinition {
    variant: PersistentEnumVariantId,
    owner: NominalDeclarationOwner,
    fields: Vec<NativeBoundaryVariantFieldDefinition>,
}

impl NativeBoundaryVariantDefinition {
    pub fn new(
        key: &EnumVariantIdentityKey,
        fields: Vec<NativeBoundaryVariantFieldDefinition>,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = key
            .source_owner()
            .ok_or(NativeBoundaryDefinitionError::ExpectedSourceVariant)?;
        let variant = PersistentEnumVariantId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::VariantIdentity)?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if field.variant != variant {
                return Err(NativeBoundaryDefinitionError::VariantFieldOwnerMismatch);
            }
            if !seen.insert(field.field) {
                return Err(NativeBoundaryDefinitionError::DuplicateVariantField);
            }
        }
        Ok(Self {
            variant,
            owner,
            fields,
        })
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn owner(&self) -> NominalDeclarationOwner {
        self.owner
    }

    pub fn fields(&self) -> &[NativeBoundaryVariantFieldDefinition] {
        &self.fields
    }
}

impl WireEncode for NativeBoundaryVariantDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryCLayoutPolicy {
    NotCLayout,
    CLayout {
        aligned: CLayoutOverride,
        packed: CLayoutOverride,
    },
}

impl WireEncode for NativeBoundaryCLayoutPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCLayout => encode_empty_sum(encoder, 1),
            Self::CLayout { aligned, packed } => encode_two_value_sum(encoder, 2, aligned, packed),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryNominalShape {
    Reference,
    Struct {
        c_layout: NativeBoundaryCLayoutPolicy,
        fields: Vec<NativeBoundaryFieldDefinition>,
    },
    Enum {
        variants: Vec<NativeBoundaryVariantDefinition>,
    },
}

impl WireEncode for NativeBoundaryNominalShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Reference => encode_empty_sum(encoder, 1),
            Self::Struct { c_layout, fields } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                c_layout.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, fields)
            }
            Self::Enum { variants } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryTypeDefinitionRecord {
    owner: NativeBoundaryNominalOwner,
    type_parameter_count: u32,
    shape: NativeBoundaryNominalShape,
}

impl NativeBoundaryTypeDefinitionRecord {
    pub fn new(
        declaration: &SourceDeclarationKey,
        binder_parameter_counts: &[u32],
        shape: NativeBoundaryNominalShape,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = NativeBoundaryNominalOwner::from_declaration(declaration)
            .map_err(NativeBoundaryDefinitionError::DeclarationIdentity)?;
        let type_parameter_count = declaration.duplicate_signature().type_parameter_count();
        validate_binder_stack(type_parameter_count, binder_parameter_counts)?;
        validate_shape(declaration, owner, binder_parameter_counts, &shape)?;
        Ok(Self {
            owner,
            type_parameter_count,
            shape,
        })
    }

    pub const fn owner(&self) -> NativeBoundaryNominalOwner {
        self.owner
    }

    pub const fn type_parameter_count(&self) -> u32 {
        self.type_parameter_count
    }

    pub const fn shape(&self) -> &NativeBoundaryNominalShape {
        &self.shape
    }
}

impl WireEncode for NativeBoundaryTypeDefinitionRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.type_parameter_count))?;
        encoder.field(3)?;
        self.shape.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeBoundaryDefinitionError {
    ExpectedSourceField,
    ExpectedSourceVariant,
    ShapeKindMismatch,
    FieldOwnerMismatch,
    VariantOwnerMismatch,
    VariantFieldOwnerMismatch,
    DuplicateField,
    DuplicateVariant,
    DuplicateVariantField,
    EmptyBinderStack,
    BinderStackHeadMismatch { expected: u32, actual: u32 },
    BinderDepthOutOfRange { depth: u32 },
    BinderIndexOutOfRange { depth: u32, index: u32, count: u32 },
    DeclarationIdentity(SourceDeclarationIdentityError),
    FieldIdentity(FieldIdentityError),
    VariantIdentity(EnumVariantIdentityError),
    Hash(HashError),
}

impl fmt::Display for NativeBoundaryDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceField => {
                formatter.write_str("native boundary field must be a source field")
            }
            Self::ExpectedSourceVariant => {
                formatter.write_str("native boundary variant must be a source enum variant")
            }
            Self::ShapeKindMismatch => {
                formatter.write_str("native boundary shape does not match the source nominal kind")
            }
            Self::FieldOwnerMismatch => {
                formatter.write_str("native boundary field belongs to another nominal")
            }
            Self::VariantOwnerMismatch => {
                formatter.write_str("native boundary variant belongs to another enum")
            }
            Self::VariantFieldOwnerMismatch => {
                formatter.write_str("native boundary variant field belongs to another variant")
            }
            Self::DuplicateField => formatter.write_str("native boundary struct repeats a field"),
            Self::DuplicateVariant => formatter.write_str("native boundary enum repeats a variant"),
            Self::DuplicateVariantField => {
                formatter.write_str("native boundary enum variant repeats a field")
            }
            Self::EmptyBinderStack => {
                formatter.write_str("native boundary binder stack must contain its owner frame")
            }
            Self::BinderStackHeadMismatch { expected, actual } => write!(
                formatter,
                "native boundary owner binder count mismatch: expected {expected}, found {actual}"
            ),
            Self::BinderDepthOutOfRange { depth } => {
                write!(
                    formatter,
                    "native boundary binder depth {depth} is out of range"
                )
            }
            Self::BinderIndexOutOfRange {
                depth,
                index,
                count,
            } => write!(
                formatter,
                "native boundary binder index {index} is out of range for depth {depth} with {count} parameters"
            ),
            Self::DeclarationIdentity(error) => error.fmt(formatter),
            Self::FieldIdentity(error) => error.fmt(formatter),
            Self::VariantIdentity(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeBoundaryDefinitionError {}

fn validate_shape(
    declaration: &SourceDeclarationKey,
    owner: NativeBoundaryNominalOwner,
    binder_parameter_counts: &[u32],
    shape: &NativeBoundaryNominalShape,
) -> Result<(), NativeBoundaryDefinitionError> {
    let kind_matches = match shape {
        NativeBoundaryNominalShape::Reference => matches!(
            declaration.declaration_kind(),
            scoop_identity::SourceDeclarationKind::Class
                | scoop_identity::SourceDeclarationKind::Interface
                | scoop_identity::SourceDeclarationKind::Object
                | scoop_identity::SourceDeclarationKind::AnnotationClass
        ),
        NativeBoundaryNominalShape::Struct { .. } => {
            declaration.declaration_kind() == scoop_identity::SourceDeclarationKind::Struct
        }
        NativeBoundaryNominalShape::Enum { .. } => {
            declaration.declaration_kind() == scoop_identity::SourceDeclarationKind::Enum
        }
    };
    if !kind_matches {
        return Err(NativeBoundaryDefinitionError::ShapeKindMismatch);
    }

    let expected_owner = owner.declaration_owner();
    match shape {
        NativeBoundaryNominalShape::Reference => {}
        NativeBoundaryNominalShape::Struct { fields, .. } => {
            let mut seen = BTreeSet::new();
            for field in fields {
                if field.owner != expected_owner {
                    return Err(NativeBoundaryDefinitionError::FieldOwnerMismatch);
                }
                if !seen.insert(field.field) {
                    return Err(NativeBoundaryDefinitionError::DuplicateField);
                }
                validate_signature_binders(&field.ty, binder_parameter_counts)?;
            }
        }
        NativeBoundaryNominalShape::Enum { variants } => {
            let mut seen = BTreeSet::new();
            for variant in variants {
                if variant.owner != expected_owner {
                    return Err(NativeBoundaryDefinitionError::VariantOwnerMismatch);
                }
                if !seen.insert(variant.variant) {
                    return Err(NativeBoundaryDefinitionError::DuplicateVariant);
                }
                for field in &variant.fields {
                    validate_signature_binders(&field.ty, binder_parameter_counts)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_binder_stack(
    type_parameter_count: u32,
    binder_parameter_counts: &[u32],
) -> Result<(), NativeBoundaryDefinitionError> {
    let Some(&actual) = binder_parameter_counts.first() else {
        return Err(NativeBoundaryDefinitionError::EmptyBinderStack);
    };
    if actual != type_parameter_count {
        return Err(NativeBoundaryDefinitionError::BinderStackHeadMismatch {
            expected: type_parameter_count,
            actual,
        });
    }
    Ok(())
}

fn validate_signature_binders(
    ty: &SignatureTypeKey,
    binder_parameter_counts: &[u32],
) -> Result<(), NativeBoundaryDefinitionError> {
    match ty {
        SignatureTypeKey::Nominal(_) => Ok(()),
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => {
            for argument in arguments.as_slice() {
                validate_signature_binders(argument, binder_parameter_counts)?;
            }
            Ok(())
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                validate_signature_binders(parameter, binder_parameter_counts)?;
            }
            validate_signature_binders(result, binder_parameter_counts)
        }
        SignatureTypeKey::RawPointer(pointee) => {
            validate_signature_binders(pointee, binder_parameter_counts)
        }
        SignatureTypeKey::Binder { depth, index } => {
            let depth_index = usize::try_from(*depth).map_err(|_| {
                NativeBoundaryDefinitionError::BinderDepthOutOfRange { depth: *depth }
            })?;
            let Some(&count) = binder_parameter_counts.get(depth_index) else {
                return Err(NativeBoundaryDefinitionError::BinderDepthOutOfRange { depth: *depth });
            };
            if *index >= count {
                return Err(NativeBoundaryDefinitionError::BinderIndexOutOfRange {
                    depth: *depth,
                    index: *index,
                    count,
                });
            }
            Ok(())
        }
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
