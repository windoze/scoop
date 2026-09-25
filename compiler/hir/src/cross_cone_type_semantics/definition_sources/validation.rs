use super::*;
use crate::ExportDefinitionSourceV1;

pub(super) struct Validator<'a, A> {
    declared: &'a [ExportDefinitionSourceV1],
    seen: Vec<bool>,
    authority: &'a mut A,
}
impl<'a, A> Validator<'a, A> {
    pub(super) fn new<E>(
        declared: &'a CanonicalExportDefinitionSourcesV1,
        authority: &'a mut A,

        path: &WirePath,
    ) -> Result<Self, TypeDefinitionSourceClosureError<E>> {
        let sources = declared.sources();
        let mut seen = Vec::new();

        scoop_wire::allocation::try_reserve(&mut seen, sources.len(), path)?;
        seen.resize(sources.len(), false);
        Ok(Self {
            declared: sources,
            seen,
            authority,
        })
    }
    pub(super) fn finish<E>(self) -> Result<(), TypeDefinitionSourceClosureError<E>> {
        if let Some(index) = self.seen.iter().position(|seen| !seen) {
            return Err(TypeDefinitionSourceClosureError::Extra { index });
        }
        Ok(())
    }
}
impl<A: TypeDefinitionSourceSemanticAuthority<E>, E> SourceVisitor<E> for Validator<'_, A> {
    fn observe(
        &mut self,
        source: &ExportDefinitionSourceV1,
        source_use: TypeDefinitionSourceUseV1<'_>,

        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<E>> {
        let (mut left, mut right) = (0, self.declared.len());
        while left < right {
            let index = left + (right - left) / 2;
            let candidate = &self.declared[index];

            match candidate.cmp(source) {
                std::cmp::Ordering::Less => left = index + 1,
                std::cmp::Ordering::Greater => right = index,
                std::cmp::Ordering::Equal => {
                    self.authority
                        .validate_type_definition_source_use(source_use, source, path)
                        .map_err(|error| TypeDefinitionSourceClosureError::Source {
                            path: path.clone(),
                            error,
                        })?;
                    self.seen[index] = true;
                    return Ok(());
                }
            }
        }
        Err(TypeDefinitionSourceClosureError::Missing {
            path: path.clone(),
            insertion_index: left,
        })
    }
}
