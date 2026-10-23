use super::*;
use crate::pipeline::cancellation::FrontendCancellation;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct BodyObservation {
    pub(crate) types: super::super::types::TypeObservation,
    pub(super) id: hir::EditingSourceBodyId,
    pub(super) root: hir::LexicalDefinitionRoot,
    pub(super) file: usize,
    pub(super) diagnostics_start: usize,
    cancellation: FrontendCancellation,
    pub(super) failed: bool,
    pub(super) bindings: imbl::HashMap<hir::BindingId, hir::EditingLocalOccurrence>,
    pub(super) uses:
        imbl::HashMap<ast::Span, (hir::BindingId, hir::SourceContextId, hir::EditingLocalRole)>,
    pub(super) expressions: imbl::HashMap<ast::Span, hir::EditingExpression>,
    pub(super) callable_names: imbl::HashMap<ast::Span, ast::Span>,
    pub(super) callables: imbl::HashMap<ast::Span, Vec<hir::EditingCallableReference>>,
    pub(super) property_names: imbl::HashMap<ast::Span, ast::Span>,
    pub(super) properties: imbl::HashMap<ast::Span, hir::EditingPropertyReference>,
    pub(super) local_functions: imbl::HashMap<hir::FunctionId, hir::EditingFunction>,
    pub(super) imports: imbl::HashMap<ast::Span, Vec<hir::EditingImportSource>>,
    pub(super) call_inputs: imbl::HashMap<ast::Span, calls::CallInput>,
    pub(super) argument_maps: imbl::HashMap<ast::Span, calls::CheckedArguments>,
    pub(super) calls: imbl::HashMap<ast::Span, hir::EditingAppliedCall>,
    pub(super) selector_inputs: imbl::HashMap<ast::Span, Vec<selectors::SelectorInput>>,
    pub(super) selectors: imbl::HashMap<ast::Span, hir::EditingSelector>,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

#[derive(Clone)]
pub(in crate::editing) struct ObservedContext {
    pub(in crate::editing) file: usize,
    pub(in crate::editing) owner: Option<crate::Owner>,
    pub(in crate::editing) parameters: Vec<hir::TypeParamDecl>,
}

#[derive(Clone)]
pub(crate) struct ObservedBody {
    pub(super) facts: V<hir::EditingBodyFacts>,
    pub(super) selectors: Vec<hir::EditingSelector>,
    pub(super) calls: Vec<hir::EditingAppliedCall>,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

impl ObservedBody {
    pub(in crate::editing) fn available(&self) -> bool {
        matches!(self.facts, V::Known(_))
    }
}

impl BodyObservation {
    pub(crate) fn new(
        id: hir::EditingSourceBodyId,
        root: hir::LexicalDefinitionRoot,
        file: usize,
        diagnostics_start: usize,
        check: impl Fn() -> bool + 'static,
    ) -> Self {
        Self {
            types: Default::default(),
            id,
            root,
            file,
            diagnostics_start,
            cancellation: FrontendCancellation::new(check),
            failed: false,
            bindings: Default::default(),
            uses: Default::default(),
            expressions: Default::default(),
            callable_names: Default::default(),
            callables: Default::default(),
            property_names: Default::default(),
            properties: Default::default(),
            local_functions: Default::default(),
            imports: Default::default(),
            call_inputs: Default::default(),
            argument_maps: Default::default(),
            calls: Default::default(),
            selector_inputs: Default::default(),
            selectors: Default::default(),
            contexts: Default::default(),
        }
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancellation.cancelled()
    }

    pub(crate) fn cancellation(&self) -> Rc<dyn Fn() -> bool> {
        let cancellation = self.cancellation.clone();
        Rc::new(move || cancellation.cancelled())
    }

    pub(crate) fn finish(self, lowerer: &mut Lowerer, complete: bool) -> ObservedBody {
        let complete =
            complete && !self.failed && lowerer.diagnostics.len() == self.diagnostics_start;
        let (types, type_applications) = if complete {
            self.types.finish(lowerer)
        } else {
            (Vec::new(), Vec::new())
        };
        let local_type = |binding, ty| {
            lowerer
                .local_delegate_plans
                .get(&binding)
                .map_or(ty, |plan| plan.property_ty)
        };
        let mut locals = self
            .bindings
            .values()
            .cloned()
            .map(|mut definition| {
                definition.value_type =
                    local_type(definition.reference.binding, definition.value_type);
                definition
            })
            .collect::<Vec<_>>();
        for (span, (binding, context, role)) in &self.uses {
            let Some(definition) = self
                .bindings
                .get(binding)
                .or_else(|| lowerer.editing_local_definitions.get(binding))
            else {
                continue;
            };
            let value_type = self.expressions.get(span).map_or_else(
                || local_type(*binding, definition.value_type),
                |expression| expression.value_type,
            );
            locals.push(hir::EditingLocalOccurrence {
                span: *span,
                context: *context,
                reference: definition.reference.clone(),
                value_type,
                role: *role,
            });
        }
        locals.sort_by_key(|local| (local.span.start, local.span.end));
        let mut expressions = self
            .expressions
            .into_iter()
            .map(|(_, expression)| expression)
            .collect::<Vec<_>>();
        expressions.sort_by_key(|expression| (expression.span.start, expression.span.end));
        let mut callables = self
            .callables
            .into_iter()
            .flat_map(|(_, references)| references)
            .collect::<Vec<_>>();
        callables.sort_by_key(|reference| (reference.span.start, reference.span.end));
        let mut properties = self
            .properties
            .into_iter()
            .map(|(_, reference)| reference)
            .collect::<Vec<_>>();
        properties.sort_by_key(|reference| (reference.span.start, reference.span.end));
        let mut local_functions = self
            .local_functions
            .into_iter()
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        local_functions.sort_by_key(|function| function.name_origin.span.start);
        let mut imports = self
            .imports
            .into_iter()
            .map(|(span, imports)| hir::EditingImportUse { span, imports })
            .collect::<Vec<_>>();
        imports.sort_by_key(|reference| (reference.span.start, reference.span.end));
        let mut selectors = self
            .selectors
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        selectors.sort_by_key(|selector| selector.selector.start);
        let mut calls = self
            .calls
            .into_iter()
            .map(|(_, call)| call)
            .collect::<Vec<_>>();
        calls.sort_by_key(|call| (call.span.start, call.span.end));
        ObservedBody {
            facts: if complete {
                V::Known(hir::EditingBodyFacts {
                    types,
                    type_applications,
                    locals,
                    expressions,
                    callables,
                    properties,
                    local_functions,
                    imports,
                })
            } else {
                V::Unavailable(U::BodyFailure)
            },
            selectors,
            calls,
            contexts: self.contexts,
        }
    }
}
