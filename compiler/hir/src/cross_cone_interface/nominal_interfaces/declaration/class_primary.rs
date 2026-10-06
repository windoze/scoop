use scoop_identity::{PersistentConstructorId, PersistentPropertyId};
use std::collections::BTreeSet;
use std::fmt;

pub(super) mod wire;
pub use wire::{ClassPrimaryConstructorResolutionError, DecodedClassPrimaryConstructorV1};

/// Source parameter order relates a primary constructor to logical properties.
/// Names, types and defaults remain in its ordinary callable interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassPrimaryConstructorV1 {
    constructor: PersistentConstructorId,
    properties: Vec<Option<PersistentPropertyId>>,
}

impl ClassPrimaryConstructorV1 {
    pub fn try_new(
        constructor: PersistentConstructorId,
        properties: Vec<Option<PersistentPropertyId>>,
    ) -> Result<Self, ClassPrimaryConstructorBuildError> {
        let mut seen = BTreeSet::new();
        for &property in properties.iter().flatten() {
            if !seen.insert(property) {
                return Err(ClassPrimaryConstructorBuildError::DuplicateProperty(
                    property,
                ));
            }
        }
        Ok(Self {
            constructor,
            properties,
        })
    }

    pub const fn constructor(&self) -> PersistentConstructorId {
        self.constructor
    }

    pub fn properties(&self) -> &[Option<PersistentPropertyId>] {
        &self.properties
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassPrimaryConstructorBuildError {
    DuplicateProperty(PersistentPropertyId),
}

impl fmt::Display for ClassPrimaryConstructorBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProperty(id) => write!(f, "primary constructor repeats property {id}"),
        }
    }
}

impl std::error::Error for ClassPrimaryConstructorBuildError {}
