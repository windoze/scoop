//! Complete inheritance source queries from one artifact's bound transcripts.

use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentExactTypeId, SourceDeclarationKey};

mod callables;
mod errors;
mod graph;
mod interfaces;
mod schemas;
pub use errors::*;
type Error = InheritanceSourceBindingError;

/// Independent source authority only. Candidate inheritance, protected
/// declarations, default expansion and machine-use require their own proofs.
#[derive(Debug)]
pub struct BoundInheritanceSourcesV1<'s, 'a, 'f> {
    protected: &'s BoundInheritanceProtectedCallableSourcesV1<'a, 'f>,
    constructors: &'s BoundInheritanceConstructorSourcesV1<'a, 'f>,
    nominals: &'s BoundNominalSourceContractsV1<'a, 'f>,
    slots: &'s BoundInheritanceSlotSourcesV1<'s, 'a, 'f>,
}

impl<'a, 'f> BoundInheritanceProtectedCallableSourcesV1<'a, 'f> {
    pub fn bind_inheritance_sources<'s>(
        &'s self,
        constructors: &'s BoundInheritanceConstructorSourcesV1<'a, 'f>,
        nominals: &'s BoundNominalSourceContractsV1<'a, 'f>,
        slots: &'s BoundInheritanceSlotSourcesV1<'s, 'a, 'f>,
    ) -> Result<BoundInheritanceSourcesV1<'s, 'a, 'f>, Error> {
        if !std::ptr::eq(self.foundation, constructors.foundation)
            || !std::ptr::eq(self.foundation, nominals.foundation)
            || !std::ptr::eq(self.foundation, slots.dispatch.foundation)
        {
            return Err(Error::FoundationMismatch);
        }

        if self.inventory != constructors.inventory || self.inventory != slots.dispatch.inventory()
        {
            return Err(Error::Inventory);
        }
        let mut bound = BoundInheritanceSourcesV1 {
            protected: self,
            constructors,
            nominals,
            slots,
        };
        let entries = self.foundation.source().entries();

        for edge in entries.local_inheritance_edges.records() {
            let node = slots
                .graph()
                .get(edge.owner())
                .ok_or(Error::MissingOwner(edge.owner()))?;
            if nominals.nominal_source(node.source())?.modality() != edge.modality() {
                return Err(Error::Modality(node.source()));
            }
        }
        for record in constructors.table().records() {
            record
                .validate_source(slots.graph(), &mut bound)
                .map_err(Error::from_constructor)?;
        }
        Ok(bound)
    }
}

impl BoundInheritanceSourcesV1<'_, '_, '_> {
    pub fn provider(&self) -> scoop_identity::ConeIdentity {
        self.protected.provider()
    }
    pub const fn graph(&self) -> &CheckedNominalInheritanceGraphV1<'_> {
        self.slots.graph()
    }
}
