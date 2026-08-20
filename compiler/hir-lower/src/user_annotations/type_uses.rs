use super::*;
use hir::{AnnotationParameterType, TypeAnnotationNode, TypeUseStep};
use std::collections::HashMap;

mod aliases;
mod binding;
mod children;
mod context;
mod finish;
mod sites;
use context::AnnotationContext;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TypeUseKey {
    file: usize,
    start: u32,
    end: u32,
}
impl TypeUseKey {
    fn new(file: usize, span: ast::Span) -> Self {
        Self {
            file,
            start: span.start,
            end: span.end,
        }
    }
    fn span(self) -> ast::Span {
        ast::Span::new(self.start, self.end)
    }
}

#[derive(Clone)]
struct TypeUsePart {
    key: TypeUseKey,
    from: Vec<TypeUseStep>,
    to: Vec<TypeUseStep>,
}

#[derive(Clone)]
enum AliasAnnotations {
    Source(TypeUseKey),
    Imported(Vec<TypeAnnotationNode>),
}

#[derive(Clone)]
struct AliasTypeUse {
    target: hir::TypeId,
    parameters: Vec<hir::TypeParamId>,
    annotations: AliasAnnotations,
    arguments: Vec<Option<TypeUsePart>>,
}

#[derive(Clone)]
struct PendingTypeUse {
    ty: AnnotationParameterType<hir::TypeId>,
    parameters: Vec<hir::TypeParamId>,
    direct: Option<(AnnotationContext, Vec<ast::Annotation>)>,
    children: Vec<TypeUsePart>,
    alias: Option<AliasTypeUse>,
}

#[derive(Clone)]
struct TypeUseFrame {
    key: TypeUseKey,
    selector: Option<ast::Span>,
    host: Option<TypeUseKey>,
    direct: Option<(AnnotationContext, Vec<ast::Annotation>)>,
    alias: Option<AliasTypeUse>,
}

#[derive(Clone, Default)]
pub(crate) struct PendingTypeAnnotations {
    uses: crate::Shared<HashMap<TypeUseKey, PendingTypeUse>>,
    frames: Vec<TypeUseFrame>,
    sites: Vec<(
        hir::SourceAnnotationTarget,
        hir::AnnotationTypeSlot,
        TypeUseKey,
    )>,
    pub(super) suspended: bool,
}

impl Lowerer {
    pub(crate) fn begin_type_annotations(&mut self, source: &ast::TypeRef) -> bool {
        if self.type_annotations.suspended {
            return false;
        }
        let selector = match &source.kind {
            ast::TypeRefKind::Named(name)
            | ast::TypeRefKind::Generic(name, _)
            | ast::TypeRefKind::AppliedMember { name, .. } => Some(name.span),
            ast::TypeRefKind::Qualified { path, .. } => path.last().map(|name| name.span),
            _ => None,
        };
        let host = match &source.kind {
            ast::TypeRefKind::AppliedMember { owner, .. } => {
                Some(TypeUseKey::new(self.current_file, owner.span))
            }
            _ => None,
        };
        let direct = (!source.annotations.is_empty())
            .then(|| (AnnotationContext::capture(self), source.annotations.clone()));
        self.type_annotations.frames.push(TypeUseFrame {
            key: TypeUseKey::new(self.current_file, source.span),
            selector,
            host,
            direct,
            alias: None,
        });
        true
    }

    pub(crate) fn end_type_annotations(&mut self, source: &ast::TypeRef, ty: Option<hir::TypeId>) {
        let frame = self
            .type_annotations
            .frames
            .pop()
            .expect("a type resolver owns its frame");
        let Some(ty) = ty else {
            return;
        };
        let children = if frame.alias.is_none() {
            self.type_annotation_children(source, ty)
        } else {
            Vec::new()
        };
        let inherited = frame.alias.as_ref().is_some_and(|alias| {
            (match &alias.annotations {
                AliasAnnotations::Source(key) => self.type_annotations.uses.contains_key(key),
                AliasAnnotations::Imported(nodes) => !nodes.is_empty(),
            }) || {
                alias
                    .arguments
                    .iter()
                    .flatten()
                    .any(|part| self.type_annotations.uses.contains_key(&part.key))
            }
        });
        if frame.direct.is_none() && children.is_empty() && !inherited {
            return;
        }
        self.type_annotations.uses.insert(
            frame.key,
            PendingTypeUse {
                ty: AnnotationParameterType::Value(ty),
                parameters: self
                    .type_params_in_scope
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect(),
                direct: frame.direct,
                children,
                alias: frame.alias,
            },
        );
    }

    fn type_use_identity(&self, key: TypeUseKey) -> hir::AnnotationTypeUseId {
        hir::AnnotationTypeUseId {
            source: self.intrinsic_sources[key.file].identity.clone(),
            span: scoop_identity::SourceSpan::new(key.start.into(), key.end.into())
                .expect("parser spans are ordered"),
        }
    }
}
