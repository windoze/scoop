use crate::*;
use scoop_identity::LocalValueSelector;

pub(super) struct LocalIndex<'a> {
    locals: &'a CanonicalTemplateLocalTableV1,
}
impl<'a> LocalIndex<'a> {
    pub(super) fn new(locals: &'a CanonicalTemplateLocalTableV1) -> Self {
        Self { locals }
    }
}
impl TemplateLocalIndexResolver for LocalIndex<'_> {
    type Error = DefaultSourceLocalIndexError;
    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        use DefaultSourceLocalIndexError as Error;
        self.locals
            .index_of(selector)
            .ok_or_else(|| Error::MissingSelector(selector.clone()))
    }
}
