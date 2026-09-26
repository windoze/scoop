//! Resolve the compiler role through the actual dependency's declarations.

use super::*;
use scoop_hir::{ExternalHirReferenceV1, HirDependencyCallReasonV1, HirDependencyCallSiteV1};

impl HirInterfaceValidationInput<'_> {
    pub(super) fn runtime_call(
        self,
        reference: &ExternalHirReferenceV1,
        site: &HirDependencyCallSiteV1,
        dependencies: &[ValidatedNominalProviderView<'_>],
    ) -> Result<(), CrossConeHirRuntimeCallError> {
        use CrossConeHirRuntimeCallError as Error;
        let HirDependencyCallReasonV1::CastFailure { .. } = site.reason() else {
            return Ok(());
        };

        let provider = dependencies
            .iter()
            .find(|provider| provider.identity == reference.origin())
            .ok_or(Error::Provider(reference.origin()))?;
        let roles = provider
            .core
            .compiler_protocols()
            .ok_or(Error::Provider(reference.origin()))?;
        let (_, owner) = site
            .runtime_constructor_source(
                reference.target(),
                provider.identity,
                provider.identities,
                provider.interface,
            )
            .map_err(|source| Error::Constructor(Box::new(source)))?;
        site.validate_runtime_constructor_role(reference.target(), owner, roles)
            .map_err(|source| Error::Constructor(Box::new(source)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirRuntimeCallError {
    Resource(WireError),
    Provider(ConeIdentity),
    Constructor(Box<scoop_hir::HirRuntimeConstructorError>),
}
impl From<WireError> for CrossConeHirRuntimeCallError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for CrossConeHirRuntimeCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared HIR runtime call: {self:?}")
    }
}
impl std::error::Error for CrossConeHirRuntimeCallError {}
