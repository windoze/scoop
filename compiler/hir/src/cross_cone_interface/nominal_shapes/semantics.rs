use std::{borrow::Cow, fmt};

use scoop_identity::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    NominalDeclarationOwner, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentFieldId, PersistentObjectValueId, SourceDeclarationIdentityError,
    SourceDeclarationKey,
};

use super::{
    EnumSourceFieldV1, EnumSourceVariantStyleV1, EnumSourceVariantV1, NominalSourceShapeV1,
    StructSourceFieldV1,
};
use crate::{
    CanonicalBinderListV1, NominalInterfaceShapeAuthority, PublicNominalKindV1,
    SignatureBinderScopeV1, SignatureTypeSemanticError, SourceNominalId,
};

/// Supplies the canonical identity key for each trusted id referenced by a
/// nominal source shape. Implementations must reject a missing id or a key
/// that was not used to derive that exact id.
pub trait NominalSourceShapeSemanticAuthority<E>: NominalInterfaceShapeAuthority<E> {
    fn struct_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, E>;

    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, E>;

    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, E>;

    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, E>;
}

impl NominalSourceShapeV1 {
    pub fn validate_semantics<A, E>(
        &self,
        declaration: SourceNominalId,
        expected_kind: PublicNominalKindV1,
        type_parameters: &CanonicalBinderListV1,
        authority: &mut A,
    ) -> Result<(), NominalSourceShapeSemanticError<E>>
    where
        A: NominalSourceShapeSemanticAuthority<E>,
    {
        let actual_kind = self.kind();
        if actual_kind != expected_kind {
            return Err(NominalSourceShapeSemanticError::Kind {
                expected: expected_kind,
                actual: actual_kind,
            });
        }

        let scope = type_parameters.signature_scope(None);
        match self {
            Self::Class | Self::Interface => Ok(()),
            Self::Intrinsic(representation) => representation
                .validate_binders(type_parameters)
                .map_err(NominalSourceShapeSemanticError::IntrinsicBinders),
            Self::Struct(shape) => {
                for (index, field) in shape.fields().iter().enumerate() {
                    validate_struct_field(field, declaration, &scope, authority).map_err(
                        |error| NominalSourceShapeSemanticError::StructField { index, error },
                    )?;
                }
                Ok(())
            }
            Self::Enum(shape) => {
                for (index, variant) in shape.variants().iter().enumerate() {
                    validate_enum_variant(variant, declaration, &scope, authority).map_err(
                        |error| NominalSourceShapeSemanticError::EnumVariant { index, error },
                    )?;
                }
                Ok(())
            }
            Self::Object(shape) => validate_object(shape.value(), declaration, authority)
                .map_err(NominalSourceShapeSemanticError::ObjectValue),
        }
    }
}

fn validate_struct_field<A, E>(
    field: &StructSourceFieldV1,
    declaration: SourceNominalId,
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), StructSourceFieldSemanticError<E>>
where
    A: NominalSourceShapeSemanticAuthority<E>,
{
    let key = authority
        .struct_field_key(field.field())
        .map_err(StructSourceFieldSemanticError::Reference)?;
    let actual = key.source_owner();
    if actual != Some(declaration) {
        return Err(StructSourceFieldSemanticError::Owner {
            expected: declaration,
            actual,
        });
    }
    scope
        .validate_signature_semantics(field.value_type(), authority)
        .map_err(StructSourceFieldSemanticError::ValueType)
}

fn validate_enum_variant<A, E>(
    variant: &EnumSourceVariantV1,
    declaration: SourceNominalId,
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), EnumSourceVariantSemanticError<E>>
where
    A: NominalSourceShapeSemanticAuthority<E>,
{
    let key = authority
        .enum_variant_key(variant.variant())
        .map_err(EnumSourceVariantSemanticError::Reference)?;
    let actual = key.source_owner();
    if actual != Some(declaration) {
        return Err(EnumSourceVariantSemanticError::Owner {
            expected: declaration,
            actual,
        });
    }

    for (index, field) in variant.fields().iter().enumerate() {
        validate_enum_field(field, variant, index, scope, authority)
            .map_err(|error| EnumSourceVariantSemanticError::Field { index, error })?;
    }
    Ok(())
}

fn validate_enum_field<A, E>(
    field: &EnumSourceFieldV1,
    variant: &EnumSourceVariantV1,
    index: usize,
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), EnumSourceFieldSemanticError<E>>
where
    A: NominalSourceShapeSemanticAuthority<E>,
{
    let key = authority
        .enum_variant_field_key(field.field())
        .map_err(EnumSourceFieldSemanticError::Reference)?;
    if key.variant() != variant.variant() {
        return Err(EnumSourceFieldSemanticError::Variant {
            expected: variant.variant(),
            actual: key.variant(),
        });
    }

    let expected_selector = match variant.style() {
        EnumSourceVariantStyleV1::Unit => return Err(EnumSourceFieldSemanticError::UnitField),
        EnumSourceVariantStyleV1::Positional => {
            let declaration_index = u32::try_from(index)
                .map_err(|_| EnumSourceFieldSemanticError::FieldIndexOverflow { index })?;
            EnumSourceFieldSelectorV1::Positional { declaration_index }
        }
        EnumSourceVariantStyleV1::Named | EnumSourceVariantStyleV1::Constructor => {
            EnumSourceFieldSelectorV1::Named
        }
    };
    if !expected_selector.matches(key.selector()) {
        return Err(EnumSourceFieldSemanticError::Selector {
            expected: expected_selector,
            actual: key.selector().clone(),
        });
    }

    scope
        .validate_signature_semantics(field.value_type(), authority)
        .map_err(EnumSourceFieldSemanticError::ValueType)
}

