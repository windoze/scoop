//! Artifact-bound protected callable sources, independent of public candidates.

use std::collections::BTreeMap;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, ExactTypeKey,
    PersistentExactTypeId, PersistentGenericTypeId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, SignatureTypeKey,
    SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::binding_keys;
use crate::*;

mod contracts;
mod errors;
mod inventory;
mod keys;
mod replay;
pub use errors::*;

/// Validated source contracts, without public lookup or materialization authority.
#[derive(Debug)]
pub struct BoundInheritanceProtectedCallableSourcesV1<'a, 'f> {
    pub(super) foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    pub(super) inventory: &'a CanonicalSourceInheritanceInventoriesV1,
    properties: &'a CanonicalInheritanceSourcePropertiesV1,
    callables: &'a CanonicalInheritanceSourceProtectedCallablesV1,
    keys: BTreeMap<CallableTemplateOrigin, &'f SourceDeclarationKey>,
    unit: PersistentTypeId,
}

impl<'a, 'f> BoundInheritancePropertySourcesV1<'a, 'f> {
    pub fn bind_protected_callable_sources(
        &self,
        callables: &'a CanonicalInheritanceSourceProtectedCallablesV1,
        core: &ImportedCoreFundamentalTypeProtocol,
        meter: &mut BudgetMeter,
    ) -> Result<
        BoundInheritanceProtectedCallableSourcesV1<'a, 'f>,
        InheritanceProtectedCallableBindingError,
    > {
        use InheritanceProtectedCallableBindingError as Error;
        meter.check_semantic_depth(1, &WirePath::root())?;
        meter.charge_nodes(1, &WirePath::root())?;
        inventory::validate(self, callables, meter)?;
        let keys = keys::bind(self, callables, meter)?;
        let unit = core.unit().persistent();
        let mut bound = BoundInheritanceProtectedCallableSourcesV1 {
            foundation: self.foundation,
            inventory: self.inventory,
            properties: self.properties,
            callables,
            keys,
            unit,
        };
        let entries = self.foundation.source().entries();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            self.foundation,
            meter,
        )
        .map_err(Error::Inheritance)?;
        for record in callables.records() {
            contracts::validate(&bound, record, meter)?;
            record
                .validate_source(&graph, &mut bound, meter)
                .map_err(|error| match error {
                    ProtectedCallableSemanticError::Resource(error) => Error::Resource(error),
                    other => Error::Semantic(Box::new(other)),
                })?;
        }
        Ok(bound)
    }
}

impl<'a, 'f> BoundInheritanceProtectedCallableSourcesV1<'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.foundation.source().entries().provider
    }
    pub const fn table(&self) -> &'a CanonicalInheritanceSourceProtectedCallablesV1 {
        self.callables
    }
    pub fn callable_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&'f SourceDeclarationKey, InheritanceProtectedCallableBindingError> {
        self.keys.get(&declaration).copied().ok_or(
            InheritanceProtectedCallableBindingError::MissingKey(declaration),
        )
    }
    pub fn callable_source(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&'a ProtectedCallableInterfaceV1, InheritanceProtectedCallableBindingError> {
        self.callables.get(declaration).ok_or(
            InheritanceProtectedCallableBindingError::MissingSource(declaration),
        )
    }
}

fn query(
    length: usize,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceProtectedCallableBindingError> {
    meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}

fn subject(
    declaration: CallableTemplateOrigin,
) -> Result<DefinitionOriginSubject, InheritanceProtectedCallableBindingError> {
    match declaration {
        CallableTemplateOrigin::Function(id) => Ok(DefinitionOriginSubject::Function(id)),
        CallableTemplateOrigin::GenericFunction(id) => {
            Ok(DefinitionOriginSubject::GenericFunction(id))
        }
        CallableTemplateOrigin::Accessor(id) => Ok(DefinitionOriginSubject::PropertyAccessor(id)),
        other => Err(InheritanceProtectedCallableBindingError::Declaration(other)),
    }
}
