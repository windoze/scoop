use super::*;

pub(super) fn validate<E>(
    local: &Exports<'_>,
    dependencies: &[&CheckedCrossConeTypeSemanticsSectionV1<'_>],
    origin: TypeSectionCommittedRootOriginV1,
) -> Result<(), TypeSelectionValidationError<E>> {
    use TypeSelectionValidationError as Error;

    match origin {
        TypeSectionCommittedRootOriginV1::Source => Ok(()),
        TypeSectionCommittedRootOriginV1::LocalSemanticSupport { parent } => {
            if local.facts.get(parent).is_some() {
                Ok(())
            } else {
                Err(Error::LocalSupportOrigin)
            }
        }
        TypeSectionCommittedRootOriginV1::ProtectedDefault { provider, key } => {
            let section = if provider == local.provider {
                local
            } else {
                &terminal(dependencies, provider)?.exports
            };
            match section.defaults.get(key) {
                Some(CheckedProtectedDefaultTemplateV1::ParamFree(_)) => Ok(()),
                Some(CheckedProtectedDefaultTemplateV1::GenericSourceMetadata(_)) => {
                    Err(Error::GenericDefault)
                }
                None => Err(Error::DefaultOrigin),
            }
        }
        TypeSectionCommittedRootOriginV1::PublicDefault { provider, key } => {
            let section = if provider == local.provider {
                local
            } else {
                &terminal(dependencies, provider)?.exports
            };
            let template = section
                .public
                .section()
                .default_templates()
                .get(key)
                .ok_or(Error::DefaultOrigin)?;
            if template.type_parameters().is_empty() {
                Ok(())
            } else {
                Err(Error::GenericDefault)
            }
        }
    }
}
