use std::collections::BTreeSet;

use scoop_source::{SourceLengthError, Span};
use scoop_syntax::{DeclarationSyntax, SyntaxKind};

use crate::{AnalysisSnapshot, Cancelled, ProjectSource, SourceInput, SyntaxPtr};

/// A source declaration search result, not a bound language entity.
#[derive(Debug)]
pub struct SourceSymbol {
    pub name: String,
    pub container: Option<String>,
    pub declaration: SyntaxPtr,
    pub name_span: Span,
    pub parent_kind: Option<SyntaxKind>,
    search_name: String,
}

#[derive(Debug)]
pub struct WorkspaceSymbol<'a> {
    pub file: &'a ProjectSource,
    pub symbol: &'a SourceSymbol,
}

#[derive(Debug)]
pub enum SymbolQueryError {
    Cancelled,
    MissingSource(crate::SourceId),
    SourceLength(SourceLengthError),
}

impl From<Cancelled> for SymbolQueryError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

impl SourceInput {
    pub fn symbols(&self) -> Result<&[SourceSymbol], &SourceLengthError> {
        let parsed = self.parse()?;
        Ok(self.symbols.get_or_init(|| {
            let declarations = parsed.declarations();
            declarations
                .iter()
                .filter_map(|declaration| {
                    let name = declaration.name.as_ref()?;
                    if !is_symbol(declaration.kind) {
                        return None;
                    }
                    let container = container(declaration, declarations)?;
                    Some(SourceSymbol {
                        name: name.text.clone(),
                        container,
                        declaration: SyntaxPtr {
                            source: self.version,
                            kind: declaration.kind,
                            span: declaration.span,
                        },
                        name_span: name.span,
                        parent_kind: declaration.parent.map(|parent| declarations[parent.0].kind),
                        search_name: name.text.to_lowercase(),
                    })
                })
                .collect()
        }))
    }
}

impl AnalysisSnapshot {
    pub fn workspace_symbols(
        &self,
        query: &str,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<Vec<WorkspaceSymbol<'_>>, SymbolQueryError> {
        self.check_cancelled()?;
        if is_cancelled() {
            return Err(SymbolQueryError::Cancelled);
        }
        let query = query.to_lowercase();
        let mut files = BTreeSet::new();
        let mut results = Vec::new();
        let resolved = self.workspace.projects.iter().flat_map(|project| {
            project
                .cones
                .values()
                .filter_map(|cone| cone.source())
                .flat_map(|cone| &cone.sources)
                .filter(|file| {
                    self.workspace
                        .project_for_source(file.source)
                        .is_some_and(|selected| std::ptr::eq(project, selected))
                })
        });
        let unresolved = self.workspace.unresolved_members.iter().flat_map(|member| {
            member.sources.iter().filter(|file| {
                self.workspace
                    .unresolved_for_source(file.source)
                    .is_some_and(|selected| std::ptr::eq(member, selected))
            })
        });
        for file in resolved.chain(unresolved) {
            self.check_cancelled()?;
            if is_cancelled() {
                return Err(SymbolQueryError::Cancelled);
            }
            if !files.insert(file.source) {
                continue;
            }
            let source = self
                .source(file.source)?
                .ok_or(SymbolQueryError::MissingSource(file.source))?;
            for symbol in source
                .symbols()
                .map_err(|error| SymbolQueryError::SourceLength(*error))?
            {
                if is_cancelled() {
                    return Err(SymbolQueryError::Cancelled);
                }
                if symbol.search_name.contains(&query) {
                    results.push(WorkspaceSymbol { file, symbol });
                }
            }
        }
        self.check_cancelled()?;
        results.sort_by(|left, right| {
            left.file.path.cmp(&right.file.path).then_with(|| {
                left.symbol
                    .name_span
                    .start
                    .cmp(&right.symbol.name_span.start)
            })
        });
        Ok(results)
    }
}

fn is_symbol(kind: SyntaxKind) -> bool {
    kind.is_declaration() && !matches!(kind, SyntaxKind::Parameter | SyntaxKind::TypeParameter)
}

fn container(
    declaration: &DeclarationSyntax,
    declarations: &[DeclarationSyntax],
) -> Option<Option<String>> {
    let mut names = Vec::new();
    let mut parent = declaration.parent;
    while let Some(index) = parent {
        let declaration = &declarations[index.0];
        if !matches!(
            declaration.kind,
            SyntaxKind::ClassDecl
                | SyntaxKind::StructDecl
                | SyntaxKind::EnumDecl
                | SyntaxKind::InterfaceDecl
                | SyntaxKind::ObjectDecl
                | SyntaxKind::CompanionDecl
                | SyntaxKind::AnnotationClassDecl
        ) {
            return None;
        }
        names.push(declaration.name.as_ref()?.text.as_str());
        parent = declaration.parent;
    }
    names.reverse();
    Some((!names.is_empty()).then(|| names.join(".")))
}
