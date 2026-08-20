use super::*;
use crate::{LinkDataError, link_data::link_error};

impl DecodedParamFreeShapeSupportPlanSetV1 {
    pub(crate) fn link_sources(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Vec<SourceDeclarationKey>, LinkDataError> {
        self.closures
            .iter()
            .map(|closure| {
                let source = closure.roles.source_nominal().map_err(link_error)?;
                let key: std::sync::Arc<SourceDeclarationKey> =
                    identities.resolve_key(source).map_err(link_error)?;
                Ok(key.as_ref().clone())
            })
            .collect()
    }
}
