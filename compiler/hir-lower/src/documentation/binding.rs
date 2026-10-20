use super::*;
use hir::DocumentationLink as Link;

mod index;
mod tags;
pub(super) use index::DocumentationIndex;

#[derive(Eq, Hash, PartialEq)]
enum ReferenceTag {
    Throws(hir::DocumentationSymbol),
    See(Vec<hir::DocumentationSymbol>),
    Url(String),
}

impl Lowerer {
    pub(crate) fn finish_documentation(
        &mut self,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Option<hir::ConeDocumentation> {
        self.document_synthesized_members();
        let mut context = self.documentation.take()?.into_owned();
        let mut local = context
            .declarations
            .iter()
            .filter_map(|entry| match entry.declaration.target {
                Target::Declaration(Source::Callable(Callable::Function(id)))
                    if self.local_function_by_function.contains_key(&id) =>
                {
                    Some(entry.declaration.target)
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        loop {
            let count = local.len();
            for entry in &context.declarations {
                if entry
                    .declaration
                    .parent
                    .is_some_and(|parent| local.contains(&parent))
                {
                    local.insert(entry.declaration.target);
                }
            }
            if local.len() == count {
                break;
            }
        }
        context
            .declarations
            .retain(|entry| !local.contains(&entry.declaration.target));
        let index = DocumentationIndex::new(&context.declarations);
        // Lookup can materialize imported type applications. Keep those query
        // results out of the ordinary compilation and machine-use selection.
        let mut resolver = self.clone();
        resolver.scopes = crate::Scopes::new();
        resolver.diagnostics.clear();
        let mut warnings = context.warnings;
        let mut seen = HashSet::new();
        let mut declarations = Vec::with_capacity(context.declarations.len());
        for captured in context.declarations {
            let mut declaration = captured.declaration;
            resolver.current_file = captured.scope.file;
            resolver.current_owner = captured.scope.owner;
            resolver.type_params_in_scope = captured.scope.parameters;
            if let Some(comment) = captured.comment {
                declaration.comment = Some(resolver.bind_documentation(
                    declaration.target,
                    comment,
                    &index,
                    world,
                    &mut warnings,
                    &mut seen,
                ));
            }
            declarations.push(declaration);
        }
        let sources = self.diagnostic_source_identities();
        for warning in &mut warnings {
            warning.resolve_sources(&sources);
        }
        Some(hir::ConeDocumentation {
            declarations,
            warnings,
            hidden_annotation: self.core_source_annotation("DocHidden"),
            alias_annotation: self.core_source_annotation("DocAlias"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_documentation(
        &mut self,
        target: Target,
        source: ast::DocDocument,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        warnings: &mut Vec<ast::Diagnostic>,
        seen: &mut HashSet<(u8, Target)>,
    ) -> hir::DocumentationComment {
        let body = self.bind_documentation_markdown(target, source.body, index, world, warnings);
        let mut tags = Vec::new();
        let mut references = HashSet::new();
        for tag in source.tags {
            if let Some(tag) =
                self.bind_documentation_tag(target, tag, index, world, warnings, seen)
            {
                let reference = match &tag.kind {
                    hir::DocumentationTagKind::Throws(symbol) => {
                        Some(ReferenceTag::Throws(*symbol))
                    }
                    hir::DocumentationTagKind::See(Link::Symbols(symbols)) => {
                        Some(ReferenceTag::See(symbols.iter().copied().collect()))
                    }
                    hir::DocumentationTagKind::See(Link::Url(url)) => {
                        Some(ReferenceTag::Url(url.clone()))
                    }
                    _ => None,
                };
                if reference.is_some_and(|reference| !references.insert(reference)) {
                    self.documentation_warning(
                        warnings,
                        tag.span,
                        format!("duplicate documentation tag `{}`", tag.label),
                    );
                    continue;
                }
                tags.push(tag);
            }
        }
        hir::DocumentationComment {
            span: source.span,
            body,
            tags,
        }
    }

    fn bind_documentation_markdown(
        &mut self,
        target: Target,
        source: ast::DocMarkdown,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        warnings: &mut Vec<ast::Diagnostic>,
    ) -> hir::DocumentationMarkdown {
        let references = source
            .references
            .into_iter()
            .map(|reference| {
                let resolution =
                    self.bind_documentation_link(target, &reference.path, index, world, warnings);
                hir::DocumentationReference {
                    range: reference.range,
                    resolution,
                }
            })
            .collect();
        hir::DocumentationMarkdown {
            text: source.text,
            definitions: source.definitions,
            references,
        }
    }

    fn bind_documentation_link(
        &mut self,
        target: Target,
        path: &ast::QualifiedNameSyntax,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        warnings: &mut Vec<ast::Diagnostic>,
    ) -> Link {
        match self.documentation_symbols(target, path, index, world) {
            Ok(symbols) => {
                let mut symbols = symbols.into_iter();
                let first = symbols
                    .next()
                    .expect("a successful symbol query is non-empty");
                Link::Symbols(ast::NonEmptyVec::new(first, symbols.collect()))
            }
            Err(reason) => {
                let name = path
                    .segments()
                    .map(|name| name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.documentation_warning(
                    warnings,
                    path.span,
                    format!("documentation reference `{name}` {reason}"),
                );
                Link::Unresolved
            }
        }
    }

    fn documentation_warning(
        &self,
        warnings: &mut Vec<ast::Diagnostic>,
        span: ast::Span,
        message: impl Into<String>,
    ) {
        warnings.push(ast::Diagnostic::warning_at_file(
            self.current_file,
            span,
            message,
        ));
    }
}
