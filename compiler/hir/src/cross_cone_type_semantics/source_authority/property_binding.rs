//! Logical property sources joined to owned keys, origins and dispatch sources.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    AccessorRole, ConeIdentity, DefinitionOriginSubject, ExactTypeKey, PersistentExactTypeId,
    PersistentPropertyAccessorId, PersistentPropertyId, PropertyOwner, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::binding_keys;
use crate::*;

mod access;
mod contracts;
mod errors;
mod inventory;
mod slots;
pub use errors::*;

/// Artifact-bound sources, without lookup, full signature or machine-use permission.
#[derive(Debug)]
pub struct BoundInheritancePropertySourcesV1<'a, 'f> {
    foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    properties: &'a CanonicalInheritanceSourcePropertiesV1,
    keys: BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>,
}

impl<'a, 'f> BoundInheritanceDispatchSourcesV1<'a, 'f> {
    pub fn bind_property_sources(
        &self,
        properties: &'a CanonicalInheritanceSourcePropertiesV1,
        meter: &mut BudgetMeter,
    ) -> Result<BoundInheritancePropertySourcesV1<'a, 'f>, InheritancePropertyBindingError> {
        use InheritancePropertyBindingError as Error;
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        inventory::validate(self, properties, meter)?;
        let entries = self.foundation.source().entries();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            self.foundation,
            meter,
        )
        .map_err(Error::Inheritance)?;
        binding_keys::charge_map(properties.records().len(), meter, &path)?;
        let mut keys = BTreeMap::new();
        for record in properties.records() {
            query(self.properties.len(), meter)?;
            let key = self
                .properties
                .get(&record.declaration())
                .copied()
                .ok_or(Error::MissingKey(record.declaration()))?;
            contracts::validate(self, &graph, key, record, meter)?;
            keys.insert(record.declaration(), key);
        }
        Ok(BoundInheritancePropertySourcesV1 {
            foundation: self.foundation,
            properties,
            keys,
        })
    }
}

impl<'a, 'f> BoundInheritancePropertySourcesV1<'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.foundation.source().entries().provider
    }
    pub const fn table(&self) -> &'a CanonicalInheritanceSourcePropertiesV1 {
        self.properties
    }
    pub fn property_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&'f SourceDeclarationKey, InheritancePropertyBindingError> {
        self.keys
            .get(&id)
            .copied()
            .ok_or(InheritancePropertyBindingError::MissingKey(id))
    }
    pub fn property_source(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&'a NominalSupportPropertyInterfaceV1, InheritancePropertyBindingError> {
        self.properties
            .get(id)
            .ok_or(InheritancePropertyBindingError::MissingSource(id))
    }
    pub fn property_shape(
        &self,
        id: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, InheritancePropertyBindingError> {
        let payload = payload(self.property_source(id)?)?;
        Ok(ProtectedPropertySourceShapeV1 {
            getter: payload.getter(),
            setter: match payload.mutability() {
                ProtectedPropertyMutabilityV1::ReadOnly => None,
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } => Some((*setter, setter_access.declared_visibility())),
            },
            representation: payload.representation(),
        })
    }
}

fn payload(
    record: &NominalSupportPropertyInterfaceV1,
) -> Result<&NominalSourcePropertyPayloadV1, InheritancePropertyBindingError> {
    match record.payload() {
        NominalSupportPropertyPayloadV1::Runtime { interface } => Ok(interface),
        NominalSupportPropertyPayloadV1::Const { .. } => Err(
            InheritancePropertyBindingError::NonRuntime(record.declaration()),
        ),
    }
}

fn query(length: usize, meter: &mut BudgetMeter) -> Result<(), InheritancePropertyBindingError> {
    meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}

fn exact_owner(
    owner: SourceNominalId,
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, InheritancePropertyBindingError> {
    let SourceNominalId::Concrete(owner) = owner else {
        return Err(InheritancePropertyBindingError::GenericOwner);
    };
    let key = ExactTypeKey::Nominal(owner);
    meter.charge_sha256(
        scoop_wire::encoded_length(&key)
            .map_err(|e| InheritancePropertyBindingError::Identity(e.to_string()))?,
        &WirePath::root(),
    )?;
    PersistentExactTypeId::from_key(&key)
        .map_err(|e| InheritancePropertyBindingError::Identity(e.to_string()))
}
