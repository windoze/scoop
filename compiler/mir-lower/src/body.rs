use super::*;

mod callbacks;
mod calls;
mod casts;
mod expressions;
mod function;
mod operators;
mod patterns;
mod statements;

/// Per-function-body lowering state.
pub(super) struct BodyLowerer<'a> {
    pub(super) module: &'a hir::Module,
    pub(super) struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    pub(super) class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
    pub(super) interfaces: &'a mut InterfaceRegistry,
    /// MIR struct definitions used for representation and GC
    /// classification of compiler-synthesized aggregates.
    pub(super) structs: &'a mut StructRegistry,
    /// Method signature key -> vtable slot per class
    /// (`compute_dispatch`).
    pub(super) method_slots: &'a HashMap<mir::ClassId, HashMap<hir::VirtualMethodId, u32>>,
    pub(super) function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    pub(super) extern_map: &'a HashMap<hir::ExternFunctionId, mir::ExternFunctionId>,
    pub(super) global_map: &'a HashMap<hir::GlobalId, mir::GlobalId>,
    pub(super) singleton_root_map:
        &'a HashMap<hir::SingletonPublishedRootId, mir::SingletonPublishedRootId>,
    pub(super) singleton_published_roots: &'a Arena<mir::SingletonPublishedRoot>,
    pub(super) callback_bridges: &'a mut Arena<mir::CallbackBridge>,
    pub(super) callback_by_target:
        &'a mut HashMap<(mir::FunctionId, mir::FunctionTypeId), mir::CallbackBridgeId>,
    pub(super) foreign_callback_adapters: &'a mut Arena<mir::ForeignCallbackAdapter>,
    pub(super) foreign_callback_families: &'a mut Arena<mir::ForeignCallbackFamily>,
    pub(super) foreign_callback_family_by_callback:
        &'a mut HashMap<mir::StructId, mir::ForeignCallbackFamilyId>,
    pub(super) foreign_callback_bridges: &'a mut Arena<mir::ForeignCallbackBridge>,
    pub(super) foreign_callback_by_registration:
        &'a mut HashMap<hir::ForeignCallbackRegistrationId, mir::ForeignCallbackBridgeId>,
    /// Local-concrete constructor callable -> MIR function.
    pub(super) ctors: &'a HashMap<hir::ClassConstructorId, mir::FunctionId>,
    pub(super) struct_ctors: &'a HashMap<hir::StructConstructorId, mir::FunctionId>,
    pub(super) strings: &'a mut Arena<mir::StringConst>,
    pub(super) functions: &'a mut Arena<mir::Function>,
    pub(super) top_level: &'a mut Vec<mir::FunctionId>,
    pub(super) instances: &'a mut InstanceRegistry,
    /// Instantiated enum definitions, filled on creation; variant
    /// field types feed pattern lowering and representation.
    pub(super) enums: &'a mut EnumRegistry,
    /// Boxed value types discovered in this body (`Box` / `is` / `as`).
    pub(super) boxed: &'a mut BoxedRegistry,
    /// MIR class arena (boxed value types are appended here).
    pub(super) classes: &'a mut Arena<mir::ClassDef>,
    /// Mangling shell (enum / struct names for `encode_type`).
    pub(super) shell: &'a mut mir::Module,
    /// HIR local -> MIR local (same declaration order per body).
    pub(super) local_map: HashMap<hir::LocalId, mir::LocalId>,
    /// Constructor-parameter identities available while lowering one
    /// generated class constructor's delegation expressions.
    pub(super) constructor_param_map: HashMap<hir::ConstructorParamId, smir::Expr>,
    /// Hidden initializer receiver. It is an ordinary MIR local; the source
    /// non-escaping capability has already been eliminated by concretization.
    pub(super) constructor_receiver: Option<smir::Expr>,
    /// MIR locals, including the hidden ones created during lowering
    /// (`when` subjects, destructuring slots, `!!` temporaries).
    pub(super) locals: Arena<mir::Local>,
    pub(super) hidden_count: usize,
    /// Statement kinds that must precede the statement currently being
    /// lowered (the trap test of `!!`); drained by the caller.
    pub(super) prelude: Vec<smir::StatementKind>,
    /// Declaration indices of `Option::Some` / `Option::None`.
    pub(super) option_variants: (u32, u32),
    pub(super) coroutines: &'a mut CoroutineRegistry,
    pub(super) lambda_closures: &'a HashMap<hir::LambdaId, mir::ClosureClassId>,
    pub(super) anonymous_closures: &'a HashMap<hir::AnonymousFunctionId, mir::ClosureClassId>,
    pub(super) reference_closures: &'a HashMap<hir::CallableReferenceId, mir::ClosureClassId>,
    pub(super) closure_classes: &'a mut Arena<mir::ClosureClass>,
    pub(super) closure_invokes: &'a mut Arena<mir::ClosureInvokeFunction>,
    pub(super) closure_capture_indices: &'a mut HashMap<(mir::ClosureClassId, hir::BindingId), u32>,
    pub(super) closure_adapters: &'a mut Arena<mir::ClosureAdapter>,
    pub(super) closure_adapter_by_types:
        &'a mut HashMap<(mir::FunctionTypeId, mir::FunctionTypeId), mir::ClosureAdapterId>,
    pub(super) dynamic_closure_adapters: &'a mut Arena<mir::DynamicClosureAdapter>,
    pub(super) dynamic_adapter_by_target:
        &'a mut HashMap<mir::FunctionTypeId, mir::DynamicClosureAdapterId>,
    pub(super) function_bridge_targets: &'a mut Vec<mir::FunctionTypeId>,
    pub(super) suspend_sources: &'a mut Vec<SuspendSource>,
    pub(super) current_closure: Option<mir::ClosureClassId>,
    pub(super) current_closure_local: Option<mir::LocalId>,
    /// Hidden by-value parameters of a lifted local function, keyed by the
    /// global lexical binding they carry.
    pub(super) current_local_capture_params: HashMap<hir::BindingId, hir::LocalId>,
}

/// A step from a pattern subject down to a nested field.
#[derive(Clone, Copy)]
enum Access {
    Field(u32),
    EnumField { variant: u32, index: u32 },
}

/// Combine two conditions with `&&`; CFG normalization expands the short circuit.
fn and(lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    logic(smir::LogicOp::And, lhs, rhs)
}

fn logic(op: smir::LogicOp, lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Boolean,
        smir::ExprKind::ShortCircuit {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

/// `Some(statements)` unless empty (an absent else branch).
fn non_empty(statements: Vec<smir::Statement>) -> Option<Vec<smir::Statement>> {
    if statements.is_empty() {
        None
    } else {
        Some(statements)
    }
}
