use super::*;

impl Lowerer {
    /// Establish one lexical source boundary for expression provenance.
    /// Contexts are arena-backed so origins stay Copy while function/type
    /// names remain typed HIR data rather than duplicated strings.
    pub(crate) fn set_source_context(&mut self, function_name: impl Into<String>) {
        let type_name = match self.current_owner {
            Some(Owner::Class(id)) => self.classes[id].name.clone(),
            Some(Owner::Interface(id)) => self.interfaces[id].name.clone(),
            Some(Owner::Struct(id)) => self.structs[id].name.clone(),
            Some(Owner::Enum(id)) => self.enums[id].name.clone(),
            Some(Owner::Object(id)) => self.objects[id].name.clone(),
            None => String::new(),
        };
        self.current_source_context = self.source_contexts.alloc(hir::SourceContext {
            function_name: function_name.into(),
            type_name,
        });
    }

    /// Allocate a hidden desugaring temporary (`$opt.N` / `$res.N`).
    /// The `$` prefix keeps it out of the source namespace (the parser
    /// never produces `$` identifiers), so it is not registered in
    /// `scopes`; generated code references it by `LocalId` directly.
    pub(crate) fn alloc_hidden(&mut self, prefix: &str, ty: TypeId) -> hir::LocalId {
        let name = format!("${prefix}.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_local(name, ty, false)
    }

    /// Allocate the branch-result local used when a structured control
    /// expression is lowered into statements. Every normally completing
    /// branch assigns it before the resulting local read is reachable.
    pub(crate) fn alloc_hidden_result(&mut self, ty: TypeId) -> hir::LocalId {
        let name = format!("$result.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_local(name, ty, true)
    }

    /// The `Option<T>` enum of `scoop.core` and the variant index of
    /// `name`, when `name` is one of its variants. These names
    /// (`Some` / `None`) are the globally visible constructors the core
    /// library's default import provides (spec 7.2).
    pub(crate) fn option_variant(&self, name: &str) -> Option<(EnumId, u32)> {
        let id = self.option_enum?;
        let index = self.enums[id]
            .variants
            .iter()
            .position(|v| v.name == name)?;
        Some((id, index as u32))
    }

    /// The variant index of `name` in `enum_id`, if it exists.
    pub(crate) fn find_variant(&self, enum_id: EnumId, name: &str) -> Option<u32> {
        self.enums[enum_id]
            .variants
            .iter()
            .position(|v| v.name == name)
            .map(|index| index as u32)
    }

    /// Whether `ty` is `Option<T>`; returns `T`.
    pub(crate) fn as_option(&self, ty: TypeId) -> Option<TypeId> {
        match &self.types[ty] {
            Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                (Some(application.template) == self.option_enum && application.arguments.len() == 1)
                    .then_some(application.arguments[0])
            }
            _ => None,
        }
    }

    /// `Option<inner>` (interned). Only called when the core `Option`
    /// validated successfully.
    pub(crate) fn option_type(&mut self, inner: TypeId) -> TypeId {
        let id = self
            .option_enum
            .expect("Option types only exist after core validation");
        self.enum_application(id, vec![inner])
    }

    pub(crate) fn push_suspension_context(&mut self, context: SuspensionContext) {
        self.suspension_contexts.push(context);
    }

    pub(crate) fn pop_suspension_context(&mut self) {
        assert!(
            self.suspension_contexts.len() > 1,
            "the root suspension context must remain present"
        );
        self.suspension_contexts.pop();
    }

    pub(crate) fn push_safety_context(&mut self, safety: hir::Safety) {
        self.safety_contexts.push(safety);
    }

    pub(crate) fn pop_safety_context(&mut self) {
        assert!(
            self.safety_contexts.len() > 1,
            "the root safety context must remain present"
        );
        self.safety_contexts.pop();
    }

    /// Diagnose a suspend call made from a declaration body whose ABI has
    /// no continuation. The resolved callable, including a generic
    /// instantiation, always leads back to exactly one function entity.
    pub(crate) fn check_suspend_call(&mut self, callable: hir::Callable, span: Span) {
        let function = self.callable_function_id(callable);
        if !self.functions[function].is_suspend {
            return;
        }
        let context = *self
            .suspension_contexts
            .last()
            .expect("the suspension context stack is initialized non-empty");
        let SuspensionContext::Forbidden(reason) = context else {
            return;
        };
        let callee = self.functions[function].name.clone();
        let location = match reason {
            ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
            ForbiddenSuspendContext::Function => {
                format!("non-suspend function `{}`", self.current_fn_name)
            }
            ForbiddenSuspendContext::DefaultExpression => {
                format!("non-suspend default expression {}", self.current_fn_name)
            }
            ForbiddenSuspendContext::ConstructorDelegation => "constructor delegation".to_string(),
            ForbiddenSuspendContext::ConstructorInitialization => {
                "constructor initialization".to_string()
            }
        };
        self.error(
            span,
            format!("suspend function `{callee}` cannot be called from {location}"),
        );
    }

    pub(crate) fn check_call_effects(&mut self, callable: hir::Callable, span: Span) {
        self.check_suspend_call(callable, span);
        let function = self.callable_function_id(callable);
        if self.functions[function].attributes.safety != hir::Safety::Unsafe {
            return;
        }
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Safe {
            self.error(
                span,
                format!(
                    "unsafe function `{}` may only be called from an unsafe context",
                    self.functions[function].name
                ),
            );
        }
    }

    pub(crate) fn require_unsafe_operation(&mut self, span: Span, operation: &str) -> bool {
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Unsafe {
            return true;
        }
        self.error(span, format!("{operation} requires an unsafe context"));
        false
    }

    pub(crate) fn error(&mut self, span: Span, message: String) {
        let mut diagnostic = Diagnostic::at(span, message);
        diagnostic.file = self.current_file;
        self.diagnostics.push(diagnostic);
    }
}
