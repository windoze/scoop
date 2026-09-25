//! Source slot replay using one artifact's dispatch and the language Unit identity.

use crate::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey};

mod errors;
mod inheritance;
mod schemas;
pub use errors::*;
type Error = InheritanceSlotSourceBindingError;

#[derive(Debug)]
pub struct BoundInheritanceSlotSourcesV1<'s, 'a, 'f> {
    pub(super) dispatch: &'s BoundInheritanceDispatchSourcesV1<'a, 'f>,
    graph: CheckedNominalInheritanceGraphV1<'a>,
    unit: PersistentExactTypeId,
}

impl<'a, 'f> BoundInheritanceDispatchSourcesV1<'a, 'f> {
    pub fn bind_slot_sources<'s>(
        &'s self,
    ) -> Result<BoundInheritanceSlotSourcesV1<'s, 'a, 'f>, Error> {
        let nominal = scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id();
        let expected = ExactTypeKey::Nominal(nominal);

        let unit = PersistentExactTypeId::from_key(&expected)
            .map_err(|error| Error::Identity(error.to_string()))?;
        let entries = self.foundation.source().entries();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            self.foundation,
        )
        .map_err(Error::from_graph)?;
        Ok(BoundInheritanceSlotSourcesV1 {
            dispatch: self,
            graph,
            unit,
        })
    }
}

impl BoundInheritanceSlotSourcesV1<'_, '_, '_> {
    pub const fn graph(&self) -> &CheckedNominalInheritanceGraphV1<'_> {
        &self.graph
    }

    pub fn validate_contract<'c>(
        &self,
        owner: PersistentExactTypeId,
        contract: &'c InheritanceSlotContractV1,
    ) -> Result<CheckedInheritanceSourceSlotContractV1<'c>, InheritanceInterfaceSemanticError<Error>>
    {
        self.graph
            .validate_slot_source_contract(owner, contract, self)
    }
}

impl InheritanceSlotContractSemanticAuthority<Error> for BoundInheritanceSlotSourcesV1<'_, '_, '_> {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, Error> {
        Ok(self.unit)
    }
}
impl InheritanceSlotSourceSemanticAuthority<Error> for BoundInheritanceSlotSourcesV1<'_, '_, '_> {
    fn inheritance_callable_source(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, Error> {
        self.dispatch.callable(declaration).map_err(Into::into)
    }
    fn inheritance_slot_selection(
        &self,
        owner: PersistentExactTypeId,
        slot: scoop_identity::PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, Error> {
        self.dispatch.selection(owner, slot).map_err(Into::into)
    }
}
