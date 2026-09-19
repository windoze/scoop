//! Metered resolution for type-section consumers; the original wire is unchanged.
use super::*;
use scoop_wire::{BudgetMeter, WireErrorKind, WirePath, encoded_length};

impl DecodedCanonicalExportDefinitionSourcesV1 {
    pub fn resolve_metered<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalExportDefinitionSourcesV1, MeteredDefinitionSourcesResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        use MeteredDefinitionSourcesResolutionError as Error;
        let mut sources: Vec<ExportDefinitionSourceV1> = Vec::new();
        meter
            .check_table_entries(self.sources.len() as u64, path)
            .map_err(Error::Resource)?;
        meter
            .try_reserve_collection_slots(&mut sources, self.sources.len(), path)
            .map_err(Error::Resource)?;
        let mut previous_bytes = 0;
        for (index, decoded) in self.sources.into_iter().enumerate() {
            let at = path.clone().index(index as u64);
            meter
                .check_semantic_depth(3, &at)
                .map_err(Error::Resource)?;
            meter
                .check_semantic_leaf(decoded.origin.logical_path_byte_len() as u64, &at)
                .map_err(Error::Resource)?;
            meter.charge_nodes(1, &at).map_err(Error::Resource)?;
            meter.charge_edges(2, &at).map_err(Error::Resource)?;
            let bytes = encoded_length(&decoded).map_err(|_| {
                Error::Resource(WireError::new(
                    WireErrorKind::IntegerOutOfRange,
                    at.clone(),
                    None,
                ))
            })?;
            meter
                .charge_owned_bytes(bytes, &at)
                .map_err(Error::Resource)?;
            meter.charge_work(bytes, &at).map_err(Error::Resource)?;
            let source = decoded.resolve(resolver).map_err(|error| {
                Error::Semantic(ExportDefinitionSourceSetValidationError::Source { index, error })
            })?;
            if let Some(previous) = sources.last() {
                meter
                    .charge_work(previous_bytes, &at)
                    .map_err(Error::Resource)?;
                meter.charge_work(bytes, &at).map_err(Error::Resource)?;
                match previous.cmp(&source) {
                    std::cmp::Ordering::Equal => {
                        return Err(Error::Semantic(
                            ExportDefinitionSourceSetValidationError::Duplicate { index },
                        ));
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(Error::Semantic(
                            ExportDefinitionSourceSetValidationError::NonCanonicalOrder { index },
                        ));
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            previous_bytes = bytes;
            sources.push(source);
        }
        Ok(CanonicalExportDefinitionSourcesV1 { sources })
    }
}

#[derive(Debug)]
pub enum MeteredDefinitionSourcesResolutionError<E> {
    Resource(WireError),
    Semantic(ExportDefinitionSourceSetValidationError<E>),
}
impl<E: fmt::Display> fmt::Display for MeteredDefinitionSourcesResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Semantic(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for MeteredDefinitionSourcesResolutionError<E>
{
}

#[cfg(test)]
mod tests;
