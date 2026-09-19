use super::*;
use crate::{
    CheckedDeclarationAccessSourceV1, CheckedNominalInheritanceGraphV1,
    ProtectedCallableSemanticAuthority, SignatureBinderScopeV1,
};
use scoop_identity::{SourceDeclarationKey, SourceDeclarationKind};
use scoop_wire::{BudgetMeter, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<CheckedProtectedPropertySourceV1<'a>, ProtectedPropertySemanticError<E>> {
        use ProtectedPropertySemanticError as Error;
        let payload = self.payload();
        let owner = graph.source(payload.owner()).ok_or(Error::Owner)?;
        if owner.key.declaration_kind() != SourceDeclarationKind::Class {
            return Err(Error::Owner);
        }
        let arity = owner.key.duplicate_signature().type_parameter_count();
        SignatureBinderScopeV1::for_declaration(0, (arity != 0).then_some(arity))
            .validate_signature_semantics_metered(
                payload.value_type(),
                authority,
                meter,
                &WirePath::root(),
            )
            .map_err(Error::Signature)?;
        let key = authority
            .property_source_key(self.declaration())
            .map_err(Error::Foundation)?;
        meter
            .charge_sha256(
                scoop_wire::encoded_length(key).map_err(Error::Encoding)?,
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        if PersistentPropertyId::from_source_declaration(key).ok() != Some(self.declaration())
            || key.origin() != owner.key.origin()
            || key.package() != owner.key.package()
            || key.owners().owners().split_last().map(|(_, outer)| outer)
                != Some(owner.key.owners().owners())
        {
            return Err(Error::Identity);
        }
        if authority
            .property_value_type(self.declaration())
            .map_err(Error::Foundation)?
            != payload.value_type()
        {
            return Err(Error::ValueType);
        }
        let shape = authority
            .property_source_shape(self.declaration())
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
            self.declaration(),
            payload.getter(),
            scoop_identity::AccessorRole::Getter,
            authority,
            meter,
        )?;
        let access = graph
            .check_declaration_source(self.declaration_access(), key, authority, meter)
            .map_err(Error::Source)?;
        if let ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = payload.mutability()
        {
            accessors::validate_key(
                self.declaration(),
                *setter,
                scoop_identity::AccessorRole::Setter,
                authority,
                meter,
            )?;
            let setter = graph
                .check_declaration_source(setter_access, key, authority, meter)
                .map_err(Error::Source)?;
            let getter_domain = graph
                .replay_declaration_access(access, meter)
                .map_err(Error::Domain)?;
            let setter_domain = graph
                .replay_declaration_access(setter, meter)
                .map_err(Error::Domain)?;
            if !getter_domain
                .lookup()
                .covers(setter_domain.lookup(), meter)
                .map_err(Error::Domain)?
            {
                return Err(Error::SetterDomain);
            }
        }
        Ok(CheckedProtectedPropertySourceV1 {
            record: self,
            access,
        })
    }
}
