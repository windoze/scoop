//! Source references retained by successful compile-time constant evaluation.

use super::*;

#[derive(Clone, Copy)]
pub(crate) enum ConstantTarget {
    Bound(hir::SemanticProperty),
    Pending(usize),
}

#[derive(Clone)]
pub(crate) struct ConstantFrame {
    file: usize,
    names: imbl::HashMap<ast::Span, ast::Span>,
    properties: imbl::HashMap<ast::Span, ConstantTarget>,
    imports: imbl::HashMap<ast::Span, Vec<hir::SemanticSourceLocation>>,
}

#[derive(Clone, Default)]
pub(crate) struct ConstantObservation {
    active: Option<ConstantFrame>,
    completed: imbl::Vector<ConstantFrame>,
}

impl ConstantObservation {
    pub(crate) fn resolve_pending(&mut self, properties: &[Option<hir::PropertyId>]) {
        for frame in &mut self.completed {
            frame.properties = std::mem::take(&mut frame.properties)
                .into_iter()
                .map(|(span, target)| {
                    let target = match target {
                        ConstantTarget::Bound(_) => target,
                        ConstantTarget::Pending(index) => {
                            ConstantTarget::Bound(hir::SemanticProperty::Current(
                                properties[index]
                                    .expect("a successful const only uses successful consts"),
                            ))
                        }
                    };
                    (span, target)
                })
                .collect();
        }
    }

    pub(crate) fn finish(self, lowerer: &Lowerer) -> Vec<hir::EditingConstantReference> {
        let mut references = self
            .completed
            .into_iter()
            .flat_map(|frame| {
                frame.properties.into_iter().map(move |(span, target)| {
                    let ConstantTarget::Bound(target) = target else {
                        unreachable!("the const worklist resolves references after allocation")
                    };
                    hir::EditingConstantReference {
                        origin: hir::SemanticSourceLocation {
                            source: lowerer.source_identity(frame.file).clone(),
                            span,
                        },
                        target,
                        imports: frame.imports.get(&span).cloned().unwrap_or_default(),
                    }
                })
            })
            .collect::<Vec<_>>();
        references.sort_by_key(|reference| {
            (reference.origin.source.clone(), reference.origin.span.start)
        });
        references
    }
}

impl Lowerer {
    pub(crate) fn begin_constant_observation(&mut self) -> Option<ConstantFrame> {
        self.editing_constants
            .as_mut()?
            .active
            .replace(ConstantFrame {
                file: self.current_file,
                names: Default::default(),
                properties: Default::default(),
                imports: Default::default(),
            })
    }

    pub(crate) fn finish_constant_observation(
        &mut self,
        outer: Option<ConstantFrame>,
        complete: bool,
    ) {
        let Some(observation) = &mut self.editing_constants else {
            return;
        };
        let frame = std::mem::replace(&mut observation.active, outer)
            .expect("constant evaluation opened its observation");
        if complete {
            observation.completed.push_back(frame);
        }
    }

    pub(crate) fn observe_constant_name(&mut self, span: ast::Span, name: ast::Span) {
        if let Some(frame) = self.constant_frame() {
            frame.names.insert(span, name);
            frame.names.insert(name, name);
        }
    }

    pub(crate) fn observe_constant_reference(&mut self, span: ast::Span, target: ConstantTarget) {
        let Some(frame) = self.constant_frame() else {
            return;
        };
        if let Some(name) = frame.names.get(&span).copied() {
            frame.properties.insert(name, target);
        }
    }

    pub(crate) fn observe_constant_import(
        &mut self,
        name: &ast::Ident,
        layer: crate::imports::ImportLookupLayer,
    ) {
        if layer != crate::imports::ImportLookupLayer::Exact
            || self
                .constant_frame()
                .is_none_or(|frame| !frame.names.contains_key(&name.span))
        {
            return;
        }
        let imports = self.editing_exact_import_origins(name);
        self.constant_frame()
            .expect("active constant evaluation")
            .imports
            .insert(name.span, imports);
    }

    fn constant_frame(&mut self) -> Option<&mut ConstantFrame> {
        self.editing_constants
            .as_mut()?
            .active
            .as_mut()
            .filter(|frame| frame.file == self.current_file)
    }
}
