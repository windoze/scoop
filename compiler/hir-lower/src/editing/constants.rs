//! Source references retained by successful compile-time constant evaluation.

use super::*;

mod calls;
mod variants;

#[derive(Default)]
pub(crate) struct ConstantFacts {
    pub(crate) references: Vec<hir::EditingConstantReference>,
    pub(crate) variants: Vec<hir::EditingStaticVariantReference>,
    pub(crate) calls: Vec<hir::EditingStaticCall>,
}

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
    imports: imbl::HashMap<ast::Span, Vec<hir::EditingImportSource>>,
    variants: imbl::HashMap<ast::Span, hir::EditingStaticVariantReference>,
    call_inputs: imbl::HashMap<ast::Span, super::bodies::CallInput>,
    calls: imbl::HashMap<ast::Span, hir::EditingAppliedCall>,
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

    pub(crate) fn finish(self, lowerer: &Lowerer) -> ConstantFacts {
        let mut facts = ConstantFacts::default();
        for frame in self.completed {
            for (span, target) in frame.properties {
                let ConstantTarget::Bound(target) = target else {
                    unreachable!("the const worklist resolves references after allocation")
                };
                facts.references.push(hir::EditingConstantReference {
                    origin: hir::SemanticSourceLocation {
                        source: lowerer.source_identity(frame.file).clone(),
                        span,
                    },
                    target,
                    imports: frame.imports.get(&span).cloned().unwrap_or_default(),
                });
            }
            for (span, mut reference) in frame.variants {
                reference.imports = frame.imports.get(&span).cloned().unwrap_or_default();
                facts.variants.push(reference);
            }
            facts.calls.extend(
                frame
                    .calls
                    .into_iter()
                    .map(|(_, call)| hir::EditingStaticCall {
                        source: lowerer.source_identity(frame.file).clone(),
                        call,
                    }),
            );
        }
        facts.references.sort_by_key(|reference| {
            (reference.origin.source.clone(), reference.origin.span.start)
        });
        facts.variants.sort_by_key(|reference| {
            (reference.origin.source.clone(), reference.origin.span.start)
        });
        facts
            .calls
            .sort_by_key(|call| (call.source.clone(), call.call.span.start));
        facts
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
                variants: Default::default(),
                call_inputs: Default::default(),
                calls: Default::default(),
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
        if self
            .constant_frame()
            .is_none_or(|frame| !frame.names.contains_key(&name.span))
        {
            return;
        }
        let imports = self.editing_import_sources(name, layer);
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
