//! Lookup keys for the immutable dependency declaration catalog.

use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, GcEffect, PersistentDispatchSlotId, PersistentPropertyAccessorId,
    PropertyOwner,
};

use super::catalog::{CallableCatalogEntry, PropertyCatalogEntry};
use super::intrinsics::CallableCatalogName;
use crate::{
    CallableImplementationV1, CallableOperatorRoleV1, IntrinsicFunctionKind,
    PublicDeclarationOwnerV1, SourceNominalId,
};

#[derive(Debug, Default)]
pub(super) struct CatalogIndexes {
    pub nominals: BTreeMap<SourceNominalId, NominalMembers>,
    pub accessors: BTreeMap<PersistentPropertyAccessorId, PropertyOwner>,
    pub intrinsics: BTreeMap<(IntrinsicFunctionKind, GcEffect), Vec<CallableTemplateOrigin>>,
}

#[derive(Debug, Default)]
pub(super) struct NominalMembers {
    pub functions: BTreeMap<String, Vec<CallableTemplateOrigin>>,
    pub operators: BTreeMap<CallableOperatorRoleV1, Vec<CallableTemplateOrigin>>,
    pub constructors: Vec<CallableTemplateOrigin>,
    pub slots: BTreeMap<PersistentDispatchSlotId, CallableTemplateOrigin>,
    pub contextual: Vec<CallableTemplateOrigin>,
    pub property_by_name: BTreeMap<String, PropertyOwner>,
    pub properties: Vec<PropertyOwner>,
}

impl CatalogIndexes {
    pub fn new(
        callables: &BTreeMap<CallableTemplateOrigin, Arc<CallableCatalogEntry>>,
        properties: &BTreeMap<PropertyOwner, PropertyCatalogEntry>,
    ) -> Self {
        let mut index = Self::default();
        // Preserve the catalog's declaration order inside each candidate list.
        for (&declaration, entry) in callables {
            let interface = &entry.interface;
            let effects = interface.effects();
            if let CallableImplementationV1::Intrinsic(kind) = effects.implementation() {
                index
                    .intrinsics
                    .entry((kind, effects.gc_effect()))
                    .or_default()
                    .push(declaration);
            }
            let PublicDeclarationOwnerV1::Nominal(owner) = interface.owner() else {
                continue;
            };
            let members = index.nominals.entry(owner).or_default();
            match &entry.name {
                CallableCatalogName::Function(name) => {
                    members
                        .functions
                        .entry(name.as_str().to_owned())
                        .or_default()
                        .push(declaration);
                    members
                        .operators
                        .entry(effects.operator_role())
                        .or_default()
                        .push(declaration);
                }
                CallableCatalogName::Constructor => members.constructors.push(declaration),
                CallableCatalogName::Accessor | CallableCatalogName::VariantConstructor(_) => {}
            }
            for &slot in interface.slot_relations().values() {
                members.slots.entry(slot).or_insert(declaration);
            }
            if !interface.context_parameters().is_empty()
                && interface.type_parameters().len_u32() == 0
            {
                members.contextual.push(declaration);
            }
        }
        for (&declaration, entry) in properties {
            let accessors = entry.interface.accessors();
            for accessor in Some(accessors.getter())
                .into_iter()
                .chain(accessors.setter())
            {
                index.accessors.entry(accessor).or_insert(declaration);
            }
            if let PublicDeclarationOwnerV1::Nominal(owner) = entry.interface.owner() {
                let members = index.nominals.entry(owner).or_default();
                members.properties.push(declaration);
                members
                    .property_by_name
                    .entry(entry.name.as_str().to_owned())
                    .or_insert(declaration);
            }
        }
        index
    }
}
