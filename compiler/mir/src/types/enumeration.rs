use super::*;
use scoop_identity::{PersistentEnumVariantFieldId, PersistentEnumVariantId};

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is a source-facing display name.
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    /// Canonical concrete arguments of this monomorphized enum instance.
    /// Together with the arena id these recover its exact MIR `Type`.
    pub type_arguments: Vec<Type>,
    /// True exactly when every fully specialized variant is GC-free.
    pub gc_free: bool,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub identity: PersistentEnumVariantId,
    pub name: String,
    /// GC-free classification of this fully specialized variant.
    pub gc_free: bool,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<VariantField>,
}

/// A source or generated payload field with an inseparable persistent identity.
#[derive(Debug)]
pub struct VariantField {
    pub identity: PersistentEnumVariantFieldId,
    pub name: String,
    pub ty: Type,
}

/// A variant identity checked against one concrete MIR enum definition.
///
/// The fields are private deliberately: a raw enum id and variant index cannot
/// be paired at an expression site without first consulting the enum arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirVariantRef {
    enum_id: EnumId,
    variant: u32,
}

impl MirVariantRef {
    pub fn new(
        enums: &Arena<EnumDef>,
        enum_id: EnumId,
        variant: u32,
    ) -> Result<Self, MirVariantRefError> {
        let enum_index = enum_id.into_raw().into_u32() as usize;
        if enum_index >= enums.len() {
            return Err(MirVariantRefError::UnknownEnum { enum_id });
        }
        let variant_count = enums[enum_id].variants.len();
        if variant as usize >= variant_count {
            return Err(MirVariantRefError::VariantOutOfBounds {
                enum_id,
                variant,
                variant_count,
            });
        }
        Ok(Self { enum_id, variant })
    }

    pub const fn enum_id(self) -> EnumId {
        self.enum_id
    }

    pub const fn variant_index(self) -> u32 {
        self.variant
    }

    pub fn definition(self, enums: &Arena<EnumDef>) -> Result<&VariantDef, MirVariantRefError> {
        Self::new(enums, self.enum_id, self.variant)?;
        Ok(&enums[self.enum_id].variants[self.variant as usize])
    }

    pub fn enum_type(self, enums: &Arena<EnumDef>) -> Result<Type, MirVariantRefError> {
        self.definition(enums)?;
        Ok(Type::Enum(
            self.enum_id,
            enums[self.enum_id].type_arguments.clone(),
        ))
    }
}

/// A payload-field identity inseparably bound to its checked enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirVariantFieldRef {
    variant: MirVariantRef,
    field: u32,
}

impl MirVariantFieldRef {
    pub fn new(
        enums: &Arena<EnumDef>,
        variant: MirVariantRef,
        field: u32,
    ) -> Result<Self, MirVariantFieldRefError> {
        let definition = variant
            .definition(enums)
            .map_err(MirVariantFieldRefError::InvalidVariant)?;
        let field_count = definition.fields.len();
        if field as usize >= field_count {
            return Err(MirVariantFieldRefError::FieldOutOfBounds {
                variant,
                field,
                field_count,
            });
        }
        Ok(Self { variant, field })
    }

    pub const fn variant(self) -> MirVariantRef {
        self.variant
    }

    pub const fn field_index(self) -> u32 {
        self.field
    }

    pub fn definition(
        self,
        enums: &Arena<EnumDef>,
    ) -> Result<&VariantField, MirVariantFieldRefError> {
        let variant = self
            .variant
            .definition(enums)
            .map_err(MirVariantFieldRefError::InvalidVariant)?;
        variant
            .fields
            .get(self.field as usize)
            .ok_or(MirVariantFieldRefError::FieldOutOfBounds {
                variant: self.variant,
                field: self.field,
                field_count: variant.fields.len(),
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantRefError {
    UnknownEnum {
        enum_id: EnumId,
    },
    VariantOutOfBounds {
        enum_id: EnumId,
        variant: u32,
        variant_count: usize,
    },
}

impl std::fmt::Display for MirVariantRefError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownEnum { enum_id } => write!(
                formatter,
                "unknown MIR enum {}",
                enum_id.into_raw().into_u32()
            ),
            Self::VariantOutOfBounds {
                enum_id,
                variant,
                variant_count,
            } => write!(
                formatter,
                "variant {variant} is out of bounds for MIR enum {} with {variant_count} variants",
                enum_id.into_raw().into_u32()
            ),
        }
    }
}

impl std::error::Error for MirVariantRefError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantFieldRefError {
    InvalidVariant(MirVariantRefError),
    FieldOutOfBounds {
        variant: MirVariantRef,
        field: u32,
        field_count: usize,
    },
}

impl std::fmt::Display for MirVariantFieldRefError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVariant(error) => error.fmt(formatter),
            Self::FieldOutOfBounds {
                variant,
                field,
                field_count,
            } => write!(
                formatter,
                "field {field} is out of bounds for MIR enum {} variant {} with {field_count} fields",
                variant.enum_id().into_raw().into_u32(),
                variant.variant_index()
            ),
        }
    }
}

impl std::error::Error for MirVariantFieldRefError {}
