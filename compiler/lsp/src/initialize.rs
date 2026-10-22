use lsp_types::{
    InitializeParams, InitializeResult, OneOf, PositionEncodingKind, SaveOptions,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions,
};
use scoop_source::PositionEncoding;

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    pub encoding: PositionEncoding,
    pub hierarchical_symbols: bool,
    pub line_folding_only: bool,
    pub folding_limit: Option<u32>,
    pub code_actions: bool,
    pub code_action_resolve: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            encoding: PositionEncoding::Utf16,
            hierarchical_symbols: false,
            line_folding_only: false,
            folding_limit: None,
            code_actions: false,
            code_action_resolve: false,
        }
    }
}

pub(crate) fn initialize(params: InitializeParams) -> (Settings, InitializeResult) {
    let encoding = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
        .and_then(|encodings| {
            encodings
                .iter()
                .find_map(|encoding| match encoding.as_str() {
                    "utf-8" => Some(PositionEncoding::Utf8),
                    "utf-16" => Some(PositionEncoding::Utf16),
                    "utf-32" => Some(PositionEncoding::Utf32),
                    _ => None,
                })
        })
        .unwrap_or(PositionEncoding::Utf16);
    let documents = params.capabilities.text_document.as_ref();
    let folding = documents.and_then(|documents| documents.folding_range.as_ref());
    let actions = documents.and_then(|documents| documents.code_action.as_ref());
    let code_actions = actions
        .and_then(|actions| actions.code_action_literal_support.as_ref())
        .is_some()
        && params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_edit.as_ref())
            .and_then(|edit| edit.document_changes)
            .unwrap_or(false);
    let code_action_resolve = code_actions
        && actions.is_some_and(|actions| {
            actions.data_support == Some(true)
                && actions.resolve_support.as_ref().is_some_and(|support| {
                    support.properties.iter().any(|property| property == "edit")
                })
        });
    let settings = Settings {
        encoding,
        hierarchical_symbols: documents
            .and_then(|documents| documents.document_symbol.as_ref())
            .and_then(|symbols| symbols.hierarchical_document_symbol_support)
            .unwrap_or(false),
        line_folding_only: folding
            .and_then(|folding| folding.line_folding_only)
            .unwrap_or(false),
        folding_limit: folding.and_then(|folding| folding.range_limit),
        code_actions,
        code_action_resolve,
    };
    let result = InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(match encoding {
                PositionEncoding::Utf8 => PositionEncodingKind::UTF8,
                PositionEncoding::Utf16 => PositionEncodingKind::UTF16,
                PositionEncoding::Utf32 => PositionEncodingKind::UTF32,
            }),
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                        include_text: Some(false),
                    })),
                    ..TextDocumentSyncOptions::default()
                },
            )),
            document_symbol_provider: Some(OneOf::Left(true)),
            hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            code_action_provider: code_actions.then(|| {
                lsp_types::CodeActionProviderCapability::Options(lsp_types::CodeActionOptions {
                    code_action_kinds: Some(vec![lsp_types::CodeActionKind::QUICKFIX]),
                    resolve_provider: Some(code_action_resolve),
                    ..Default::default()
                })
            }),
            completion_provider: Some(lsp_types::CompletionOptions {
                trigger_characters: Some(vec![".".into()]),
                ..Default::default()
            }),
            signature_help_provider: Some(lsp_types::SignatureHelpOptions {
                trigger_characters: Some(vec!["(".into(), ",".into()]),
                retrigger_characters: Some(vec![")".into()]),
                ..Default::default()
            }),
            folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
            selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(
                true,
            )),
            workspace: Some(lsp_types::WorkspaceServerCapabilities {
                workspace_folders: Some(lsp_types::WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    change_notifications: Some(OneOf::Left(true)),
                }),
                ..Default::default()
            }),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "scoop-lsp".to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    };
    (settings, result)
}
