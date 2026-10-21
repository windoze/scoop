use super::*;
use binding::DocumentationIndex;

type TypeUses = BTreeMap<(usize, u32, u32), (Vec<ast::Ident>, hir::TypeId)>;

impl Lowerer {
    /// Retain only actual resolved type syntax. Rendering never guesses which
    /// identifiers in a source fragment denote types.
    pub(crate) fn document_type_use(&mut self, syntax: &ast::TypeRef, ty: hir::TypeId) {
        let Some(context) = self.documentation.as_mut() else {
            return;
        };
        let (path, span) = match &syntax.kind {
            ast::TypeRefKind::Named(name) | ast::TypeRefKind::Generic(name, _) => {
                (vec![name.clone()], name.span)
            }
            ast::TypeRefKind::Qualified { path, .. } => (
                path.clone(),
                ast::Span::new(path[0].span.start, path.last().unwrap().span.end),
            ),
            ast::TypeRefKind::AppliedMember { name, .. } => (Vec::new(), name.span),
            ast::TypeRefKind::Unit => (Vec::new(), syntax.span),
            _ => return,
        };
        context
            .types
            .insert((self.current_file, span.start, span.end), (path, ty));
    }

    pub(super) fn link_documentation_signature(
        &mut self,
        declaration: &mut hir::DocumentationDeclaration,
        types: &TypeUses,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) {
        let mut result = Vec::new();
        for part in std::mem::take(&mut declaration.signature) {
            let Part::Source(span) = part else {
                result.push(part);
                continue;
            };
            let mut position = span.start;
            for ((_, start, end), (path, ty)) in types
                .range((self.current_file, span.start, 0)..=(self.current_file, span.end, u32::MAX))
            {
                if *start < position || *end > span.end {
                    continue;
                }
                if position < *start {
                    result.push(Part::Source(ast::Span::new(position, *start)));
                }
                let span = ast::Span::new(*start, *end);
                let symbol = path.first().and_then(|first| {
                    let path = ast::QualifiedNameSyntax {
                        first: first.clone(),
                        rest: path
                            .windows(2)
                            .map(|pair| ast::QualifiedNameTailSyntax {
                                dot_span: ast::Span::new(pair[0].span.end, pair[1].span.start),
                                identifier: pair[1].clone(),
                            })
                            .collect(),
                        span,
                    };
                    self.documentation_symbols(declaration.target, &path, index, world, true)
                            .ok()
                            .and_then(|symbols| match symbols.as_slice() {
                                [
                                    symbol @ hir::DocumentationSymbol::Current(
                                        Target::TypeParameter { .. }
                                        | Target::Declaration(
                                            Source::Nominal(_) | Source::TypeAlias(_),
                                        ),
                                    ),
                                ] => Some(*symbol),
                                [
                                    symbol @ hir::DocumentationSymbol::Imported(
                                        hir::AnnotationTargetV1::Nominal(_)
                                        | hir::AnnotationTargetV1::TypeAlias(_),
                                    ),
                                ] => Some(*symbol),
                                _ => None,
                            })
                });
                result.push(match symbol {
                    Some(symbol) => Part::LinkedSource { span, symbol },
                    None => Part::SourceType { span, ty: *ty },
                });
                position = *end;
            }
            if position < span.end {
                result.push(Part::Source(ast::Span::new(position, span.end)));
            }
        }
        declaration.signature = result;
    }
}
