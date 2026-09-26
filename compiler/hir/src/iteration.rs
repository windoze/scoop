use super::*;

/// Complete compiler-owned identity of the source iteration protocol.
///
/// The core `Option` relation remains the module's single canonical
/// `OptionCore`; this value owns only the identities introduced by the
/// iteration protocol itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationCore {
    iterator: InterfaceId,
    next: InterfaceMethodId,
}

impl IterationCore {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        interfaces: &Arena<InterfaceDecl>,
        interface_applications: &Arena<InterfaceApplication>,
        interface_methods: &Arena<InterfaceMethod>,
        functions: &Arena<Function>,
        enums: &Arena<EnumDecl>,
        enum_applications: &Arena<EnumApplication>,
        types: &Arena<Type>,
        option: OptionCore,
        iterator: InterfaceId,
        next: InterfaceMethodId,
    ) -> Option<Self> {
        if !arena_contains(interfaces, iterator) || !arena_contains(interface_methods, next) {
            return None;
        }
        let interface = &interfaces[iterator];
        let method = &interface_methods[next];
        if !arena_contains(functions, method.function)
            || !arena_contains(interface_applications, interface.self_application)
        {
            return None;
        }
        let function = &functions[method.function];
        let checked_option =
            OptionCore::checked(enums, types, option.some_payload(), option.none())?;
        if checked_option != option
            || interface.name != "Iterator"
            || interface.owner.is_some()
            || interface.access.declared != DeclaredVisibility::Public
            || !interface.gc_free_pointee_requirements.is_empty()
            || !interface.parents.is_empty()
            || !interface.private_methods.is_empty()
            || !interface.properties.is_empty()
            || interface.methods.as_slice() != [next]
            || method.owner != iterator
            || method.role != InterfaceMemberRole::Function
            || method.implementation != InterfaceMemberImplementation::AbstractSlot
            || !method.overrides.is_empty()
            || function.name.rsplit('.').next() != Some("next")
            || function.access.declared != DeclaredVisibility::Public
            || function.is_suspend
            || function.modifiers != CallableModifiers::default()
            || function.attributes != FunctionAttributes::default()
            || !function.override_access.is_empty()
            || function.params.len() != 1
        {
            return None;
        }

        let [parameter] = interface.type_params.as_slice() else {
            return None;
        };
        if parameter.bounds != TypeParamBounds::Unconstrained {
            return None;
        }
        let FunctionGenericity::OwnerParameterizedMethod {
            owner_parameters,
            no_gc_type_params,
            gc_free_pointee_requirements,
        } = &function.genericity
        else {
            return None;
        };
        if owner_parameters.as_slice() != interface.type_params.as_slice()
            || !no_gc_type_params.is_empty()
            || !gc_free_pointee_requirements.is_empty()
        {
            return None;
        }

        let self_application = &interface_applications[interface.self_application];
        if self_application.template != iterator
            || self_application.arguments.len() != 1
            || !arena_contains(types, self_application.arguments[0])
            || !arena_contains(types, self_application.canonical_type)
            || !matches!(types[self_application.arguments[0]], Type::Param(found) if found == parameter.id)
            || !matches!(types[self_application.canonical_type], Type::Interface(found) if found == interface.self_application)
        {
            return None;
        }

        let function_method = function.method?;
        if function_method.owner != self_application.canonical_type
            || function_method.modifier != MethodModifier::Abstract
            || function_method.dispatch != MethodDispatch::Interface(next)
        {
            return None;
        }
        let receiver = &function.params[0];
        if receiver.name != "this"
            || receiver.ty != self_application.canonical_type
            || !function_has_exact_receiver_body(function, receiver)
        {
            return None;
        }

        if !arena_contains(types, function.return_ty) {
            return None;
        }
        let Type::Enum(option_application_id) = types[function.return_ty] else {
            return None;
        };
        if !arena_contains(enum_applications, option_application_id) {
            return None;
        }
        let option_application = &enum_applications[option_application_id];
        if option_application.template != option.enumeration()
            || option_application.arguments.as_slice() != [self_application.arguments[0]]
            || option_application.canonical_type != function.return_ty
        {
            return None;
        }

        Some(Self { iterator, next })
    }

    pub const fn iterator(self) -> InterfaceId {
        self.iterator
    }

    pub const fn next(self) -> InterfaceMethodId {
        self.next
    }
}