fn validate_object<A, E>(
    value: PersistentObjectValueId,
    declaration: SourceNominalId,
    authority: &mut A,
) -> Result<(), ObjectSourceShapeSemanticError<E>>
where
    A: NominalSourceShapeSemanticAuthority<E>,
{
    let key = authority
        .object_value_key(value)
        .map_err(ObjectSourceShapeSemanticError::Reference)?;
    let actual = NominalDeclarationOwner::from_source_declaration(&key)
        .map_err(ObjectSourceShapeSemanticError::Declaration)?;
    if actual != declaration {
        return Err(ObjectSourceShapeSemanticError::Owner {
            expected: declaration,
            actual,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumSourceFieldSelectorV1 {
    Positional { declaration_index: u32 },
    Named,
}

impl EnumSourceFieldSelectorV1 {
    fn matches(self, actual: &EnumVariantFieldSelector) -> bool {
        match (self, actual) {
            (
                Self::Positional { declaration_index },
                EnumVariantFieldSelector::Positional {
                    declaration_index: actual,
                },
            ) => declaration_index == *actual,
            (Self::Named, EnumVariantFieldSelector::Named(_)) => true,
            (Self::Positional { .. }, _) | (Self::Named, _) => false,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum StructSourceFieldSemanticError<E> {
    Reference(E),
    Owner {
        expected: SourceNominalId,
        actual: Option<SourceNominalId>,
    },
    ValueType(SignatureTypeSemanticError<E>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum EnumSourceFieldSemanticError<E> {
    Reference(E),
    UnitField,
    Variant {
        expected: PersistentEnumVariantId,
        actual: PersistentEnumVariantId,
    },
    FieldIndexOverflow {
        index: usize,
    },
    Selector {
        expected: EnumSourceFieldSelectorV1,
        actual: EnumVariantFieldSelector,
    },
    ValueType(SignatureTypeSemanticError<E>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum EnumSourceVariantSemanticError<E> {
    Reference(E),
    Owner {
        expected: SourceNominalId,
        actual: Option<SourceNominalId>,
    },
    Field {
        index: usize,
        error: EnumSourceFieldSemanticError<E>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ObjectSourceShapeSemanticError<E> {
    Reference(E),
    Declaration(SourceDeclarationIdentityError),
    Owner {
        expected: SourceNominalId,
        actual: SourceNominalId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum NominalSourceShapeSemanticError<E> {
    IntrinsicBinders(crate::NominalIntrinsicBinderError),
    Kind {
        expected: PublicNominalKindV1,
        actual: PublicNominalKindV1,
    },
    StructField {
        index: usize,
        error: StructSourceFieldSemanticError<E>,
    },
    EnumVariant {
        index: usize,
        error: EnumSourceVariantSemanticError<E>,
    },
    ObjectValue(ObjectSourceShapeSemanticError<E>),
}

impl<E: fmt::Display> fmt::Display for StructSourceFieldSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => write!(formatter, "invalid field identity: {error}"),
            Self::Owner { expected, actual } => write!(
                formatter,
                "field owner {actual:?} does not match nominal {expected:?}"
            ),
            Self::ValueType(error) => write!(formatter, "invalid field type: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StructSourceFieldSemanticError<E> {}

impl<E: fmt::Display> fmt::Display for EnumSourceFieldSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => write!(formatter, "invalid enum field identity: {error}"),
            Self::UnitField => formatter.write_str("unit enum variant must not contain fields"),
            Self::Variant { expected, actual } => write!(
                formatter,
                "enum field variant {actual} does not match enclosing variant {expected}"
            ),
            Self::FieldIndexOverflow { index } => {
                write!(formatter, "enum field index {index} exceeds u32")
            }
            Self::Selector { expected, actual } => write!(
                formatter,
                "enum field selector {actual:?} does not match {expected:?} source shape"
            ),
            Self::ValueType(error) => write!(formatter, "invalid enum field type: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumSourceFieldSemanticError<E> {}

impl<E: fmt::Display> fmt::Display for EnumSourceVariantSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => write!(formatter, "invalid enum variant identity: {error}"),
            Self::Owner { expected, actual } => write!(
                formatter,
                "enum variant owner {actual:?} does not match nominal {expected:?}"
            ),
            Self::Field { index, error } => {
                write!(formatter, "invalid enum field {index}: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumSourceVariantSemanticError<E> {}

impl<E: fmt::Display> fmt::Display for ObjectSourceShapeSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => write!(formatter, "invalid object value identity: {error}"),
            Self::Declaration(error) => {
                write!(formatter, "invalid object value declaration: {error}")
            }
            Self::Owner { expected, actual } => write!(
                formatter,
                "object value declaration {actual:?} does not match nominal {expected:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ObjectSourceShapeSemanticError<E> {}

impl<E: fmt::Display> fmt::Display for NominalSourceShapeSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntrinsicBinders(error) => error.fmt(formatter),
            Self::Kind { expected, actual } => write!(
                formatter,
                "source shape kind {actual:?} does not match nominal kind {expected:?}"
            ),
            Self::StructField { index, error } => {
                write!(formatter, "invalid struct field {index}: {error}")
            }
            Self::EnumVariant { index, error } => {
                write!(formatter, "invalid enum variant {index}: {error}")
            }
            Self::ObjectValue(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalSourceShapeSemanticError<E> {}

#[cfg(test)]
mod tests;
