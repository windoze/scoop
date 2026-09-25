use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentLayoutId, RepresentationRole};
use scoop_wire::WireError;

use super::*;
use crate::{ExternalStrongShapeSubjectV1, LirTargetProfile, OdrFreeLirFoundation};

mod wire;
pub use wire::DecodedCanonicalExactLayoutExportsV1;

#[cfg(test)]
mod tests;

/// Canonical physical records from one provider foundation. This table does
/// not replace source representation joins or selected dependency authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactLayoutExportsV1(Arc<LayoutTable>);

#[derive(Debug, Eq, PartialEq)]
struct LayoutTable {
    provider: ConeIdentity,
    target: LirTargetProfile,
    records: Vec<ExactLayoutExportV1>,
}

impl CanonicalExactLayoutExportsV1 {
    pub fn try_new(
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ExactLayoutExportV1>,
    ) -> Result<Self, ExactLayoutTableError> {
        records.sort_unstable_by_key(|record| record.identity().layout());
        for (index, record) in records.iter().enumerate() {
            let identity = record.identity();
            if index > 0 && records[index - 1].identity().layout() == identity.layout() {
                return Err(ExactLayoutTableError::Duplicate(identity.layout()));
            }
            if identity.target() != target {
                return Err(ExactLayoutTableError::Target(identity.layout()));
            }
            if identity.physical_definition().provider() != foundation.producer() {
                return Err(ExactLayoutTableError::Provider(identity.layout()));
            }
            let expected = ExactLayoutIdentityV1::from_foundation(
                target,
                identity.exact_record().clone(),
                identity.layout_key().representation(),
                foundation,
            )?;
            if &expected != identity {
                return Err(ExactLayoutTableError::Identity(identity.layout()));
            }

            if !foundation
                .scans()
                .iter()
                .any(|scan| scan.id() == record.scan())
            {
                return Err(ExactLayoutTableError::Scan(identity.layout()));
            }
            let scan = crate::StrongShapeDefinitionRefV1::from_foundation(
                ExternalStrongShapeSubjectV1::Scan(record.scan()),
                foundation,
            )?;
            if scan != record.scan_definition() {
                return Err(ExactLayoutTableError::Scan(identity.layout()));
            }
        }
        Ok(Self(Arc::new(LayoutTable {
            provider: foundation.producer(),
            target,
            records,
        })))
    }

    pub fn provider(&self) -> ConeIdentity {
        self.0.provider
    }
    pub fn target(&self) -> LirTargetProfile {
        self.0.target
    }
    pub fn records(&self) -> &[ExactLayoutExportV1] {
        &self.0.records
    }
    pub fn get(&self, layout: PersistentLayoutId) -> Option<&ExactLayoutExportV1> {
        self.0
            .records
            .binary_search_by_key(&layout, |record| record.identity().layout())
            .ok()
            .map(|index| &self.0.records[index])
    }

    pub fn find_exact_role(
        &self,
        exact: PersistentExactTypeId,
        role: RepresentationRole,
    ) -> Option<&ExactLayoutExportV1> {
        self.0.records.iter().find(|record| {
            record.identity().exact() == exact
                && record.identity().layout_key().representation() == role
        })
    }
}

#[derive(Debug)]
pub enum ExactLayoutTableError {
    CountOverflow,
    Count,
    Duplicate(PersistentLayoutId),
    Target(PersistentLayoutId),
    Provider(PersistentLayoutId),
    Identity(PersistentLayoutId),
    Scan(PersistentLayoutId),
    Binding(ExactLayoutIdentityError),
    Definition(crate::StrongShapeDefinitionError),
    Record {
        index: usize,
        source: ExactLayoutWireError,
    },
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactLayoutTableError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(ExactLayoutIdentityError, Binding);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(WireError, Resource);

impl std::fmt::Display for ExactLayoutTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid exact layout table: {self:?}")
    }
}
impl std::error::Error for ExactLayoutTableError {}
