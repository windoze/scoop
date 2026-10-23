use super::*;

#[derive(Default)]
pub(super) struct Positions {
    files: HashMap<SourceIdentity, Vec<(Span, usize)>>,
}

impl Positions {
    pub(super) fn new(world: &World, edits: &[SourceEdit]) -> Self {
        let mut by_source = HashMap::<_, Vec<_>>::new();
        for edit in edits {
            by_source
                .entry(edit.source.source)
                .or_default()
                .push((edit.span, edit.replacement.len()));
        }
        for edits in by_source.values_mut() {
            edits.sort_by_key(|(span, _)| span.start);
        }
        Self {
            files: world
                .sources
                .iter()
                .filter_map(|(identity, source)| {
                    by_source
                        .get(&source.version.source)
                        .map(|edits| (identity.clone(), edits.clone()))
                })
                .collect(),
        }
    }

    pub(super) fn origin(&self, origin: &Origin) -> Result<Origin, RenameError> {
        let Some(edits) = self.files.get(&origin.source) else {
            return Ok(origin.clone());
        };
        let mapped = |offset| -> Option<u32> {
            let mut delta = 0i64;
            for &(span, length) in edits {
                if offset <= span.start {
                    break;
                }
                if offset < span.end {
                    return None;
                }
                delta += length as i64 - i64::from(span.end - span.start);
            }
            u32::try_from(i64::from(offset) + delta).ok()
        };
        let span = mapped(origin.span.start)
            .zip(mapped(origin.span.end))
            .map(|(start, end)| Span::new(start, end))
            .ok_or_else(|| RenameError::Conflict(origin.clone()))?;
        Ok(Origin {
            span,
            source: origin.source.clone(),
        })
    }
}
