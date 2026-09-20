//! Complete property and callable source binding for nominal declarations.
use super::binding_keys;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod access;
mod contracts;
mod core;
mod errors;
mod inventory;
mod keys;
mod replay;
pub use errors::*;
type Error = NominalMemberBindingError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalMemberPropertyProofV1 {
    Runtime(NominalSupportPropertyAccessProofV1),
    Const,
}

/// Atomic source binding only. Selection, defaults and dispatch implementation
/// replay still belong to the complete declarations transaction.
#[derive(Debug)]
pub struct BoundNominalMemberSourcesV1<'s, 'a, 'f> {
    pub(super) nominals: &'s BoundNominalSourceContractsV1<'a, 'f>,
    properties: &'s CanonicalNominalSourcePropertiesV1,
    callables: &'s CanonicalNominalSourceCallablesV1,
    property_keys: BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>,
    callable_keys: BTreeMap<CallableTemplateOrigin, &'f SourceDeclarationKey>,
    constants: BTreeMap<PersistentPropertyId, ConstPropertyDeclarationSourceV1>,
    proofs: BTreeMap<PersistentPropertyId, NominalMemberPropertyProofV1>,
    core: &'s ImportedCoreFundamentalTypeProtocol,
}
impl<'a, 'f> BoundNominalSourceContractsV1<'a, 'f> {
    pub fn bind_member_sources<'s>(
        &'s self,
        properties: &'s CanonicalNominalSourcePropertiesV1,
        callables: &'s CanonicalNominalSourceCallablesV1,
        core: &'s ImportedCoreFundamentalTypeProtocol,
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalMemberSourcesV1<'s, 'a, 'f>, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        inventory::validate(self, properties, callables, meter)?;
        let property_keys = keys::properties(self, properties, meter)?;
        let callable_keys = keys::callables(self, &property_keys, callables, meter)?;
        let mut bound = BoundNominalMemberSourcesV1 {
            nominals: self,
            properties,
            callables,
            property_keys,
            callable_keys,
            constants: BTreeMap::new(),
            proofs: BTreeMap::new(),
            core,
        };
        core::validate(&bound, core.unit().persistent(), meter)?;
        contracts::prepare(&mut bound, meter)?;
        let entries = self.foundation.source().entries();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            self.foundation,
            meter,
        )
        .map_err(Error::Inheritance)?;
        for record in callables.records() {
            record
                .validate_source(&graph, &mut bound, meter)
                .map_err(Error::from_callable)?;
        }
        contracts::properties(&mut bound, &graph, meter)?;
        Ok(bound)
    }
}
impl<'s, 'a, 'f> BoundNominalMemberSourcesV1<'s, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.nominals.foundation.source().entries().provider
    }
    pub const fn properties(&self) -> &'s CanonicalNominalSourcePropertiesV1 {
        self.properties
    }
    pub const fn callables(&self) -> &'s CanonicalNominalSourceCallablesV1 {
        self.callables
    }
    pub fn property_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        self.property_keys
            .get(&id)
            .copied()
            .ok_or(Error::MissingProperty(id))
    }
    pub fn callable_key(
        &self,
        id: CallableTemplateOrigin,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        self.callable_keys
            .get(&id)
            .copied()
            .ok_or(Error::MissingCallableKey(id))
    }
    pub fn property_source(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&'s NominalSupportPropertyInterfaceV1, Error> {
        self.properties.get(id).ok_or(Error::MissingProperty(id))
    }
    pub fn callable_source(
        &self,
        id: CallableTemplateOrigin,
    ) -> Result<&'s NominalSupportCallableInterfaceV1, Error> {
        self.callables.get(id).ok_or(Error::MissingCallable(id))
    }
    pub fn property_proof(
        &self,
        id: PersistentPropertyId,
    ) -> Result<NominalMemberPropertyProofV1, Error> {
        self.proofs
            .get(&id)
            .copied()
            .ok_or(Error::MissingProperty(id))
    }
}
fn query(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}
