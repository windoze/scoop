//! Validated output kind and executable-entry identity for one current Cone.

use std::fmt;

use scoop_identity::{
    DeclarationName, DefinitionOriginSubject, ExactOrdinaryNoArgUnitSignature,
    PersistentFunctionId, SourceSignatureFingerprint,
};
use scoop_wire::HashError;

use crate::{ExportHir, FunctionGenericity, FunctionId, FunctionKind, HirSourceFunctionIdentity};

/// Request-local Export HIR function proven to belong to the current Cone.
///
/// The raw arena id cannot be constructed through this type without also
/// validating its persistent source identity and executable-entry shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CurrentFunctionId(FunctionId);

impl CurrentFunctionId {
    pub const fn function(self) -> FunctionId {
        self.0
    }
}

/// Unique, fully validated local source entry of an executable Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableEntry {
    declaration: PersistentFunctionId,
    local_function: CurrentFunctionId,
    source_signature: ExactOrdinaryNoArgUnitSignature,
    source_signature_fingerprint: SourceSignatureFingerprint,
}

impl LocalExecutableEntry {
    pub fn try_new(
        module: &ExportHir,
        function_id: FunctionId,
    ) -> Result<Self, LocalExecutableEntryError> {
        let function = module
            .functions
            .iter()
            .find_map(|(id, function)| (id == function_id).then_some(function))
            .ok_or(LocalExecutableEntryError::MissingFunction)?;
        if !module.top_level.contains(&function_id) {
            return Err(LocalExecutableEntryError::NotTopLevel);
        }
        if function.method.is_some() {
            return Err(LocalExecutableEntryError::HasReceiver);
        }

        let identity = module.function_identities[function_id]
            .source_identity()
            .ok_or(LocalExecutableEntryError::NotSourceFunction)?;
        if identity.declaration().origin() != module.cone {
            return Err(LocalExecutableEntryError::NotCurrentCone);
        }
        if !identity.declaration().owners().owners().is_empty() {
            return Err(LocalExecutableEntryError::NotTopLevel);
        }
        if !matches!(
            identity.declaration().name(),
            DeclarationName::Named(name) if name.as_str() == "main"
        ) || function.name != "main"
        {
            return Err(LocalExecutableEntryError::NotMain);
        }
        let HirSourceFunctionIdentity::Plain(record) = identity else {
            return Err(LocalExecutableEntryError::Generic);
        };
        if !matches!(function.genericity, FunctionGenericity::Plain) {
            return Err(LocalExecutableEntryError::Generic);
        }
        if function.is_suspend {
            return Err(LocalExecutableEntryError::Suspend);
        }
        if !function.params.is_empty() {
            return Err(LocalExecutableEntryError::HasParameters);
        }
        if function.return_ty != module.unit {
            return Err(LocalExecutableEntryError::NotUnitResult);
        }
        if !matches!(function.kind, FunctionKind::User(_)) {
            return Err(LocalExecutableEntryError::NotScoopDefined);
        }

        let unit = module
            .type_identities
            .get(module.unit)
            .and_then(|identity| identity.exact())
            .ok_or(LocalExecutableEntryError::UnitHasNoExactIdentity)?
            .id();
        let declaration = record.id();
        let origin = module
            .export_definition_origins
            .get(DefinitionOriginSubject::Function(declaration))
            .ok_or(LocalExecutableEntryError::MissingDefinitionOrigin)?;
        if origin.origin().source().cone() != module.cone {
            return Err(LocalExecutableEntryError::DefinitionOriginConeMismatch);
        }

        let source_signature = ExactOrdinaryNoArgUnitSignature::new(unit);
        let source_signature_fingerprint =
            SourceSignatureFingerprint::from_signature(&source_signature)
                .map_err(LocalExecutableEntryError::Fingerprint)?;
        Ok(Self {
            declaration,
            local_function: CurrentFunctionId(function_id),
            source_signature,
            source_signature_fingerprint,
        })
    }

    pub const fn declaration(&self) -> PersistentFunctionId {
        self.declaration
    }

    pub const fn local_function(&self) -> CurrentFunctionId {
        self.local_function
    }

    pub const fn source_signature(&self) -> &ExactOrdinaryNoArgUnitSignature {
        &self.source_signature
    }

    pub const fn source_signature_fingerprint(&self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }
}

/// Output contract after HIR has completed executable-entry selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConeOutputKind {
    Library,
    Executable { local_entry: LocalExecutableEntry },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalExecutableEntryError {
    MissingFunction,
    NotTopLevel,
    HasReceiver,
    NotSourceFunction,
    NotCurrentCone,
    NotMain,
    Generic,
    Suspend,
    HasParameters,
    NotUnitResult,
    NotScoopDefined,
    UnitHasNoExactIdentity,
    MissingDefinitionOrigin,
    DefinitionOriginConeMismatch,
    Fingerprint(HashError),
}

impl fmt::Display for LocalExecutableEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingFunction => "the executable entry does not belong to Export HIR",
            Self::NotTopLevel => "the executable entry is not a top-level source function",
            Self::HasReceiver => "the executable entry has a receiver",
            Self::NotSourceFunction => "the executable entry is not a source function",
            Self::NotCurrentCone => "the executable entry does not belong to the current Cone",
            Self::NotMain => "the executable entry is not named `main`",
            Self::Generic => "the executable entry is generic",
            Self::Suspend => "the executable entry is suspend",
            Self::HasParameters => "the executable entry has parameters",
            Self::NotUnitResult => "the executable entry does not return `Unit`",
            Self::NotScoopDefined => "the executable entry has no ordinary Scoop body",
            Self::UnitHasNoExactIdentity => "the HIR `Unit` type has no exact identity",
            Self::MissingDefinitionOrigin => "the executable entry has no definition origin",
            Self::DefinitionOriginConeMismatch => {
                "the executable entry definition origin belongs to a different Cone"
            }
            Self::Fingerprint(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for LocalExecutableEntryError {}
