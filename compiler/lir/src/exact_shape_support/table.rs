use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentTypeId, SourceDeclarationKey};
use scoop_wire::WirePath;

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
    ) -> Result<Self, ParamFreeShapeSupportTableError> {
        Self::from_source_refs(sources.iter(), layouts, descriptors, foundation)
    }

    pub fn from_source_refs<'a>(
        sources: impl ExactSizeIterator<Item = &'a SourceDeclarationKey>,
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, ParamFreeShapeSupportTableError> {
        validate_inputs(layouts, descriptors, foundation)?;
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, sources.len(), &path)?;
        for source in sources {
            records.push(ParamFreeShapeSupportExportV1::replay(
                source,
                layouts,
                descriptors,
                foundation,
            )?);
        }
        sort_records(&mut records)?;
        for pair in records.windows(2) {
            if pair[0].source_nominal() == pair[1].source_nominal() {
                return Err(ParamFreeShapeSupportTableError::DuplicateRequiredSource(
                    pair[0].source_nominal(),
                ));
            }
        }
        Ok(Self(Arc::new(ShapeSupportTable {
            provider: foundation.producer(),
            target: layouts.target(),
            records,
        })))
    }

    pub fn try_new(
        sources: &[SourceDeclarationKey],
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ParamFreeShapeSupportExportV1>,
    ) -> Result<Self, ParamFreeShapeSupportTableError> {
        validate_inputs(layouts, descriptors, foundation)?;
        sort_records(&mut records)?;
        for pair in records.windows(2) {
            if pair[0].source_nominal() == pair[1].source_nominal() {
                return Err(ParamFreeShapeSupportTableError::Duplicate(
                    pair[0].source_nominal(),
                ));
            }
        }
        let expected = Self::from_sources(sources, layouts, descriptors, foundation)?;
        let expected = expected.records();
        if records.len() != expected.len() {
            return Err(ParamFreeShapeSupportTableError::Coverage);
        }
        for (record, expected) in records.iter().zip(expected) {
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

fn validate_inputs(
    layouts: &CanonicalExactLayoutExportsV1,
    descriptors: &CanonicalExactDescriptorExportsV1,
    foundation: &OdrFreeLirFoundation,
) -> Result<(), ParamFreeShapeSupportTableError> {
    if layouts.provider() != foundation.producer()
        || descriptors.provider() != foundation.producer()
    {
        return Err(ParamFreeShapeSupportTableError::TableProvider);
    }
    if layouts.target() != descriptors.target() {
        return Err(ParamFreeShapeSupportTableError::TableTarget);
    }
    Ok(())
}

fn sort_records(
    records: &mut [ParamFreeShapeSupportExportV1],
) -> Result<(), ParamFreeShapeSupportTableError> {
    records.sort_unstable_by_key(ParamFreeShapeSupportExportV1::source_nominal);
    Ok(())
}
