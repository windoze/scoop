use super::{
    NominalSourceCallablePayloadV1, ProtectedCallableInterfaceV1, ProtectedCallablePayloadV1,
    ProtectedConstructorInterfaceV1,
};
use crate::{
    CheckedDeclarationAccessSourceV1, CheckedNominalInheritanceGraphV1, InheritanceGraphError,
    NominalInheritanceSemanticAuthority, NominalInterfaceShapeAuthority,
    SignatureTypeSemanticError, TypeParameterBinderSemanticValidationError,
};
use scoop_identity::{
    CallableTemplateOrigin, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
    PropertyAccessorKey, SourceDeclarationKey,
};
use scoop_wire::WireError;
use std::fmt;

mod identity;
pub(super) mod signature;

/// Independently resolved source keys, nominal shapes, logical property types,
/// and the validated core Unit role. No ordinary public lookup record is built.
pub trait ProtectedCallableSemanticAuthority<E>:
    NominalInheritanceSemanticAuthority<E> + NominalInterfaceShapeAuthority<E>
{
    fn callable_source_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, E>;
    fn property_accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, E>;
    fn property_value_type(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&scoop_identity::SignatureTypeKey, E>;
    fn unit_type(&self) -> Result<PersistentTypeId, E>;
}

/// Proves source identity, signature/binder shape, and protected declaration
/// origin only. Slot selection, source defaults, and complete export closure
/// are established before the enclosing interface can be selected.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedCallableSourceV1<'a> {
    declaration: CallableTemplateOrigin,
    payload: &'a ProtectedCallablePayloadV1,
    access: CheckedDeclarationAccessSourceV1<'a>,
}
impl<'a> CheckedProtectedCallableSourceV1<'a> {
    pub const fn declaration(&self) -> CallableTemplateOrigin {
        self.declaration
    }
    pub const fn payload(&self) -> &'a ProtectedCallablePayloadV1 {
        self.payload
    }
    pub const fn declaration_access(&self) -> CheckedDeclarationAccessSourceV1<'a> {
        self.access
    }
}

impl ProtectedCallableInterfaceV1 {
    pub fn validate_source<'a, A: ProtectedCallableSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedProtectedCallableSourceV1<'a>, ProtectedCallableSemanticError<E>> {
        validate(
            self.declaration(),
            self.payload(),
            self.declaration_access(),
            graph,
            authority,
        )
    }
}
impl ProtectedConstructorInterfaceV1 {
    pub fn validate_source<'a, A: ProtectedCallableSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedProtectedCallableSourceV1<'a>, ProtectedCallableSemanticError<E>> {
        validate(
            CallableTemplateOrigin::Constructor(self.declaration()),
            self.payload(),
            self.declaration_access(),
            graph,
            authority,
        )
    }
}
fn validate<'a, A: ProtectedCallableSemanticAuthority<E>, E>(
    declaration: CallableTemplateOrigin,
    payload: &'a ProtectedCallablePayloadV1,
    source: &'a crate::DeclarationAccessSourceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,
) -> Result<CheckedProtectedCallableSourceV1<'a>, ProtectedCallableSemanticError<E>> {
    let owner = graph
        .source(payload.owner())
        .ok_or(ProtectedCallableSemanticError::Owner)?;
    if owner.key.declaration_kind() != scoop_identity::SourceDeclarationKind::Class {
        return Err(ProtectedCallableSemanticError::Owner);
    }
    let access =
        validate_source_contract(declaration, payload, owner.key, source, graph, authority)?;
    Ok(CheckedProtectedCallableSourceV1 {
        declaration,
        payload,
        access,
    })
}

pub(super) fn validate_source_contract<'a, A: ProtectedCallableSemanticAuthority<E>, E>(
    declaration: CallableTemplateOrigin,
    payload: &NominalSourceCallablePayloadV1,
    owner: &SourceDeclarationKey,
    source: &'a crate::DeclarationAccessSourceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, ProtectedCallableSemanticError<E>> {
    signature::validate_types(
        payload,
        owner.duplicate_signature().type_parameter_count(),
        authority,
    )?;
    let key = identity::validate(declaration, payload, owner, authority)?;
    graph
        .check_declaration_source(source, key, authority)
        .map_err(ProtectedCallableSemanticError::Source)
}

#[derive(Debug)]
pub enum ProtectedCallableSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(InheritanceGraphError<E>),
    Signature(SignatureTypeSemanticError<E>),
    Binders(TypeParameterBinderSemanticValidationError<E>),
    Owner,
    Identity,
    ParameterShape,
    Result,
}
impl<E: fmt::Display> fmt::Display for ProtectedCallableSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Binders(error) => error.fmt(f),
            Self::Owner => {
                f.write_str("protected callable requires a canonical lexical class owner")
            }
            Self::Identity => {
                f.write_str("protected callable source identity or owner site disagrees")
            }
            Self::ParameterShape => {
                f.write_str("protected callable parameters disagree with the source declaration")
            }
            Self::Result => f.write_str(
                "protected constructor or accessor result disagrees with its source contract",
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedCallableSemanticError<E> {}
