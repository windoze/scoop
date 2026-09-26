use super::*;
use crate::{
    CallableModalityV1, CanonicalPersistentIdsV1, CheckedNominalInheritanceGraphV1,
    CheckedProtectedDeclarationSourcesV1, DeclarationAccessSourceV1,
    InheritanceCallableDeclarationV1, InheritanceCallableSignatureV1,
    InheritanceSlotContractSemanticAuthority, NominalSupportCallableSemanticAuthority,
    NominalSupportConstructorInterfaceV1,
};
use scoop_identity::{PersistentConstructorId, PersistentDispatchSlotId};

mod declarations;
mod slots;

/// Source facts projected independently from the declaration's actual checked
/// source contract. This is not recovered from a transported slot record.
pub struct InheritanceSourceCallableFactsV1<'a> {
    pub signature: &'a InheritanceCallableSignatureV1,
    pub modality: CallableModalityV1,
    pub declaration_access: &'a DeclarationAccessSourceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceSourceSlotSelectionV1 {
    Abstract,
    Concrete(InheritanceCallableDeclarationV1),
    InterfaceDefault(InheritanceCallableDeclarationV1),
}

pub trait NominalInheritanceInterfaceSemanticAuthority<E>:
    InheritanceSlotSourceSemanticAuthority<E> + NominalSupportCallableSemanticAuthority<E>
{
    fn required_inheritance_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, E>;
    fn required_inheritance_constructors(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, E>;
    fn required_inheritance_protected_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, E>;
    fn constructor_source(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&NominalSupportConstructorInterfaceV1, E>;
}

/// Independent callable contracts and sealed implementation choices for slots.
pub trait InheritanceSlotSourceSemanticAuthority<E>:
    InheritanceSlotContractSemanticAuthority<E>
{
    fn inheritance_callable_source(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, E>;
    fn inheritance_slot_selection(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, E>;
}

pub use slots::CheckedInheritanceSourceSlotContractV1;

/// Proves complete source/graph/slot joins. Representation, default coverage,
/// terminal provider and selected-use closure remain section-level obligations.
#[derive(Clone, Copy, Debug)]
pub struct CheckedNominalInheritanceInterfacesV1<'a> {
    table: &'a CanonicalNominalInheritanceInterfacesV1,
}
impl<'a> CheckedNominalInheritanceInterfacesV1<'a> {
    pub const fn table(&self) -> &'a CanonicalNominalInheritanceInterfacesV1 {
        self.table
    }
}

impl CanonicalNominalInheritanceInterfacesV1 {
    pub fn validate_interfaces<'a, A: NominalInheritanceInterfaceSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        protected: CheckedProtectedDeclarationSourcesV1<'_>,
        authority: &mut A,
    ) -> Result<CheckedNominalInheritanceInterfacesV1<'a>, InheritanceInterfaceSemanticError<E>>
    {
        use InheritanceInterfaceSemanticError as Error;
        let required = authority
            .required_inheritance_owners()
            .map_err(Error::Foundation)?;

        if !self
            .records()
            .iter()
            .map(NominalInheritanceInterfaceV1::owner)
            .eq(required.values().iter().copied())
        {
            return Err(Error::Inventory);
        }
        for record in self.records() {
            let node = graph.get(record.owner()).ok_or(Error::Edges)?;
            compare(record.edges(), node.edges()).map_err(|error| match error {
                Error::SourceContract => Error::Edges,
                other => other,
            })?;
            let schemas = authority
                .schemas(record.owner())
                .map_err(Error::Foundation)?;
            compare(record.slot_schemas(), schemas)?;
            graph
                .validate_slot_schemas(record.owner(), authority)
                .map_err(Error::Schema)?;
            declarations::validate(record, node.source(), graph, protected, authority)?;
            slots::validate(record, graph, authority)?;
        }
        Ok(CheckedNominalInheritanceInterfacesV1 { table: self })
    }
}

fn compare<T: WireEncode + PartialEq, E>(
    left: &T,
    right: &T,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    use InheritanceInterfaceSemanticError as Error;

    if left != right {
        return Err(Error::SourceContract);
    }
    Ok(())
}

mod selection;
pub use selection::DecodedInheritanceSourceSlotSelectionV1;
