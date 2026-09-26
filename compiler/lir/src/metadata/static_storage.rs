//! Value layouts used by storage, with their actual definition owner.

use std::sync::Arc;

use scoop_identity::{ConeIdentity, LayoutKey, PersistentLayoutId, PersistentScanId};

use super::LayoutIdentity;
use crate::ExactValueLayoutV1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum StaticStorageLayout {
    Local(Box<LayoutIdentity>),
    External(Arc<ExactValueLayoutV1>),
}

impl From<LayoutIdentity> for StaticStorageLayout {
    fn from(value: LayoutIdentity) -> Self {
        Self::Local(Box::new(value))
    }
}

impl StaticStorageLayout {
    pub(crate) fn matches_value(
        &self,
        byte_size: u64,
        alignment: u64,
        scan: &crate::RefScan,
    ) -> bool {
        match self {
            Self::Local(_) => true,
            Self::External(value) => {
                let storage = value.value().storage();
                storage.byte_size() == byte_size
                    && storage.alignment().get() == alignment
                    && storage
                        .nonzero()
                        .map_or(&crate::RefScan::None, |v| v.scan().as_ref_scan())
                        == scan
            }
        }
    }

    pub fn local(&self) -> Option<&LayoutIdentity> {
        match self {
            Self::Local(identity) => Some(identity),
            Self::External(_) => None,
        }
    }

    pub fn provider(&self, producer: ConeIdentity) -> ConeIdentity {
        match self {
            Self::Local(_) => producer,
            Self::External(value) => value.identity().physical_definition().provider(),
        }
    }

    pub fn layout(&self) -> PersistentLayoutId {
        match self {
            Self::Local(identity) => identity.layout_record().id(),
            Self::External(value) => value.identity().layout(),
        }
    }

    pub fn layout_key(&self) -> &LayoutKey {
        match self {
            Self::Local(identity) => identity.layout_record().key(),
            Self::External(value) => value.identity().layout_key(),
        }
    }

    pub fn scan(&self) -> PersistentScanId {
        match self {
            Self::Local(identity) => identity.scan_record().id(),
            Self::External(value) => value.scan(),
        }
    }
}
