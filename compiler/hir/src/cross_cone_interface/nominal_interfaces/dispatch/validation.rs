use super::*;
use crate::{CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, SourceNominalId};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub fn validate_dispatch_declarations(
        &self,
        callables: &CanonicalCallableInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), NominalDispatchDeclarationError> {
        let path = WirePath::root();
        let mut required = BTreeMap::<SourceNominalId, BTreeSet<PersistentDispatchSlotId>>::new();
        for callable in callables.all_declarations() {
            meter.charge_work(1, &path)?;
            let Some(owner) = callable.owner().nominal_owner() else {
                continue;
            };
            for slot in callable.slot_relations().values() {
                meter.charge_work(
                    1 + u64::from(callables.declaration_count().max(1).ilog2()),
                    &path,
                )?;
                meter.charge_collection_slots(2, &path)?;
                required.entry(owner).or_default().insert(*slot);
            }
        }
        for nominal in self.all_records() {
            let order = nominal.declaration_details().dispatch_order();
            let bytes = scoop_wire::encoded_length(order)
                .map_err(NominalDispatchDeclarationError::Encoding)?;
            meter.charge_work(
                bytes.saturating_mul(1 + u64::from(bytes.max(1).ilog2())),
                &path,
            )?;
            meter.charge_collection_slots(order.declared_slots().count() as u64, &path)?;
            let actual: BTreeSet<_> = order.declared_slots().collect();
            if actual != required.remove(&nominal.declaration()).unwrap_or_default() {
                return Err(NominalDispatchDeclarationError::Slots(
                    nominal.declaration(),
                ));
            }
            if let NominalDispatchOrderV1::Interface { parents, .. } = order {
                meter.charge_collection_slots(parents.len() as u64, &path)?;
                let parents: BTreeSet<_> = parents.iter().collect();
                if !parents.into_iter().eq(nominal.exact_supertypes().values()) {
                    return Err(NominalDispatchDeclarationError::Parents(
                        nominal.declaration(),
                    ));
                }
            }
        }
        if let Some(owner) = required.keys().next() {
            return Err(NominalDispatchDeclarationError::Slots(*owner));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum NominalDispatchDeclarationError {
    Resource(scoop_wire::WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Slots(SourceNominalId),
    Parents(SourceNominalId),
}
impl From<scoop_wire::WireError> for NominalDispatchDeclarationError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for NominalDispatchDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Slots(owner) => write!(
                f,
                "nominal {owner:?} dispatch order does not cover its declared slots"
            ),
            Self::Parents(owner) => write!(
                f,
                "interface {owner:?} dispatch parent order differs from its declared supertypes"
            ),
        }
    }
}
impl std::error::Error for NominalDispatchDeclarationError {}
