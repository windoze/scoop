//! Validated output kind and executable-entry identity for one current Cone.

use std::fmt;

use scoop_identity::{
    DeclarationName, DefinitionOriginSubject, ExactCallableSignature, ExecutableEntryIdentity,
    ExecutableEntryIdentityError, SourceSignatureFingerprint,
};

use crate::{ExportHir, FunctionGenericity, FunctionId, FunctionKind, HirSourceFunctionIdentity};

/// Request-local Export HIR function proven to belong to the current Cone.
///
/// The raw arena id cannot be constructed through this type without also
/// validating its persistent declaration identity and executable-entry shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CurrentFunctionId(FunctionId);

impl CurrentFunctionId {
    pub const fn function(self) -> FunctionId {
        self.0
    }
}

/// Unique, fully validated local entry of an executable Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableEntry {
    identity: ExecutableEntryIdentity,
    local_function: CurrentFunctionId,
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

        if !matches!(function.genericity, FunctionGenericity::Plain) {
            return Err(LocalExecutableEntryError::Generic);
        }
        if function.is_suspend {
            return Err(LocalExecutableEntryError::Suspend);
        }
        if !matches!(function.kind, FunctionKind::User(_)) {
            return Err(LocalExecutableEntryError::NotScoopDefined);
        }

        let exact_type = |ty| {
            module
                .type_identities
                .get(ty)
                .and_then(|identity| identity.exact())
                .map(|identity| identity.id())
                .ok_or(LocalExecutableEntryError::MissingExactType)
        };
        let signature = ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            function
                .params
                .iter()
                .map(|parameter| exact_type(parameter.ty))
                .collect::<Result<_, _>>()?,
            exact_type(function.return_ty)?,
        );
        let identity = match &module.function_identities[function_id] {
            crate::HirFunctionIdentity::Source(identity) => {
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
                let declaration = record.id();
                let origin = module
                    .export_definition_origins
                    .get(DefinitionOriginSubject::Function(declaration))
                    .ok_or(LocalExecutableEntryError::MissingDefinitionOrigin)?;
                if origin.origin().source().cone() != module.cone {
                    return Err(LocalExecutableEntryError::DefinitionOriginConeMismatch);
                }

                ExecutableEntryIdentity::try_new(record, signature)
                    .map_err(LocalExecutableEntryError::Identity)?
            }
            crate::HirFunctionIdentity::TestEntry(record) => {
                if !matches!(record.key(), scoop_identity::GeneratedCallableKey::TestEntry { cone } if *cone == module.cone)
                {
                    return Err(LocalExecutableEntryError::NotCurrentCone);
                }
                ExecutableEntryIdentity::for_test(record, signature)
                    .map_err(LocalExecutableEntryError::Identity)?
            }
            _ => return Err(LocalExecutableEntryError::NotSourceFunction),
        };
        Ok(Self {
            identity,
            local_function: CurrentFunctionId(function_id),
        })
    }

    pub const fn declaration(&self) -> scoop_identity::ExecutableEntryDeclaration {
        self.identity.declaration()
    }

    pub const fn local_function(&self) -> CurrentFunctionId {
        self.local_function
    }

    pub const fn source_signature(&self) -> &ExactCallableSignature {
        self.identity.source_signature()
    }

    pub const fn source_signature_fingerprint(&self) -> SourceSignatureFingerprint {
        self.identity.source_signature_fingerprint()
    }

    pub const fn identity(&self) -> &ExecutableEntryIdentity {
        &self.identity
    }
}

/// Output contract after HIR has completed executable-entry selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConeOutputKind {
    Library,
    Executable {
        local_entry: Box<LocalExecutableEntry>,
    },
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
    NotScoopDefined,
    MissingExactType,
    MissingDefinitionOrigin,
    DefinitionOriginConeMismatch,
    Identity(ExecutableEntryIdentityError),
}

impl fmt::Display for LocalExecutableEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingFunction => "the executable entry does not belong to Export HIR",
            Self::NotTopLevel => "the executable entry is not a top-level function",
            Self::HasReceiver => "the executable entry has a receiver",
            Self::NotSourceFunction => {
                "the executable entry is neither a source function nor a generated test entry"
            }
            Self::NotCurrentCone => "the executable entry does not belong to the current Cone",
            Self::NotMain => "the executable entry is not named `main`",
            Self::Generic => "the executable entry is generic",
            Self::Suspend => "the executable entry is suspend",
            Self::NotScoopDefined => "the executable entry has no ordinary Scoop body",
            Self::MissingExactType => "an executable signature type has no exact identity",
            Self::MissingDefinitionOrigin => "the executable entry has no definition origin",
            Self::DefinitionOriginConeMismatch => {
                "the executable entry definition origin belongs to a different Cone"
            }
            Self::Identity(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for LocalExecutableEntryError {}
