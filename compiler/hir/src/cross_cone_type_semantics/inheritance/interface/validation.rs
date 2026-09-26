use super::*;
use crate::{
    CallableModalityV1, CheckedNominalInheritanceGraphV1, DeclarationAccessSourceV1,
    InheritanceCallableDeclarationV1, InheritanceCallableSignatureV1,
    InheritanceSlotContractSemanticAuthority,
};
use scoop_identity::PersistentDispatchSlotId;

mod slots;

/// Callable signature and visibility from the shared declaration.
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

/// Shared callable declarations and resolved implementation choices for slots.
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
