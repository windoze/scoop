use std::collections::BTreeMap;

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, NominalInterfaceShapeAuthority, PublicNominalKindV1,
    PublicNominalShapeV1, SourceNominalId,
};
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, PersistentGenericTypeId, PersistentTypeId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

/// A transient type-shape index over already checked metadata. It grants no
/// lookup or access rights to the private declarations used for source typing.
pub(super) struct DefaultNominalShapes(BTreeMap<SourceNominalId, PublicNominalShapeV1>);

impl DefaultNominalShapes {
    pub(super) fn new<'a>(
        providers: impl IntoIterator<Item = (ConeIdentity, &'a CrossConeHirInterfaceSectionV1)>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, DefaultMetadataNominalError> {
        let mut result = Self(BTreeMap::new());
        for (provider, interface) in providers {
            meter.charge_work(1, path)?;
            for record in interface.nominal_interfaces().all_records() {
                result.insert(
                    record.declaration(),
                    PublicNominalShapeV1::new(record.kind(), record.type_parameters().len_u32()),
                    meter,
                    path,
                )?;
            }
            for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
                let record = builtin.identity_record();
                if provider == record.key().origin() {
                    let kind = match builtin {
                        CoreBuiltinNominal::Unit => PublicNominalKindV1::Struct,
                        CoreBuiltinNominal::Any => PublicNominalKindV1::Class,
                    };
                    result.insert(
                        SourceNominalId::Concrete(record.id()),
                        PublicNominalShapeV1::new(kind, 0),
                        meter,
                        path,
                    )?;
                }
            }
        }
        Ok(result)
    }

    fn insert(
        &mut self,
        declaration: SourceNominalId,
        shape: PublicNominalShapeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultMetadataNominalError> {
        meter.check_table_entries(self.0.len() as u64 + 1, path)?;
        meter.charge_work(u64::from(self.0.len().max(1).ilog2()) + 1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_collection_slots(1, path)?;
        if self.0.insert(declaration, shape).is_some() {
            return Err(DefaultMetadataNominalError::Duplicate(declaration));
        }
        Ok(())
    }

    pub(super) fn get(
        &self,
        declaration: SourceNominalId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.0
            .get(&declaration)
            .copied()
            .ok_or(DefaultMetadataNominalError::Missing(declaration))
    }
}

impl NominalInterfaceShapeAuthority<DefaultMetadataNominalError> for DefaultNominalShapes {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.get(SourceNominalId::Concrete(declaration))
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.get(SourceNominalId::GenericTemplate(declaration))
    }
}

#[derive(Debug)]
pub enum DefaultMetadataNominalError {
    Resource(WireError),
    Missing(SourceNominalId),
    Duplicate(SourceNominalId),
}
impl From<WireError> for DefaultMetadataNominalError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefaultMetadataNominalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Missing(declaration) => write!(
                f,
                "default source type {declaration:?} has no reachable declaration"
            ),
            Self::Duplicate(declaration) => write!(
                f,
                "default source type {declaration:?} occurs in multiple provider tables"
            ),
        }
    }
}
impl std::error::Error for DefaultMetadataNominalError {}
