//! Projection of public properties and necessary source declarations.

use super::signatures::HirInterfaceSignatureProjector;
use crate::{
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, ExportHir, HirPropertyIdentity,
    PropertyInterfaceRecordV1,
};
use scoop_identity::PropertyOwner as PersistentPropertyOwner;
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};
use std::collections::HashSet;

mod accessors;
mod declaration;
mod errors;
mod signature;
mod support;

pub(in crate::production) use accessors::project_representation as source_property_representation;
pub use errors::{
    ExportPropertyAccessorBuildError, PropertyInterfaceBuildError, PropertyNominalOwnerKind,
};

impl CanonicalPropertyInterfacesV1 {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, PropertyInterfaceBuildError> {
        Self::from_export_hir_with_budget(export, &mut BudgetMeter::new(DecodeLimits::default()))
    }

    pub fn from_export_hir_with_budget(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PropertyInterfaceBuildError> {
        let nominals = CanonicalNominalInterfacesV1::from_export_hir_with_budget(export, meter)
            .map_err(PropertyInterfaceBuildError::Nominals)?;
        Self::from_export_hir_with_nominals(export, &nominals, meter)
    }

    pub(in crate::production) fn from_export_hir_with_nominals(
        export: &ExportHir,
        nominals: &CanonicalNominalInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PropertyInterfaceBuildError> {
        use PropertyInterfaceBuildError as Error;
        let path = WirePath::root().field(4);
        let projector = HirInterfaceSignatureProjector::new(export);
        let surface = &export.public_surface;
        let count = surface.property_getters.len() as u64 + surface.property_setters.len() as u64;
        meter
            .charge_collection_slots(count, &path)
            .map_err(Error::Resource)?;
        meter.charge_work(count, &path).map_err(Error::Resource)?;
        let public_getters = surface
            .property_getters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let public_setters = surface
            .property_setters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut required = nominals
            .declared_source_properties(meter)
            .map_err(Error::Inventory)?;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, surface.properties.len(), &path)
            .map_err(Error::Resource)?;
        for &id in &surface.properties {
            query(meter, required.len())?;
            let data = declaration::project(export, &projector, id, meter)?;
            required.remove(&data.declaration());
            let (access, setter) = accessors::public_lookup(
                export,
                id,
                data.declaration(),
                &public_getters,
                &public_setters,
            )?;
            let property = data.declaration();
            records.push(
                PropertyInterfaceRecordV1::from_declaration(data, access, setter)
                    .map_err(|source| Error::Record { property, source })?,
            );
        }
        let support = support::project(export, &projector, required, meter)?;
        let table = Self::with_support(records, support).map_err(Error::Table)?;
        table
            .validate_declaration_inventory(nominals, meter)
            .map_err(Error::Inventory)?;
        Ok(table)
    }
}

fn query(meter: &mut BudgetMeter, count: usize) -> Result<(), PropertyInterfaceBuildError> {
    meter
        .charge_work(
            u64::from(count.max(1).ilog2()) + 1,
            &WirePath::root().field(4),
        )
        .map_err(PropertyInterfaceBuildError::Resource)
}

fn persistent_property_owner(identity: &HirPropertyIdentity) -> PersistentPropertyOwner {
    match identity {
        HirPropertyIdentity::Ordinary(record) => PersistentPropertyOwner::Property(record.id()),
        HirPropertyIdentity::Extension(record) => {
            PersistentPropertyOwner::ExtensionProperty(record.id())
        }
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
