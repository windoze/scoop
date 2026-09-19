use crate::{
    DefaultTemplateProviderShapeV1, DefaultTemplateRootSemanticValidationError,
    PersistentLexicalRootV1, ProtectedDefaultTemplateKeyV1,
};
use scoop_identity::StructuralDefinitionPath;
use scoop_wire::BudgetMeter;

/// Supplies the independently established provider and override relation for
/// the protected default key space.
pub trait ProtectedDefaultRootSemanticAuthority<E> {
    fn protected_default_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultTemplateProviderShapeV1, E>;

    fn validate_inherited_protected_default_provider(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<(), E>;
}
pub(super) fn validate<A: ProtectedDefaultRootSemanticAuthority<E>, E>(
    key: ProtectedDefaultTemplateKeyV1,
    root: PersistentLexicalRootV1,
    path: &StructuralDefinitionPath,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<DefaultTemplateProviderShapeV1, DefaultTemplateRootSemanticValidationError<E>> {
    use DefaultTemplateRootSemanticValidationError as Error;
    let last = path.segments().last().ok_or(Error::EmptyDefinitionPath)?;
    if last.site_role() != scoop_identity::StructuralDefinitionSiteRole::DefaultValue {
        return Err(Error::InvalidDefinitionPathRole {
            actual: last.site_role(),
        });
    }
    let provider = authority
        .protected_default_provider_shape(root, path, meter)
        .map_err(Error::Provider)?;
    if root.declaration() != key.owner() {
        authority
            .validate_inherited_protected_default_provider(key, root, path, meter)
            .map_err(Error::InheritedRelation)?;
    }
    Ok(provider)
}
