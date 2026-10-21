use lsp_server::ResponseError;
use lsp_types::{DocumentSymbol, Location, SymbolInformation, SymbolKind, Uri};
use scoop_source::SourceText;
use scoop_syntax::{DeclarationSyntax, SyntaxKind};
use serde_json::Value;

use crate::{convert, initialize::Settings};

#[allow(deprecated)]
pub(super) fn document_symbols(
    declarations: &[DeclarationSyntax],
    text: &SourceText,
    uri: &Uri,
    settings: Settings,
) -> Result<Value, ResponseError> {
    let mut children: Vec<Vec<DocumentSymbol>> = vec![Vec::new(); declarations.len()];
    let mut roots = Vec::new();
    for (index, declaration) in declarations.iter().enumerate().rev() {
        let Some(name) = &declaration.name else {
            continue;
        };
        let parent_kind = declaration.parent.map(|parent| declarations[parent.0].kind);
        let Some(kind) = symbol_kind(declaration.kind, parent_kind) else {
            continue;
        };
        let mut nested = std::mem::take(&mut children[index]);
        nested.reverse();
        let symbol = DocumentSymbol {
            name: name.text.clone(),
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range: convert::range(text, declaration.span, settings.encoding)?,
            selection_range: convert::range(text, name.span, settings.encoding)?,
            children: (!nested.is_empty()).then_some(nested),
        };
        if let Some(parent) = declaration.parent {
            children[parent.0].push(symbol);
        } else {
            roots.push(symbol);
        }
    }
    roots.reverse();
    if settings.hierarchical_symbols {
        return super::json(roots);
    }
    let mut flattened = Vec::new();
    flatten(roots, None, uri, &mut flattened);
    super::json(flattened)
}

#[allow(deprecated)]
fn flatten(
    symbols: Vec<DocumentSymbol>,
    container: Option<&str>,
    uri: &Uri,
    output: &mut Vec<SymbolInformation>,
) {
    for symbol in symbols {
        output.push(SymbolInformation {
            name: symbol.name.clone(),
            kind: symbol.kind,
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri.clone(),
                range: symbol.selection_range,
            },
            container_name: container.map(str::to_owned),
        });
        if let Some(children) = symbol.children {
            flatten(children, Some(&symbol.name), uri, output);
        }
    }
}

fn symbol_kind(kind: SyntaxKind, parent: Option<SyntaxKind>) -> Option<SymbolKind> {
    Some(match kind {
        SyntaxKind::FunctionDecl
            if matches!(
                parent,
                Some(
                    SyntaxKind::ClassDecl
                        | SyntaxKind::StructDecl
                        | SyntaxKind::EnumDecl
                        | SyntaxKind::InterfaceDecl
                        | SyntaxKind::ObjectDecl
                        | SyntaxKind::CompanionDecl
                )
            ) =>
        {
            SymbolKind::METHOD
        }
        SyntaxKind::FunctionDecl => SymbolKind::FUNCTION,
        SyntaxKind::PropertyDecl => SymbolKind::PROPERTY,
        SyntaxKind::FieldDecl => SymbolKind::FIELD,
        SyntaxKind::StructDecl => SymbolKind::STRUCT,
        SyntaxKind::EnumDecl => SymbolKind::ENUM,
        SyntaxKind::Variant => SymbolKind::ENUM_MEMBER,
        SyntaxKind::InterfaceDecl => SymbolKind::INTERFACE,
        SyntaxKind::ClassDecl | SyntaxKind::AnnotationClassDecl | SyntaxKind::TypeAliasDecl => {
            SymbolKind::CLASS
        }
        SyntaxKind::ObjectDecl | SyntaxKind::CompanionDecl => SymbolKind::OBJECT,
        SyntaxKind::SecondaryConstructor => SymbolKind::CONSTRUCTOR,
        _ => return None,
    })
}
