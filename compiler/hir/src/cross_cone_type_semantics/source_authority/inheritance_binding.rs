//! Complete inheritance source queries from one artifact's bound transcripts.

use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentExactTypeId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<BoundInheritanceSourcesV1<'s, 'a, 'f>, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(3, &path)?;
        if !std::ptr::eq(self.foundation, constructors.foundation)
            || !std::ptr::eq(self.foundation, nominals.foundation)
            || !std::ptr::eq(self.foundation, slots.dispatch.foundation)
        {
            return Err(Error::FoundationMismatch);
        }
        for inventory in [
            self.inventory,
            constructors.inventory,
            slots.dispatch.inventory(),
        ] {
            super::binding_keys::charge_inheritance_inventory(inventory, meter, &path)?;
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
        meter.check_table_entries(
            entries.local_inheritance_edges.records().len() as u64,
            &path,
        )?;
        for edge in entries.local_inheritance_edges.records() {
            meter.charge_work(
                u64::from(self.inventory.records().len().max(1).ilog2())
                    + u64::from(nominals.table().records().len().max(1).ilog2())
                    + 2,
                &path,
            )?;
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
                .validate_source(slots.graph(), &mut bound, meter)
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
