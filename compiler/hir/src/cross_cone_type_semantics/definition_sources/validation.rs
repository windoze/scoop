use super::*;
use crate::ExportDefinitionSourceV1;
use scoop_wire::{WireError, WireErrorKind, encoded_length};

pub(super) struct Validator<'a, A> {
    declared: &'a [ExportDefinitionSourceV1],
    seen: Vec<bool>,
    authority: &'a mut A,
}
impl<'a, A> Validator<'a, A> {
    pub(super) fn new<E>(
        declared: &'a CanonicalExportDefinitionSourcesV1,
        authority: &'a mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, TypeDefinitionSourceClosureError<E>> {
        let sources = declared.sources();
        let mut seen = Vec::new();
        meter.check_table_entries(sources.len() as u64, path)?;
        meter.charge_work(sources.len() as u64, path)?;
        meter.try_reserve_collection_slots(&mut seen, sources.len(), path)?;
        seen.resize(sources.len(), false);
        Ok(Self {
            declared: sources,
            seen,
            authority,
        })
    }
    pub(super) fn finish<E>(
        self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<E>> {
        meter.charge_work(self.seen.len() as u64, path)?;
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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<E>> {
        meter.charge_nodes(1, path)?;
        meter.check_semantic_leaf(
            source.origin().source().logical_path().as_str().len() as u64,
            path,
        )?;
        let bytes = source_bytes(source, path)?;
        meter.charge_work(bytes, path)?;
        let (mut left, mut right) = (0, self.declared.len());
        while left < right {
            let index = left + (right - left) / 2;
            let candidate = &self.declared[index];
            meter.charge_work(bytes, path)?;
            meter.charge_work(source_bytes(candidate, path)?, path)?;
            match candidate.cmp(source) {
                std::cmp::Ordering::Less => left = index + 1,
                std::cmp::Ordering::Greater => right = index,
                std::cmp::Ordering::Equal => {
                    self.authority
                        .validate_type_definition_source_use(source_use, source, meter, path)
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
pub(super) fn source_bytes(
    source: &ExportDefinitionSourceV1,
    path: &WirePath,
) -> Result<u64, WireError> {
    encoded_length(source)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))
}
