//! Identity-only observations share the complete body traversal.
use super::*;
use crate::DefaultNestedCallableIdentityV1 as Identity;
use scoop_identity::StructuralDefinitionPath;

pub(in super::super) fn visit_nested_identities<V, E>(
    body: &ExportDefaultBodyV1,
    visitor: &mut V,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), E>
where
    V: FnMut(
        Identity,
        &StructuralDefinitionPath,
        &ExportDefinitionSourceV1,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), E>,
    E: From<WireError>,
{
    Validator {
        mode: Visitor(visitor),
        meter,
        path,
    }
    .run(body)
}

pub(super) fn descriptor(
    node: BodyNode<'_>,
) -> Option<(
    Identity,
    &StructuralDefinitionPath,
    &ExportDefinitionSourceV1,
)> {
    match node {
        BodyNode::LocalFunction {
            function,
            definition_origin,
        } => Some((
            Identity::LocalFunction(function.declaration()),
            function.definition_path(),
            definition_origin,
        )),
        BodyNode::Lambda {
            lambda,
            definition_origin,
        } => Some((
            Identity::Lambda(lambda.body()),
            lambda.definition_path(),
            definition_origin,
        )),
        BodyNode::AnonymousFunction {
            function,
            definition_origin,
        } => Some((
            Identity::AnonymousFunction(function.body()),
            function.definition_path(),
            definition_origin,
        )),
        BodyNode::CallableReference {
            reference,
            definition_origin,
        } => Some((
            Identity::CallableReference(reference.invoke()),
            reference.definition_path(),
            definition_origin,
        )),
        _ => None,
    }
}

struct Visitor<'a, V>(&'a mut V);
impl<V, E> BodyWalkMode for Visitor<'_, V>
where
    V: FnMut(
        Identity,
        &StructuralDefinitionPath,
        &ExportDefinitionSourceV1,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), E>,
    E: From<WireError>,
{
    type Error = E;
    fn resource(error: WireError) -> E {
        error.into()
    }
    fn visit_nested_identity(
        &mut self,
        identity: Identity,
        definition_path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E> {
        (self.0)(identity, definition_path, origin, meter, path)
    }
    // Types, binders and origins are consumed by their separate semantic passes.
    fn validate_type(
        &mut self,
        _: &SignatureTypeKey,
        _: DefaultBodyProviderTypeSiteV1,
        _: &ExportDefinitionSourceV1,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), E> {
        Ok(())
    }
    fn validate_binder(
        &mut self,
        _: u32,
        _: u32,
        _: DefaultBodyProviderTypeSiteV1,
        _: &ExportDefinitionSourceV1,
    ) -> Result<(), E> {
        Ok(())
    }
    fn visit_origin(
        &mut self,
        _: &ExportDefinitionSourceV1,
        _: DefaultBodyOriginSiteV1,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), E> {
        Ok(())
    }
}
