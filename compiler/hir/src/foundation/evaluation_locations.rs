//! Executable evaluation locations checked against the current foundation.

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner, ConeIdentity,
    EvaluationOrigin, GeneratedCallableKey, PersistentSourceContextId,
};

use super::{
    CanonicalHirFoundation, DefinitionSourceLocationValidationError, OdrFreeHirFoundation,
};

mod contexts;

impl OdrFreeHirFoundation {
    pub fn validate_executable_evaluation_origin(
        &self,
        current: ConeIdentity,
        root: CallableMaterialization,
        origin: &EvaluationOrigin,
    ) -> Result<(), ExecutableEvaluationValidationError> {
        self.as_canonical().validate_executable_evaluation_origin(
            current,
            root,
            origin,
            self.as_canonical(),
        )
    }
}

impl CanonicalHirFoundation {
    pub fn validate_evaluation_source_location(
        &self,
        provider: ConeIdentity,
        origin: &EvaluationOrigin,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        self.validate_source_location(provider, origin.source(), origin.span(), origin.context())
    }

    /// Resolve locations in this source provider and generated roots in the
    /// current materialization, which may instantiate an external template.
    pub fn validate_executable_evaluation_origin(
        &self,
        provider: ConeIdentity,
        root: CallableMaterialization,
        origin: &EvaluationOrigin,
        materializations: &CanonicalHirFoundation,
    ) -> Result<(), ExecutableEvaluationValidationError> {
        use ExecutableEvaluationValidationError as Error;
        self.validate_source_location(provider, origin.source(), origin.span(), origin.context())
            .map_err(Error::Location)?;
        if matches!(root.template(), CallableTemplateOwner::GenericFunction(_))
            && !matches!(
                root.context(),
                CallableMaterializationContext::Application(_)
            )
        {
            return Err(Error::Materialization(root));
        }
        if let CallableTemplateOwner::Generated(id) = root.template()
            && materializations.generated_callables.iter().any(|record| {
                record.id() == id
                    && matches!(
                        record.key(),
                        GeneratedCallableKey::DerivedEquality { .. }
                            | GeneratedCallableKey::TupleEncoding { .. }
                    )
            })
        {
            // Derived bodies are owned by an exact type, without a lexical source
            // body. Their locations can belong to an uninstantiated provider
            // template, while the exact helper is defined in the current Cone.
            return match root.context() {
                CallableMaterializationContext::NoSubstitution => Ok(()),
                _ => Err(Error::Materialization(root)),
            };
        }
        let context_error = || Error::Context {
            root,
            context: origin.context(),
        };
        let context = self
            .source_contexts
            .binary_search_by_key(&origin.context(), |record| record.id())
            .ok()
            .map(|index| self.source_contexts[index].key())
            .ok_or_else(context_error)?;
        // Generated lexical and initialization bodies use their canonical source anchor.
        let subject = contexts::source_subject(self, materializations, root.template(), context)
            .ok_or_else(context_error)?;

        let definition = self
            .definition_origin(subject)
            .ok_or(Error::MissingRootOrigin(root))?;
        if definition.origin().source() != origin.source() {
            return Err(Error::RootSource(root));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableEvaluationValidationError {
    Location(DefinitionSourceLocationValidationError),
    Materialization(CallableMaterialization),
    MissingRootOrigin(CallableMaterialization),
    RootSource(CallableMaterialization),
    Context {
        root: CallableMaterialization,
        context: PersistentSourceContextId,
    },
}

impl std::fmt::Display for ExecutableEvaluationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Location(error) => error.fmt(f),
            Self::Materialization(root) => {
                write!(f, "call root {root:?} has an invalid instantiation context")
            }
            Self::MissingRootOrigin(root) => {
                write!(f, "call root {root:?} has no source definition")
            }
            Self::RootSource(root) => {
                write!(f, "call evaluation is outside the source of root {root:?}")
            }
            Self::Context { root, context } => {
                write!(f, "call context {context} is not owned by root {root:?}")
            }
        }
    }
}

impl std::error::Error for ExecutableEvaluationValidationError {}
