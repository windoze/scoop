//! Definition-site result queries reuse ordinary expression lowering.

use crate::{FunctionId, Lowerer, TypeId};
use scoop_ast as ast;
use scoop_hir as hir;
use std::rc::Rc;

mod contracts;
pub(crate) use contracts::ResultType;
mod properties;
mod scope;

#[derive(Clone, Default)]
pub(crate) struct DeclarationInference {
    functions: imbl::HashMap<FunctionId, InferredFunction>,
    active: Vec<DeclarationKey>,
    properties: imbl::HashMap<hir::PropertyId, properties::InferredProperty>,
    pub(crate) ready: bool,
    lowered_accessors: std::collections::HashSet<FunctionId>,
    pub(crate) lowered_initializers: std::collections::HashSet<hir::InitializationUnitId>,
    pub(crate) field_initializers: imbl::HashMap<hir::PropertyId, hir::ClassInitializationStep>,
    contracts: Vec<contracts::ResultContract>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclarationKey {
    Function(FunctionId),
    Property(hir::PropertyId),
}

#[derive(Clone)]
struct InferredFunction {
    declaration: Rc<ast::FunctionDecl>,
    state: InferenceState,
}

#[derive(Clone, Copy)]
enum InferenceState {
    Pending,
    Active,
    Complete,
    Failed,
}

#[derive(Clone, Copy)]
pub(crate) enum DeclarationResult {
    Known(TypeId),
    Function(FunctionId),
}

pub(crate) fn infers_result(declaration: &ast::FunctionDecl) -> bool {
    declaration.return_ty.is_none() && matches!(declaration.body, ast::FunctionBody::Expr(_))
}

impl Lowerer {
    fn declaration_inference_path(&self, end: DeclarationKey) -> String {
        self.declaration_inference
            .active
            .iter()
            .chain(std::iter::once(&end))
            .map(|key| match *key {
                DeclarationKey::Function(id) => self.source_function_name(id).to_owned(),
                DeclarationKey::Property(id) => self.properties[id].name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    pub(crate) fn register_inferred_function(
        &mut self,
        id: FunctionId,
        source: &ast::FunctionDecl,
    ) {
        if infers_result(source) && self.function_accessor_kind(id).is_none() {
            self.declaration_inference.functions.insert(
                id,
                InferredFunction {
                    declaration: Rc::new(source.clone()),
                    state: InferenceState::Pending,
                },
            );
        }
    }

    pub(crate) fn has_inferred_function(&self, id: FunctionId) -> bool {
        self.declaration_inference.functions.contains_key(&id)
    }

    pub(crate) fn begin_local_result_inference(
        &mut self,
        id: FunctionId,
        source: &ast::FunctionDecl,
    ) {
        self.register_inferred_function(id, source);
        if let Some(inference) = self.declaration_inference.functions.get_mut(&id) {
            inference.state = InferenceState::Active;
            self.declaration_inference
                .active
                .push(DeclarationKey::Function(id));
        }
    }

    pub(crate) fn finish_local_result_inference(&mut self, id: FunctionId, valid: bool) {
        assert_eq!(
            self.declaration_inference.active.pop(),
            Some(DeclarationKey::Function(id))
        );
        self.declaration_inference
            .functions
            .get_mut(&id)
            .expect("local result query is registered")
            .state = if valid {
            InferenceState::Complete
        } else {
            InferenceState::Failed
        };
    }

    pub(crate) fn set_inferred_function_result(&mut self, id: FunctionId, result: TypeId) {
        self.functions[id].return_ty = result;
        self.signatures
            .get_mut(&id)
            .expect("declared result query")
            .result_pending = false;
        self.signatures
            .get_mut(&id)
            .expect("declared function has a signature")
            .return_ty = result;
        self.current_return_ty = result;
    }

    pub(crate) fn resolve_declaration_result(
        &mut self,
        result: DeclarationResult,
    ) -> Option<TypeId> {
        match result {
            DeclarationResult::Known(ty) => Some(ty),
            DeclarationResult::Function(function) => self.infer_function_result(function),
        }
    }

    pub(crate) fn declaration_inference_failed(&self) -> bool {
        self.declaration_inference
            .functions
            .values()
            .any(|query| matches!(query.state, InferenceState::Failed))
            || self
                .declaration_inference
                .properties
                .values()
                .any(|query| matches!(query.state, InferenceState::Failed))
    }

    pub(crate) fn complete_inferred_functions(&mut self) {
        let mut functions = self
            .declaration_inference
            .functions
            .keys()
            .copied()
            .chain(self.interface_forwarders.keys().copied())
            .collect::<Vec<_>>();
        functions.sort_by_key(|id| id.into_raw().into_u32());
        for function in functions {
            self.infer_function_result(function);
        }
    }

    pub(crate) fn infer_function_result(&mut self, id: FunctionId) -> Option<TypeId> {
        if self.interface_forwarders.contains_key(&id) {
            return self.infer_interface_forwarder_signature(id);
        }
        if let Some(property) = self
            .property_accessor_sources
            .iter()
            .find(|source| source.function == id)
            .map(|source| source.property)
        {
            self.infer_property_type(property)?;
            return Some(self.signatures[&id].return_ty);
        }
        let Some(inference) = self.declaration_inference.functions.get(&id).cloned() else {
            return Some(self.signatures[&id].return_ty);
        };
        match inference.state {
            InferenceState::Complete => return Some(self.signatures[&id].return_ty),
            InferenceState::Failed => return None,
            InferenceState::Active => {
                let names = self.declaration_inference_path(DeclarationKey::Function(id));
                let file = std::mem::replace(&mut self.current_file, self.function_files[&id]);
                self.error(
                    inference.declaration.name.span,
                    format!(
                        "recursive declaration type inference: {names}; add an explicit return type"
                    ),
                );
                self.current_file = file;
                return None;
            }
            InferenceState::Pending => {}
        }
        self.declaration_inference
            .functions
            .get_mut(&id)
            .expect("query is registered")
            .state = InferenceState::Active;
        self.declaration_inference
            .active
            .push(DeclarationKey::Function(id));
        let diagnostics_before = self.diagnostics.len();
        let body = self.lower_inferred_function_body(id, &inference.declaration);
        let file = std::mem::replace(&mut self.current_file, self.function_files[&id]);
        self.validate_inferred_operator_result(id, &inference.declaration);
        self.current_file = file;
        let valid = self.diagnostics.len() == diagnostics_before;
        self.functions[id].kind = hir::FunctionKind::User(body);
        assert_eq!(
            self.declaration_inference.active.pop(),
            Some(DeclarationKey::Function(id))
        );
        self.declaration_inference
            .functions
            .get_mut(&id)
            .expect("query remains registered")
            .state = if valid {
            InferenceState::Complete
        } else {
            InferenceState::Failed
        };
        valid.then_some(self.signatures[&id].return_ty)
    }
}
