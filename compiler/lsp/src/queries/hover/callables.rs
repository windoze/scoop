use super::*;

pub(super) fn description(
    analysis: &mut DeclarationAnalysis,
    reference: hir::EditingCallableReference,
    snapshot: &AnalysisSnapshot,
    cursor: &semantic::Cursor<'_>,
) -> Result<Option<Description>, ResponseError> {
    let Some(callable) = reference.target.callable() else {
        return constructors::description(analysis, reference.target, snapshot, cursor);
    };
    let mut origin = reference.definition;
    let label = match callable {
        hir::SemanticCallable::Current(function) => match analysis.signature(function) {
            EditingValue::Known(signature) => {
                let label = signature_label(analysis, &signature)?;
                origin = Some(signature.origin);
                Some(label)
            }
            EditingValue::Unavailable(_) => None,
        },
        hir::SemanticCallable::Imported(callable) => {
            imported::label(analysis, callable, snapshot, cursor.project)?
        }
        hir::SemanticCallable::CurrentGetter(_) | hir::SemanticCallable::CurrentSetter(_) => None,
    };
    Ok(label.map(|label| Description {
        label,
        documentation: origin.map(|origin| documentation::Query {
            symbol: documentation::callable(callable),
            origin,
        }),
    }))
}