fn function_has_exact_receiver_body(function: &Function, receiver: &Param) -> bool {
    let FunctionKind::User(body) = &function.kind else {
        return false;
    };
    if body.locals.len() != 1
        || receiver.local.into_raw().into_u32() as usize >= body.locals.len()
        || !body.statements.is_empty()
    {
        return false;
    }
    let local = &body.locals[receiver.local];
    local.name == receiver.name && local.ty == receiver.ty && !local.mutable
}

/// Exact specialization of the canonical `Some(E)` / `None` relation used
/// by one `for` plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedOptionCore {
    some_payload: AppliedEnumVariantFieldRef,
    none: AppliedEnumVariantRef,
}

impl AppliedOptionCore {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        types: &Arena<Type>,
        option: OptionCore,
        element: TypeId,
        some_payload: AppliedEnumVariantFieldRef,
        none: AppliedEnumVariantRef,
    ) -> Option<Self> {
        if !arena_contains(types, element)
            || AppliedEnumVariantFieldRef::checked(
                enums,
                applications,
                some_payload.variant(),
                some_payload.local_index(),
            ) != Some(some_payload)
            || AppliedEnumVariantRef::checked(
                enums,
                applications,
                none.application(),
                none.declaration(),
            ) != Some(none)
            || some_payload.variant().application() != none.application()
        {
            return None;
        }
        let application = &applications[some_payload.variant().application()];
        if application.template != option.enumeration()
            || application.arguments.as_slice() != [element]
            || !arena_contains(types, application.canonical_type)
            || !matches!(types[application.canonical_type], Type::Enum(found) if found == some_payload.variant().application())
            || some_payload.variant().declaration() != option.some()
            || some_payload.local_index() != option.some_payload().local_index()
            || none.declaration() != option.none()
        {
            return None;
        }
        Some(Self { some_payload, none })
    }

    pub const fn application(self) -> EnumApplicationId {
        self.some_payload.variant().application()
    }

    pub const fn some_payload(self) -> AppliedEnumVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> AppliedEnumVariantRef {
        self.none
    }
}

/// The selected iterator value is adapted only after generic substitution.
/// This is essential for a type parameter that can instantiate as either a
/// value (box) or reference (zero-cost interface retype).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IteratorConformanceWitness {
    source: BindingTemporary,
    iterator: BindingTemporary,
    application: InterfaceApplicationId,
    span: Span,
    origin: ExpressionOrigin,
}

impl IteratorConformanceWitness {
    pub const fn new(
        source: BindingTemporary,
        iterator: BindingTemporary,
        application: InterfaceApplicationId,
        span: Span,
        origin: ExpressionOrigin,
    ) -> Self {
        Self {
            source,
            iterator,
            application,
            span,
            origin,
        }
    }

    pub const fn source(self) -> BindingTemporary {
        self.source
    }

    pub const fn iterator(self) -> BindingTemporary {
        self.iterator
    }

    pub const fn application(self) -> InterfaceApplicationId {
        self.application
    }

    pub const fn span(self) -> Span {
        self.span
    }

    pub const fn origin(self) -> ExpressionOrigin {
        self.origin
    }
}

/// Exact per-use specialization of `Iterator<E>.next(): Option<E>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IteratorNextPlan {
    callable: MethodApplicationId,
    result: BindingTemporary,
    option: AppliedOptionCore,
    element: BindingTemporary,
    span: Span,
    origin: ExpressionOrigin,
}

