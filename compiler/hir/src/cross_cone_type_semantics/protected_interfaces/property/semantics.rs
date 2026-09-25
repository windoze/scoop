use super::*;
use crate::{
    CheckedDeclarationAccessSourceV1, CheckedNominalInheritanceGraphV1,
    ProtectedCallableSemanticAuthority, SignatureBinderScopeV1,
};
use scoop_identity::{SourceDeclarationKey, SourceDeclarationKind};

mod accessors;
mod errors;
pub use errors::*;

/// Definition-side logical property facts, independent of the transported
/// interface record and its calculated access domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedPropertySourceShapeV1 {
    pub getter: PersistentPropertyAccessorId,
    pub setter: Option<(PersistentPropertyAccessorId, DeclaredVisibilityV1)>,
    pub representation: PropertyRepresentationV1,
}
pub trait ProtectedPropertySemanticAuthority<E>: ProtectedCallableSemanticAuthority<E> {
    fn property_source_key(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, E>;
    fn property_source_shape(
        &self,
        property: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, E>;
}

/// Source identity, logical type, accessor roles and setter domain have been
/// replayed. Complete accessor/slot/export closure remains a separate proof.
#[derive(Debug)]
pub struct CheckedProtectedPropertySourceV1<'a> {
    record: &'a ProtectedPropertyInterfaceV1,
    access: CheckedDeclarationAccessSourceV1<'a>,
}
impl CheckedProtectedPropertySourceV1<'_> {
    pub const fn record(&self) -> &ProtectedPropertyInterfaceV1 {
        self.record
    }
    pub const fn declaration_access(&self) -> CheckedDeclarationAccessSourceV1<'_> {
        self.access
    }
}

impl ProtectedPropertyInterfaceV1 {
    pub fn validate_source<'a, A: ProtectedPropertySemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedProtectedPropertySourceV1<'a>, ProtectedPropertySemanticError<E>> {
        if graph
            .source(self.payload().owner())
            .is_none_or(|owner| owner.key.declaration_kind() != SourceDeclarationKind::Class)
        {
            return Err(ProtectedPropertySemanticError::Owner);
        }
        let access = validate_source_contract(
            self.declaration(),
            self.declaration_access(),
            self.payload(),
            graph,
            authority,
        )?;
        Ok(CheckedProtectedPropertySourceV1 {
            record: self,
            access,
        })
    }
}

pub(in crate::cross_cone_type_semantics::protected_interfaces) fn validate_source_contract<
    'a,
    A: ProtectedPropertySemanticAuthority<E>,
    E,
>(
    declaration: PersistentPropertyId,
    source: &'a DeclarationAccessSourceV1,
    payload: &NominalSourcePropertyPayloadV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, ProtectedPropertySemanticError<E>> {
    validate_source_parts(declaration, source, payload, graph, authority, true)
}

pub(in crate::cross_cone_type_semantics::protected_interfaces) fn validate_source_template_contract<
    'a,
    A: ProtectedPropertySemanticAuthority<E>,
    E,
>(
    declaration: PersistentPropertyId,
    source: &'a DeclarationAccessSourceV1,
    payload: &NominalSourcePropertyPayloadV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, ProtectedPropertySemanticError<E>> {
    if !source
        .lexical_owners()
        .iter()
        .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)))
    {
        return Err(ProtectedPropertySemanticError::Owner);
    }
    validate_source_parts(declaration, source, payload, graph, authority, false)
}

#[allow(clippy::too_many_arguments)]
fn validate_source_parts<'a, A: ProtectedPropertySemanticAuthority<E>, E>(
    declaration: PersistentPropertyId,
    source: &'a DeclarationAccessSourceV1,
    payload: &NominalSourcePropertyPayloadV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,

    replay_domains: bool,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, ProtectedPropertySemanticError<E>> {
    use ProtectedPropertySemanticError as Error;
    let owner = graph.source(payload.owner()).ok_or(Error::Owner)?;
    let arity = owner.key.duplicate_signature().type_parameter_count();
    SignatureBinderScopeV1::for_declaration(0, (arity != 0).then_some(arity))
        .validate_signature_semantics(payload.value_type(), authority)
        .map_err(Error::Signature)?;
    let key = authority
        .property_source_key(declaration)
        .map_err(Error::Foundation)?;

    if PersistentPropertyId::from_source_declaration(key).ok() != Some(declaration)
        || key.origin() != owner.key.origin()
        || key.package() != owner.key.package()
        || key.owners().owners().split_last().map(|(_, outer)| outer)
            != Some(owner.key.owners().owners())
    {
        return Err(Error::Identity);
    }
    if authority
        .property_value_type(declaration)
        .map_err(Error::Foundation)?
        != payload.value_type()
    {
        return Err(Error::ValueType);
    }
    let shape = authority
        .property_source_shape(declaration)
        .map_err(Error::Foundation)?;
    let actual_setter = match payload.mutability() {
        ProtectedPropertyMutabilityV1::ReadOnly => None,
        ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } => Some((*setter, setter_access.declared_visibility())),
    };
    if shape.getter != payload.getter()
        || shape.setter != actual_setter
        || shape.representation != payload.representation()
    {
        return Err(Error::SourceShape);
    }
    accessors::validate_key(
        declaration,
        payload.getter(),
        scoop_identity::AccessorRole::Getter,
        authority,
    )?;
    let access = graph
        .check_declaration_source(source, key, authority)
        .map_err(Error::Source)?;
    if let ProtectedPropertyMutabilityV1::ReadWrite {
        setter,
        setter_access,
    } = payload.mutability()
    {
        accessors::validate_key(
            declaration,
            *setter,
            scoop_identity::AccessorRole::Setter,
            authority,
        )?;
        let setter = graph
            .check_declaration_source(setter_access, key, authority)
            .map_err(Error::Source)?;
        if replay_domains {
            let getter_domain = graph
                .replay_declaration_access(access)
                .map_err(Error::Domain)?;
            let setter_domain = graph
                .replay_declaration_access(setter)
                .map_err(Error::Domain)?;
            if !getter_domain
                .lookup()
                .covers(setter_domain.lookup())
                .map_err(Error::Domain)?
            {
                return Err(Error::SetterDomain);
            }
        }
    } else if replay_domains {
        graph
            .replay_declaration_access(access)
            .map_err(Error::Domain)?;
    }
    Ok(access)
}
