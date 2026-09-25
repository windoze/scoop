use super::*;
use crate::{CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, SourceNominalId};
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub fn validate_dispatch_declarations(
        &self,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<(), NominalDispatchDeclarationError> {
        let mut required = BTreeMap::<SourceNominalId, BTreeSet<PersistentDispatchSlotId>>::new();
        for callable in callables.all_declarations() {
            let Some(owner) = callable.owner().nominal_owner() else {
                continue;
            };
            for slot in callable.slot_relations().values() {
                required.entry(owner).or_default().insert(*slot);
            }
        }
        for nominal in self.all_records() {
            let order = nominal.declaration_details().dispatch_order();

            let actual: BTreeSet<_> = order.declared_slots().collect();
            if actual != required.remove(&nominal.declaration()).unwrap_or_default() {
                return Err(NominalDispatchDeclarationError::Slots(
                    nominal.declaration(),
                ));
            }
            if let NominalDispatchOrderV1::Interface { parents, .. } = order {
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
