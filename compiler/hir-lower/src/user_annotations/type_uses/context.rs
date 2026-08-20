use super::*;

/// Only lexical expression-binding state is saved. Declaration tables and
/// annotation schemas remain the final shared tables of the current lowering.
#[derive(Clone)]
pub(super) struct AnnotationContext {
    current_file: usize,
    current_owner: Option<Owner>,
    type_params_in_scope: Vec<hir::TypeParamDecl>,
    current_source_context: Option<hir::SourceContextId>,
    current_this: Option<(hir::LocalId, hir::TypeId)>,
    locals: crate::SnapshotArena<hir::Local>,
    scopes: crate::Scopes,
    local_function_scopes: crate::LocalFunctionScopes,
    capture_contexts: Vec<crate::CaptureContext>,
    receiver_scopes: Vec<crate::ReceiverScope>,
    local_type_capture_scope: Option<crate::local_types::CaptureScope>,
    constructor_params_in_scope:
        HashMap<String, (hir::ConstructorParamId, hir::TypeId, hir::BindingId)>,
}

macro_rules! context_fields {
    ($action:ident, $a:ident, $b:ident) => {
        $action!(
            $a,
            $b,
            current_file,
            current_owner,
            type_params_in_scope,
            current_source_context,
            current_this,
            locals,
            scopes,
            local_function_scopes,
            capture_contexts,
            receiver_scopes,
            local_type_capture_scope,
            constructor_params_in_scope
        )
    };
}

impl AnnotationContext {
    pub(super) fn capture(lowerer: &Lowerer) -> Self {
        macro_rules! capture { ($a:ident, $b:ident, $($field:ident),*) => { Self { $($field: $a.$field.clone()),* } }; }
        context_fields!(capture, lowerer, unused)
    }

    pub(super) fn swap(&mut self, lowerer: &mut Lowerer) {
        macro_rules! swap { ($a:ident, $b:ident, $($field:ident),*) => { $(std::mem::swap(&mut $a.$field, &mut $b.$field);)* }; }
        context_fields!(swap, self, lowerer);
    }
}
