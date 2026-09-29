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

fn arena_contains<T>(arena: &Arena<T>, id: la_arena::Idx<T>) -> bool {
    (id.into_raw().into_u32() as usize) < arena.len()
}
