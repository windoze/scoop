use super::*;

/// A function's source-facing MIR display name. Symbol construction uses the
/// independently supplied typed link stem below.
pub(super) fn fn_name(function: &hir::Function) -> String {
    // Extension receivers are structurally the first immutable HIR parameter
    // named `this`, while real members also carry `Method` metadata. Source
    // syntax cannot declare an ordinary parameter named `this`, so this is an
    // unambiguous discriminator. Keep extension symbols in a private namespace:
    // `fun f(x: Int)` and `fun Int.f()` otherwise have the same ABI parameter
    // shape and would collide despite belonging to different source layers.
    if function.method.is_none()
        && function
            .params
            .first()
            .is_some_and(|parameter| parameter.name == "this")
    {
        format!("$extension.{}", function.name)
    } else {
        function.name.clone()
    }
}

/// Native symbol base selected by HIR from typed declaration identity.  MIR
/// must not reconstruct it from `fn_name` or any source-facing label.
pub(super) fn fn_link_stem(function: &hir::Function) -> &hir::CallableLinkStem {
    &function.link_stem
}

/// The names shared by more than one plainly-mangled function (M7
/// overloads), over the whole module including scoop.core. Only
/// functions that get a plain `scoop.<name>` symbol count: `User` functions
/// with no source type arguments (free functions, class members, interface
/// method shells). Intrinsics have no MIR symbol; instantiated functions use
/// `$`-mangled symbols, which cannot collide with the
/// overload encoding (`.`).
pub(super) fn overloaded_link_stems(module: &hir::Module) -> HashSet<hir::CallableLinkStem> {
    let mut counts: HashMap<hir::CallableLinkStem, usize> = HashMap::new();
    for (_, function) in module.functions.iter() {
        if !matches!(function.kind, hir::FunctionKind::User(_))
            || function_instance(module, function).is_some()
        {
            continue;
        }
        *counts.entry(fn_link_stem(function).clone()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(stem, _)| stem)
        .collect()
}

/// `mir::mangle_instance` / `mir::encode_type` take `&mir::Module`
/// but only ever read struct / enum / class / interface names; this
/// shell provides exactly those. Its arenas share the real arenas'
/// allocation order, so ids align.
pub(super) fn mangling_shell(
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
                interior_mutable,
                ..
            } => mir::StructRepresentation::Declared {
                c_layout: *c_layout,
                interior_mutable: *interior_mutable,
                fields: Vec::new(),
            },
            mir::StructRepresentation::Intrinsic(representation) => {
                mir::StructRepresentation::Intrinsic(representation.clone())
            }
        };
        shell_structs.alloc(mir::StructDef {
            name: def.name.clone(),
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
            methods: Vec::new(),
        });
    }
    let mut functions = Arena::new();
    let entry = functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: String::new(),
        symbol: String::new(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    mir::Module {
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
        entry,
        meta: mir::MirMeta::default(),
    }
}
