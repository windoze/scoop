use crate::{
    CanonicalBinderUseListV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticValidationError, PersistentLexicalRootV1,
    ProtectedDefaultTemplateKeyV1,
};
use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};
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

    /// Returns the true provider's source receiver in its original binder
    /// frame, independently of the publishing override or candidate body.
    fn protected_default_provider_receiver(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<Option<SignatureTypeKey>, E>;

    /// Borrows the independently declared parameters and the exact parameter
    /// position denoted by this provider path, in the original binder frame.
    fn protected_default_provider_parameter(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<crate::DefaultTemplateProviderParameterV1<'_>, E>;

    /// Replays the actual override relation, its complete binder substitution
    /// (including unused arguments), and publishing-to-provider receiver use.
    fn validate_inherited_protected_default_provider(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        mapping: &CanonicalBinderUseListV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), E>;
}
pub(super) fn validate<A: ProtectedDefaultRootSemanticAuthority<E>, E>(
    key: ProtectedDefaultTemplateKeyV1,
    root: PersistentLexicalRootV1,
    path: &StructuralDefinitionPath,
    mapping: &CanonicalBinderUseListV1,
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
            .validate_inherited_protected_default_provider(key, root, path, mapping, meter)
            .map_err(Error::InheritedRelation)?;
    }
    Ok(provider)
}
