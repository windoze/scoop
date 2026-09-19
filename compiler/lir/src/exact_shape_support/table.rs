use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentTypeId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1, LirTargetProfile,
    OdrFreeLirFoundation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalParamFreeShapeSupportExportsV1(Arc<ShapeSupportTable>);

#[derive(Debug, Eq, PartialEq)]
struct ShapeSupportTable {
    provider: ConeIdentity,
    target: LirTargetProfile,
    records: Vec<ParamFreeShapeSupportExportV1>,
}

impl CanonicalParamFreeShapeSupportExportsV1 {
    pub fn from_sources(
        sources: &[SourceDeclarationKey],
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ParamFreeShapeSupportTableError> {
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, sources.len(), &WirePath::root())?;
        for source in sources {
            records.push(ParamFreeShapeSupportExportV1::replay(
                source,
                layouts,
                descriptors,
                foundation,
                meter,
            )?);
        }
        Self::try_new(sources, layouts, descriptors, foundation, records, meter)
    }

    pub fn try_new(
        sources: &[SourceDeclarationKey],
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ParamFreeShapeSupportExportV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ParamFreeShapeSupportTableError> {
        let path = WirePath::root();
        let count = records.len() as u64;
        meter.check_table_entries(count, &path)?;
        meter.charge_collection_slots(count, &path)?;
        let comparisons = count
            .checked_mul(u64::from(count.max(1).ilog2()) + 1)
            .ok_or(ParamFreeShapeSupportTableError::CountOverflow)?;
        meter.charge_work(comparisons, &path)?;
        if layouts.provider() != foundation.producer()
            || descriptors.provider() != foundation.producer()
        {
            return Err(ParamFreeShapeSupportTableError::TableProvider);
        }
        if layouts.target() != descriptors.target() {
            return Err(ParamFreeShapeSupportTableError::TableTarget);
        }
        if foundation.producer() == ConeIdentity::CORE {
            return if sources.is_empty() && records.is_empty() {
                Ok(Self(Arc::new(ShapeSupportTable {
                    provider: ConeIdentity::CORE,
                    target: layouts.target(),
                    records,
                })))
            } else {
                Err(ParamFreeShapeSupportTableError::CoreUsesExistingAuthority)
            };
        }

        records.sort_unstable_by_key(ParamFreeShapeSupportExportV1::source_nominal);
        for pair in records.windows(2) {
            if pair[0].source_nominal() == pair[1].source_nominal() {
                return Err(ParamFreeShapeSupportTableError::Duplicate(
                    pair[0].source_nominal(),
                ));
            }
        }
        let mut expected = Vec::new();
        meter.try_reserve_collection_slots(&mut expected, sources.len(), &path)?;
        for source in sources {
            expected.push(ParamFreeShapeSupportExportV1::replay(
                source,
                layouts,
                descriptors,
                foundation,
                meter,
            )?);
        }
        expected.sort_unstable_by_key(ParamFreeShapeSupportExportV1::source_nominal);
        for pair in expected.windows(2) {
            if pair[0].source_nominal() == pair[1].source_nominal() {
                return Err(ParamFreeShapeSupportTableError::DuplicateRequiredSource(
                    pair[0].source_nominal(),
                ));
            }
        }
        if records.len() != expected.len() {
            return Err(ParamFreeShapeSupportTableError::Coverage);
        }
        for (record, expected) in records.iter().zip(&expected) {
            if record.provider() != foundation.producer() {
                return Err(ParamFreeShapeSupportTableError::Provider(
                    record.source_nominal(),
                ));
            }
            if record.target() != layouts.target() {
                return Err(ParamFreeShapeSupportTableError::Target(
                    record.source_nominal(),
                ));
            }
            if record != expected {
                return Err(ParamFreeShapeSupportTableError::RecordMismatch(
                    record.source_nominal(),
                ));
            }
        }
        Ok(Self(Arc::new(ShapeSupportTable {
            provider: foundation.producer(),
            target: layouts.target(),
            records,
        })))
    }

    pub fn provider(&self) -> ConeIdentity {
        self.0.provider
    }

    pub fn target(&self) -> LirTargetProfile {
        self.0.target
    }

    pub fn records(&self) -> &[ParamFreeShapeSupportExportV1] {
        &self.0.records
    }

    pub fn get(&self, source: PersistentTypeId) -> Option<&ParamFreeShapeSupportExportV1> {
        self.0
            .records
            .binary_search_by_key(&source, ParamFreeShapeSupportExportV1::source_nominal)
            .ok()
            .map(|index| &self.0.records[index])
    }
}
