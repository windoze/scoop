use scoop_identity::{
    ConeIdentity, ObjectDefinitionPlanId, PersistentSymbolError, PersistentSymbolRequest,
};
use scoop_lir::{ExternalStrongShapeSubjectV1, ShapeLinkError, StrongShapeDefinitionError};
use scoop_wire::{HashError, WireError};

use crate::LinkDefinitionOwnerV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossConeLayoutTerminalImportContextV1 {
    pub consumer: ConeIdentity,
    pub provider: ConeIdentity,
    pub subject: ExternalStrongShapeSubjectV1,
}

impl CrossConeLayoutTerminalImportContextV1 {
    pub const fn new(
        consumer: ConeIdentity,
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Self {
        Self {
            consumer,
            provider,
            subject,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLayoutTerminalValidationError {
    NoArtifacts,
    ArtifactIdentityMismatch {
        foundation: ConeIdentity,
        section: ConeIdentity,
        defined_symbols: ConeIdentity,
    },
    DuplicateProvider {
        provider: ConeIdentity,
        first: usize,
        second: usize,
    },
    MissingProvider {
        consumer: ConeIdentity,
        provider: ConeIdentity,
    },
    TargetMismatch {
        consumer: ConeIdentity,
        provider: ConeIdentity,
    },
    ImportCountOverflow,
    ProviderSection(ShapeLinkError),
    ImportReplay {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        source: Box<ShapeLinkError>,
    },
    SymbolMismatch {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        expected: PersistentSymbolRequest,
        actual: PersistentSymbolRequest,
    },
    DefinitionMismatch {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    ContractMismatch(Box<CrossConeLayoutTerminalImportContextV1>),
    DerivedDefinitionMismatch(Box<CrossConeLayoutTerminalImportContextV1>),
    InvalidStrongOwner {
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
    MissingStrongDefinition {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        definition: ObjectDefinitionPlanId,
        symbol: PersistentSymbolRequest,
    },
    StrongOwnerMismatch {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        expected: LinkDefinitionOwnerV1,
        actual: LinkDefinitionOwnerV1,
    },
    ForeignStrongDefinition {
        context: Box<CrossConeLayoutTerminalImportContextV1>,
        actual_owner: ConeIdentity,
        symbol: PersistentSymbolRequest,
    },
    Definition(StrongShapeDefinitionError),
    Hash(HashError),
    Symbol(PersistentSymbolError),
    Resource(WireError),
}

impl From<StrongShapeDefinitionError> for CrossConeLayoutTerminalValidationError {
    fn from(source: StrongShapeDefinitionError) -> Self {
        Self::Definition(source)
    }
}

impl From<HashError> for CrossConeLayoutTerminalValidationError {
    fn from(source: HashError) -> Self {
        Self::Hash(source)
    }
}

impl From<PersistentSymbolError> for CrossConeLayoutTerminalValidationError {
    fn from(source: PersistentSymbolError) -> Self {
        Self::Symbol(source)
    }
}

impl From<WireError> for CrossConeLayoutTerminalValidationError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}

impl std::fmt::Display for CrossConeLayoutTerminalValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone layout terminal closure: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLayoutTerminalValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProviderSection(source) => Some(source),
            Self::ImportReplay { source, .. } => Some(source.as_ref()),
            Self::Definition(source) => Some(source),
            Self::Hash(source) => Some(source),
            Self::Symbol(source) => Some(source),
            Self::Resource(source) => Some(source),
            _ => None,
        }
    }
}
