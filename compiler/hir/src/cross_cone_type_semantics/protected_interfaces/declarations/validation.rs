use super::*;
use crate::{
    CanonicalNominalRepresentationSupportV1, CheckedNominalInheritanceGraphV1, DeclaredVisibilityV1,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

/// The required inventory comes from complete definition-side metadata. It
/// must not be inferred from the records supplied to this validator.
pub trait ProtectedDeclarationSemanticAuthority<E>: NestedNominalSemanticAuthority<E> {
    fn required_protected_declarations(&self) -> Result<&CanonicalProtectedDeclarationRefsV1, E>;
}

/// Complete source declarations and accessor relationships, without default
/// coverage, inheritance selection, or production materialization authority.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedDeclarationSourcesV1<'a> {
    table: &'a CanonicalProtectedDeclarationInterfacesV1,
}
impl<'a> CheckedProtectedDeclarationSourcesV1<'a> {
    pub const fn table(&self) -> &'a CanonicalProtectedDeclarationInterfacesV1 {
        self.table
    }
}
impl CanonicalProtectedDeclarationInterfacesV1 {
    pub fn validate_sources<'a, A: ProtectedDeclarationSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        representations: &CanonicalNominalRepresentationSupportV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedProtectedDeclarationSourcesV1<'a>, ProtectedDeclarationSemanticError<E>>
    {
        use ProtectedDeclarationSemanticError as Error;
        let required = authority
            .required_protected_declarations()
            .map_err(Error::Foundation)?;
        meter
            .charge_work(
                (self.records().len() as u64)
                    .saturating_add(required.values().len() as u64)
                    .saturating_mul(128),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        if !self
            .records()
            .iter()
            .map(ProtectedDeclarationInterfaceV1::reference)
            .eq(required.values().iter().copied())
        {
            return Err(Error::Table(ProtectedDeclarationTableError::Inventory));
        }
        for record in self.records() {
            meter
                .charge_nodes(1, &WirePath::root())
                .map_err(Error::Resource)?;
            match record {
                ProtectedDeclarationInterfaceV1::Callable(value) => {
                    value
                        .validate_source(graph, authority, meter)
                        .map_err(Error::Callable)?;
                }
                ProtectedDeclarationInterfaceV1::Constructor(value) => {
                    value
                        .validate_source(graph, authority, meter)
                        .map_err(Error::Callable)?;
                }
                ProtectedDeclarationInterfaceV1::Property(value) => {
                    value
                        .validate_source(graph, authority, meter)
                        .map_err(Error::Property)?;
                }
                ProtectedDeclarationInterfaceV1::NestedNominal(value) => {
                    value
                        .validate_source(graph, representations, authority, meter)
                        .map_err(Error::Nested)?;
                }
            }
        }
        // Every accessor selected below has already passed source validation.
        // This internal composition never accepts an unchecked external DTO.
        for record in self.records() {
            let ProtectedDeclarationInterfaceV1::Property(property) = record else {
                continue;
            };
            let getter = self.accessor(property.payload().getter(), meter)?;
            let setter = match property.payload().mutability() {
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } if setter_access.declared_visibility() == DeclaredVisibilityV1::Protected => {
                    Some(self.accessor(*setter, meter)?)
                }
                ProtectedPropertyMutabilityV1::ReadOnly
                | ProtectedPropertyMutabilityV1::ReadWrite { .. } => None,
            };
            property
                .validate_source(graph, authority, meter)
                .map_err(Error::Property)?
                .validate_resolved_accessor_records(getter, setter, meter)
                .map_err(Error::Accessor)?;
        }
        Ok(CheckedProtectedDeclarationSourcesV1 { table: self })
    }
    fn accessor<E>(
        &self,
        accessor: scoop_identity::PersistentPropertyAccessorId,
        meter: &mut BudgetMeter,
    ) -> Result<&ProtectedCallableInterfaceV1, ProtectedDeclarationSemanticError<E>> {
        meter
            .charge_work(
                128 * (u64::BITS - (self.records().len() as u64).leading_zeros()) as u64,
                &WirePath::root(),
            )
            .map_err(ProtectedDeclarationSemanticError::Resource)?;
        let reference = ProtectedDeclarationRefV1::Callable(ProtectedCallableDeclarationRefV1(
            CallableTemplateOrigin::Accessor(accessor),
        ));
        match self.get(reference) {
            Some(ProtectedDeclarationInterfaceV1::Callable(value)) => Ok(value),
            _ => Err(ProtectedDeclarationSemanticError::Table(
                ProtectedDeclarationTableError::AccessorClosure,
            )),
        }
    }
}
