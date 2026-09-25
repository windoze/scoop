//! Executable evaluation locations checked against the current foundation.

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner, ConeIdentity,
    EvaluationOrigin, PersistentSourceContextId,
};

use super::{DefinitionSourceLocationValidationError, OdrFreeHirFoundation};

mod contexts;

impl OdrFreeHirFoundation {
    pub fn validate_executable_evaluation_origin(
        &self,
        current: ConeIdentity,
        root: CallableMaterialization,
        origin: &EvaluationOrigin,
    ) -> Result<(), ExecutableEvaluationValidationError> {
        use ExecutableEvaluationValidationError as Error;
        self.validate_source_location(current, origin.source(), origin.span(), origin.context())
            .map_err(Error::Location)?;
        if root.context() != CallableMaterializationContext::NoSubstitution
            || matches!(
                root.template(),
                CallableTemplateOwner::GenericFunction(_)
                    | CallableTemplateOwner::VariantConstructor(_)
            )
        {
            return Err(Error::Materialization(root));
        }
        let context_error = || Error::Context {
            root,
            context: origin.context(),
        };
        let context = self
            .source_context_key(origin.context())
            .ok_or_else(context_error)?;
        // Generated lexical and initialization bodies use their canonical source anchor.
        let subject = contexts::source_subject(self.as_canonical(), root.template(), context)
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
                write!(f, "call root {root:?} is not an ODR-free executable")
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
