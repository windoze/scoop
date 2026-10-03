use super::*;

pub(super) struct DefinitionSourceVisitor<'a, V, W> {
    pub visitor: &'a mut V,
    pub evaluations: &'a mut W,
}
impl<V, W, E> BodyWalkMode for DefinitionSourceVisitor<'_, V, W>
where
    V: FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1, &WirePath) -> Result<(), E>,
    W: FnMut(&scoop_identity::EvaluationOrigin, &WirePath) -> Result<(), E>,
    E: From<WireError>,
{
    type Error = E;
    fn resource(error: WireError) -> Self::Error {
        error.into()
    }
    fn validate_type(
        &mut self,
        _: &SignatureTypeKey,
        _: DefaultBodyProviderTypeSiteV1,
        _: &ExportDefinitionSourceV1,

        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn validate_local_function_signature(
        &mut self,
        _: &DefaultLocalFunctionV1,
        _: &ExportDefinitionSourceV1,

        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn visit_origin(
        &mut self,
        source: &ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,

        path: &WirePath,
    ) -> Result<(), Self::Error> {
        (self.visitor)(source, site, path)
    }
    fn visit_evaluation_origin(
        &mut self,
        source: &scoop_identity::EvaluationOrigin,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        (self.evaluations)(source, path)
    }
}
