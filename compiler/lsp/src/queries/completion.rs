use super::{json, semantic};
use crate::{convert, initialize::Settings};
use lsp_server::ResponseError;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionTextEdit, Position, TextEdit,
};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{EditingMemberTarget, EditingValue};
use serde_json::Value;

pub(crate) fn completion(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    position: Position,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let Some(cursor) = semantic::position_cursor(snapshot, source, position, settings)? else {
        return json(None::<CompletionList>);
    };
    let Some(analysis) = semantic::declarations(semantics, snapshot, source)? else {
        return json(None::<CompletionList>);
    };
    for function in semantic::functions(analysis, &cursor) {
        let receivers = analysis
            .receivers(function.function, || {
                cancellation.check().is_err() || snapshot.check_cancelled().is_err()
            })
            .map_err(|_| semantic::cancelled())?;
        let Some(receiver) = receivers.iter().find(|receiver| {
            receiver.dot.end <= cursor.span.start && cursor.span.end <= receiver.selector.end
        }) else {
            continue;
        };
        let EditingValue::Known(members) = &receiver.members else {
            return json(None::<CompletionList>);
        };
        let range = convert::range(&cursor.source.text, cursor.span, settings.encoding)?;
        let items = members
            .candidates
            .iter()
            .map(|member| CompletionItem {
                label: member.name.clone(),
                kind: Some(match member.target {
                    EditingMemberTarget::Callable(_) => CompletionItemKind::METHOD,
                    EditingMemberTarget::Property(_) | EditingMemberTarget::ImportedProperty(_) => {
                        CompletionItemKind::PROPERTY
                    }
                }),
                detail: member.is_extension.then(|| "extension".into()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: scoop_analysis::identifier_spelling(&member.name).into_owned(),
                })),
                ..CompletionItem::default()
            })
            .collect();
        return json(CompletionList {
            is_incomplete: !members.unavailable.is_empty(),
            items,
        });
    }
    json(None::<CompletionList>)
}
