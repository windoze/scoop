use super::*;

/// A function's source-facing MIR display name.
pub(super) fn fn_name(function: &hir::Function) -> String {
    // Keep extension display names in a private namespace:
    // `fun f(x: Int)` and `fun Int.f()` otherwise have the same ABI parameter
    // shape and would collide despite belonging to different source layers.
    if matches!(function.receiver, hir::FunctionReceiver::Extension(_)) {
        format!("$extension.{}", function.name)
    } else {
        function.name.clone()
    }
}

/// Type context maintained while the independently built MIR arenas are still
/// being populated. Its arena allocation order mirrors the output arenas, so
/// stage-local type ids remain valid when the final module is assembled.
pub(super) fn type_context(
    cone: mir::ConeIdentity,
    structs: &Arena<mir::StructDef>,
    enums: &Arena<mir::EnumDef>,
    classes: &Arena<mir::ClassDef>,
    interfaces: &Arena<mir::InterfaceDef>,
) -> mir::Module {
    let mut shell_structs = Arena::new();
    for (_, def) in structs.iter() {
        let representation = match &def.representation {
            mir::StructRepresentation::Declared {
                c_layout,
                c_abi,
                interior_mutable,
                ..
            } => mir::StructRepresentation::Declared {
                c_layout: *c_layout,
                c_abi: *c_abi,
                interior_mutable: *interior_mutable,
                fields: Vec::new(),
            },
            mir::StructRepresentation::Intrinsic(representation) => {
                mir::StructRepresentation::Intrinsic(representation.clone())
            }
        };
        shell_structs.alloc(mir::StructDef {
            name: def.name.clone(),
            type_arguments: def.type_arguments.clone(),
            gc_free: def.gc_free,
            representation,
        });
    }
    let mut shell_enums = Arena::new();
    for (_, def) in enums.iter() {
        shell_enums.alloc(mir::EnumDef {
            name: def.name.clone(),
            type_arguments: def.type_arguments.clone(),
            gc_free: def.gc_free,
            variants: Vec::new(),
        });
    }
    let mut shell_classes = Arena::new();
    for (_, def) in classes.iter() {
        let representation = match &def.representation {
            mir::ClassRepresentation::Declared { .. } => mir::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            mir::ClassRepresentation::Intrinsic(representation) => {
                mir::ClassRepresentation::Intrinsic(representation.clone())
            }
        };
        shell_classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: def.name.clone(),
            type_arguments: def.type_arguments.clone(),
            representation,
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
    }
    let mut shell_interfaces = Arena::new();
    for (_, def) in interfaces.iter() {
        shell_interfaces.alloc(mir::InterfaceDef {
            name: def.name.clone(),
            type_arguments: def.type_arguments.clone(),
            parents: Vec::new(),
            methods: Vec::new(),
        });
    }
    let mut functions = Arena::new();
    let entry = functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: String::new(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    mir::Module {
        cone,
        functions,
        extern_functions: Arena::new(),
        globals: Arena::new(),
        initialization_units: Arena::new(),
        initialization_failure_roots: Arena::new(),
        objects: Arena::new(),
        object_types: Arena::new(),
        singleton_values: Arena::new(),
        singleton_published_roots: Arena::new(),
        callback_bridges: Arena::new(),
        foreign_callback_adapters: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        function_types: Arena::new(),
        closure_classes: Arena::new(),
        closure_invoke_functions: Arena::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: shell_structs,
        enums: shell_enums,
        classes: shell_classes,
        interfaces: shell_interfaces,
        option_core: Vec::new(),
        output: mir::MirOutput::Executable { entry },
        meta: mir::MirMeta::default(),
    }
}
