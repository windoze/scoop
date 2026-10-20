use super::*;
use hir::DocumentationTagKind as Bound;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_documentation_tag(
        &mut self,
        owner: Target,
        tag: ast::DocTag,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        warnings: &mut Vec<ast::Diagnostic>,
        seen: &mut HashSet<(u8, Target)>,
    ) -> Option<hir::DocumentationTag> {
        let mut destination = None;
        let (kind, label) = match &tag.kind {
            ast::DocTagKind::Param(name) => {
                let target = index
                    .children(owner)
                    .find(|(target, kind, candidate)| {
                        matches!(
                            kind,
                            Kind::Parameter | Kind::TypeParameter | Kind::ContextParameter
                        ) && !matches!(target, Target::Declaration(Source::Receiver(_)))
                            && *candidate == name.text
                            && name.text != "_"
                    })
                    .map(|entry| entry.0)
                    .or_else(|| {
                        let constructor = self.documentation_primary(owner)?;
                        index
                            .children(constructor)
                            .find(|(_, _, candidate)| *candidate == name.text)
                            .map(|entry| entry.0)
                    });
                destination = target.map(|target| (0, target, true));
                (target.map(Bound::Param), name.text.clone())
            }
            ast::DocTagKind::Property(name) => {
                let target = index
                    .children(owner)
                    .find(|(_, kind, candidate)| {
                        matches!(kind, Kind::Property | Kind::Field) && *candidate == name.text
                    })
                    .map(|entry| entry.0);
                destination = target.map(|target| (1, target, true));
                (target.map(Bound::Property), name.text.clone())
            }
            ast::DocTagKind::Constructor => {
                let target = self.documentation_primary(owner);
                destination = target.map(|target| (2, target, true));
                (target.map(Bound::Constructor), "constructor".into())
            }
            ast::DocTagKind::Receiver => {
                let target = index
                    .children(owner)
                    .find(|(target, _, _)| {
                        matches!(target, Target::Declaration(Source::Receiver(_)))
                    })
                    .map(|entry| entry.0);
                destination = target.map(|target| (3, target, true));
                (target.map(Bound::Receiver), "receiver".into())
            }
            ast::DocTagKind::Return => {
                let target = match owner {
                    Target::Declaration(Source::Callable(
                        Callable::Function(_) | Callable::Getter(_),
                    )) => Some(owner),
                    _ => None,
                };
                destination = target.map(|target| (4, target, false));
                (target.map(Bound::Return), "return".into())
            }
            ast::DocTagKind::Since => {
                destination = Some((5, owner, false));
                (Some(Bound::Since), "since".into())
            }
            ast::DocTagKind::See(target) => {
                let (link, label) = match target {
                    ast::DocSeeTarget::Symbol(path) => (
                        self.bind_documentation_link(owner, path, index, world, warnings),
                        path.segments()
                            .map(|name| name.text.as_str())
                            .collect::<Vec<_>>()
                            .join("."),
                    ),
                    ast::DocSeeTarget::Url { text, .. } => (Link::Url(text.clone()), text.clone()),
                };
                (Some(Bound::See(link)), label)
            }
            ast::DocTagKind::Throws(path) => {
                let label = path
                    .segments()
                    .map(|name| name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let link = self.bind_documentation_link(owner, path, index, world, warnings);
                let symbol = match link {
                    Link::Symbols(symbols) if symbols.len() == 1 => Some(*symbols.first()),
                    Link::Unresolved => return None,
                    _ => None,
                };
                let exception = symbol.and_then(|symbol| {
                    let ty = self.documentation_symbol_type(symbol, world)?;
                    let throwable = self.throwable_ty(path.span)?;
                    self.is_subtype(ty, throwable).then_some(symbol)
                });
                (exception.map(Bound::Throws), label)
            }
        };
        let Some(kind) = kind else {
            self.documentation_warning(
                warnings,
                tag.span,
                format!("documentation tag has no applicable target `{label}`"),
            );
            return None;
        };
        if let Some((category, target, direct)) = destination
            && (!seen.insert((category, target)) || (direct && index.has_comment(target)))
        {
            self.documentation_warning(warnings, tag.span, format!("duplicate documentation for `{label}`; the first direct or tagged description is retained"));
            return None;
        }
        let content_owner = destination.map_or(owner, |(_, target, _)| target);
        let body =
            self.bind_documentation_markdown(content_owner, tag.body, index, world, warnings);
        Some(hir::DocumentationTag {
            kind,
            label,
            span: tag.span,
            body,
        })
    }

    fn documentation_primary(&self, target: Target) -> Option<Target> {
        let callable = match target {
            Target::Declaration(Source::Nominal(hir::NominalOwner::Class(id))) => self.classes[id]
                .constructors
                .iter()
                .copied()
                .find(|id| {
                    matches!(
                        self.class_constructors[*id].kind,
                        hir::ClassConstructorKind::Primary { .. }
                    ) && self.class_constructors[*id].identity_kind
                        == hir::ClassConstructorIdentityKind::Source
                })
                .map(Callable::ClassConstructor),
            Target::Declaration(Source::Nominal(hir::NominalOwner::Struct(id))) => self.structs[id]
                .constructors
                .iter()
                .copied()
                .find(|id| {
                    matches!(
                        self.struct_constructors[*id].kind,
                        hir::StructConstructorKind::Primary
                    )
                })
                .map(Callable::StructConstructor),
            _ => None,
        }?;
        Some(Source::Callable(callable).into())
    }
}