impl IteratorNextPlan {
    pub const fn new(
        callable: MethodApplicationId,
        result: BindingTemporary,
        option: AppliedOptionCore,
        element: BindingTemporary,
        span: Span,
        origin: ExpressionOrigin,
    ) -> Self {
        Self {
            callable,
            result,
            option,
            element,
            span,
            origin,
        }
    }

    pub const fn callable(self) -> MethodApplicationId {
        self.callable
    }

    pub const fn result(self) -> BindingTemporary {
        self.result
    }

    pub const fn option(self) -> AppliedOptionCore {
        self.option
    }

    pub const fn element(self) -> BindingTemporary {
        self.element
    }

    pub const fn span(self) -> Span {
        self.span
    }

    pub const fn origin(self) -> ExpressionOrigin {
        self.origin
    }
}

/// Complete Export-HIR plan for one source `for` statement.
///
/// A module boundary validator checks this entire relation before any reader
/// may concretize it. LocalConcrete HIR never contains this source node.
#[derive(Debug, Clone)]
pub struct ForIterationPlan {
    target: LoopId,
    source_setup: Vec<Statement>,
    source: BindingTemporary,
    source_init: Expr,
    iterator_setup: Vec<Statement>,
    iterator_call: Expr,
    conformance: IteratorConformanceWitness,
    next: IteratorNextPlan,
    binding: IrrefutableBindingPlan,
    body: Vec<Statement>,
}

impl ForIterationPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: LoopId,
        source_setup: Vec<Statement>,
        source: BindingTemporary,
        source_init: Expr,
        iterator_setup: Vec<Statement>,
        iterator_call: Expr,
        conformance: IteratorConformanceWitness,
        next: IteratorNextPlan,
        binding: IrrefutableBindingPlan,
        body: Vec<Statement>,
    ) -> Self {
        Self {
            target,
            source_setup,
            source,
            source_init,
            iterator_setup,
            iterator_call,
            conformance,
            next,
            binding,
            body,
        }
    }

    pub const fn target(&self) -> LoopId {
        self.target
    }

    pub fn source_setup(&self) -> &[Statement] {
        &self.source_setup
    }

    pub const fn source(&self) -> BindingTemporary {
        self.source
    }

    pub const fn source_init(&self) -> &Expr {
        &self.source_init
    }

    pub fn iterator_setup(&self) -> &[Statement] {
        &self.iterator_setup
    }

    pub const fn iterator_call(&self) -> &Expr {
        &self.iterator_call
    }

    pub const fn conformance(&self) -> IteratorConformanceWitness {
        self.conformance
    }

    pub const fn next(&self) -> IteratorNextPlan {
        self.next
    }

    pub const fn binding(&self) -> &IrrefutableBindingPlan {
        &self.binding
    }

    pub fn body(&self) -> &[Statement] {
        &self.body
    }

    pub fn into_parts(self) -> ForIterationPlanParts {
        ForIterationPlanParts {
            target: self.target,
            source_setup: self.source_setup,
            source: self.source,
            source_init: self.source_init,
            iterator_setup: self.iterator_setup,
            iterator_call: self.iterator_call,
            conformance: self.conformance,
            next: self.next,
            binding: self.binding,
            body: self.body,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForIterationPlanParts {
    pub target: LoopId,
    pub source_setup: Vec<Statement>,
    pub source: BindingTemporary,
    pub source_init: Expr,
    pub iterator_setup: Vec<Statement>,
    pub iterator_call: Expr,
    pub conformance: IteratorConformanceWitness,
    pub next: IteratorNextPlan,
    pub binding: IrrefutableBindingPlan,
    pub body: Vec<Statement>,
}

fn arena_contains<T>(arena: &Arena<T>, id: la_arena::Idx<T>) -> bool {
    (id.into_raw().into_u32() as usize) < arena.len()
}
