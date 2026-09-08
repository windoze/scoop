use super::*;
use scoop_ast::Span;
use scoop_hir as hir;

mod basic_lowering;
mod callbacks;
mod control_flow;
mod coroutine_pending;
mod coroutines;
mod generics;
mod harness_core;
mod harness_functions;
mod harness_gc;
mod harness_nominals;
mod m23_wire;
mod operators;
mod overloads;
mod reference_types;
mod singletons;
mod value_types;

/// The mangled symbol of the (unique) MIR function with this source name.
fn symbol_of<'a>(module: &'a mir::Module, name: &str) -> &'a str {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .map(|(_, function)| function.symbol.as_str())
        .unwrap_or_else(|| panic!("no MIR function named {name:?}"))
}

/// Like [`symbol_of`], disambiguating same-named overloads by parameter
/// count (excluding the receiver of methods).
fn symbol_of_arity<'a>(module: &'a mir::Module, name: &str, params: usize) -> &'a str {
    module
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == name
                && function.params.iter().filter(|p| p.name != "this").count() == params
        })
        .map(|(_, function)| function.symbol.as_str())
        .unwrap_or_else(|| panic!("no MIR function named {name:?} with {params} params"))
}

fn lower(module: &hir::Module) -> mir::Module {
    let concrete = scoop_hir_lower::concretize_export(module);
    let mut module = super::lower(&concrete);
    // Handcrafted unit modules use hidden, valid exception shells to satisfy
    // LocalConcreteHir's complete core contract. Keep their constructor
    // functions out of unrelated top-level ordering/dump assertions.
    let functions = &module.functions;
    module.top_level.retain(|id| {
        let name = &functions[*id].name;
        !(name.starts_with("init.$") && name.contains("ExceptionProtocol.$c"))
            && !name.starts_with("init.$ThrowableProtocol.$c")
    });
    module
}

fn dump(module: &mir::Module) -> String {
    mir::dump(module)
        .lines()
        .filter(|line| {
            !(line.contains("class $") && line.contains("ExceptionProtocol"))
                && !line.contains("class $ThrowableProtocol")
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

fn visible_class_count(module: &mir::Module) -> usize {
    module
        .classes
        .iter()
        .filter(|(_, class)| {
            !class.name.ends_with("Protocol")
                && matches!(
                    class.representation,
                    mir::ClassRepresentation::Declared { .. }
                )
        })
        .count()
}

fn boxed_class<'a>(module: &'a mir::Module, name: &str) -> &'a mir::ClassDef {
    module
        .classes
        .iter()
        .map(|(_, class)| class)
        .find(|class| class.name == name)
        .unwrap_or_else(|| panic!("missing boxed class `{name}`"))
}

const SPAN: Span = Span { start: 0, end: 0 };

fn expression_origin() -> hir::ExpressionOrigin {
    hir::ExpressionOrigin::Definition(definition_origin())
}

fn definition_origin() -> hir::DefinitionOrigin {
    hir::DefinitionOrigin {
        provider: hir::IntrinsicProviderId::from_raw(0),
        file: 0,
        span: SPAN,
        context: hir::SourceContextId::from_raw(0.into()),
    }
}

fn type_param(name: impl Into<String>) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id: hir::TypeParamId::from_raw(0),
        name: name.into(),
        bounds: hir::TypeParamBounds::Unconstrained,
        span: SPAN,
    }
}

fn entry_statements(body: &mir::Body) -> &[mir::Statement] {
    &body.blocks[body.entry].statements
}

fn statement_call(statement: &mir::Statement) -> (&mir::Call, Option<mir::LocalId>) {
    let mir::StatementKind::Call(effect) = &statement.kind else {
        panic!("expected an explicit call effect")
    };
    match effect {
        mir::CallEffect::Unit(call) => (call, None),
        mir::CallEffect::Value { destination, call } => (call, Some(*destination)),
    }
}

fn block_named<'a>(body: &'a mir::Body, prefix: &str) -> &'a mir::BasicBlock {
    body.blocks
        .iter()
        .map(|(_, block)| block)
        .find(|block| block.name.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing MIR block `{prefix}`"))
}

/// HIR module shell as hir-lower produces it: well-known types, core's
/// managed output/formatting externs and ordinary `print` / `println`
/// declarations (all created on demand), plus core's `Option` enum allocated
/// first.
enum CanonicalTypePlan {
    Existing(hir::TypeId),
    Allocate,
}

struct Harness {
    types: Arena<hir::Type>,
    functions: Arena<hir::Function>,
    extern_functions: Arena<hir::ExternFunction>,
    generic_functions: Arena<hir::GenericFunction>,
    method_applications: Arena<hir::MethodApplication>,
    method_applications_by_key:
        HashMap<(hir::FunctionId, hir::MethodOwnerApplication), hir::MethodApplicationId>,
    generic_methods: Arena<hir::GenericMethod>,
    generic_method_applications: Arena<hir::GenericMethodApplication>,
    structs: Arena<hir::StructDecl>,
    struct_constructors: Arena<hir::StructConstructor>,
    struct_constructor_applications: Arena<hir::StructConstructorApplication>,
    struct_applications: Arena<hir::StructApplication>,
    struct_applications_by_key:
        HashMap<(hir::StructId, Vec<hir::TypeId>), hir::StructApplicationId>,
    enums: Arena<hir::EnumDecl>,
    enum_applications: Arena<hir::EnumApplication>,
    enum_applications_by_key: HashMap<(hir::EnumId, Vec<hir::TypeId>), hir::EnumApplicationId>,
    classes: Arena<hir::ClassDecl>,
    class_fields: Arena<hir::ClassField>,
    properties: Arena<hir::Property>,
    property_getters: Arena<hir::PropertyGetter>,
    property_setters: Arena<hir::PropertySetter>,
    class_constructors: Arena<hir::ClassConstructor>,
    class_constructor_applications: Arena<hir::ClassConstructorApplication>,
    class_applications: Arena<hir::ClassApplication>,
    class_applications_by_key: HashMap<(hir::ClassId, Vec<hir::TypeId>), hir::ClassApplicationId>,
    interfaces: Arena<hir::InterfaceDecl>,
    interface_applications: Arena<hir::InterfaceApplication>,
    interface_methods: Arena<hir::InterfaceMethod>,
    interface_applications_by_key:
        HashMap<(hir::InterfaceId, Vec<hir::TypeId>), hir::InterfaceApplicationId>,
    top_level: Vec<hir::FunctionId>,
    unit: hir::TypeId,
    integers: hir::IntegerTypeCore<hir::TypeId>,
    int: hir::TypeId,
    long: hir::TypeId,
    uint: hir::TypeId,
    ulong: hir::TypeId,
    boolean: hir::TypeId,
    string: hir::TypeId,
    option_enum: hir::EnumId,
    needs_initialization_core: bool,
    write: Option<hir::FunctionId>,
    long_to_string: Option<hir::FunctionId>,
    bool_to_string: Option<hir::FunctionId>,
    /// core's `print` / `println` overloads (ordinary functions,
    /// M7), created on first use.
    print_string: Option<hir::FunctionId>,
    print_int: Option<hir::FunctionId>,
    print_boolean: Option<hir::FunctionId>,
    println_string: Option<hir::FunctionId>,
    println_int: Option<hir::FunctionId>,
    println_boolean: Option<hir::FunctionId>,
    instantiations: Arena<hir::ResolvedGenericFunction>,
    gc_core: Option<GcCore>,
    intrinsic_array: Option<hir::ClassId>,
    intrinsic_mutable_array: Option<hir::ClassId>,
    next_constructor_param: u32,
}

/// core's GC facilities (M9), as `Harness::gc_core` declares them.
#[derive(Clone, Copy)]
struct GcCore {
    pinned_ptr: hir::StructId,
    gc_handle: hir::StructId,
    pin_raw: hir::FunctionId,
    unpin_raw: hir::FunctionId,
    get_handle_raw: hir::FunctionId,
    release_handle_raw: hir::FunctionId,
    gc_collect: hir::FunctionId,
    gc_stats: hir::FunctionId,
}

fn local(name: &str, ty: hir::TypeId) -> hir::Local {
    hir::Local {
        binding: hir::BindingId::from_raw(0),
        name: name.to_string(),
        ty,
        mutable: false,
    }
}

fn expr(kind: hir::ExprKind, ty: hir::TypeId) -> hir::Expr {
    hir::Expr {
        kind,
        ty,
        span: SPAN,
        origin: expression_origin(),
    }
}

fn stmt(kind: hir::StatementKind) -> hir::Statement {
    hir::Statement { kind, span: SPAN }
}

fn val_decl(local: hir::LocalId, init: hir::Expr) -> hir::Statement {
    stmt(hir::StatementKind::ValDecl {
        pattern: hir::Pattern::Binding { local },
        init,
    })
}

fn expr_stmt(expr: hir::Expr) -> hir::Statement {
    stmt(hir::StatementKind::Expr(expr))
}

fn int_lit(h: &Harness, value: i32) -> hir::Expr {
    expr(
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(value as u32)),
        h.int,
    )
}

fn integer_lit(h: &Harness, kind: hir::IntegerKind, raw_bits: u64) -> hir::Expr {
    let value = match kind {
        hir::IntegerKind::SIGNED_8 => hir::HirIntegerConstant::Signed8(raw_bits as u8),
        hir::IntegerKind::SIGNED_16 => hir::HirIntegerConstant::Signed16(raw_bits as u16),
        hir::IntegerKind::SIGNED_32 => hir::HirIntegerConstant::Signed32(raw_bits as u32),
        hir::IntegerKind::SIGNED_64 => hir::HirIntegerConstant::Signed64(raw_bits),
        hir::IntegerKind::UNSIGNED_8 => hir::HirIntegerConstant::Unsigned8(raw_bits as u8),
        hir::IntegerKind::UNSIGNED_16 => hir::HirIntegerConstant::Unsigned16(raw_bits as u16),
        hir::IntegerKind::UNSIGNED_32 => hir::HirIntegerConstant::Unsigned32(raw_bits as u32),
        hir::IntegerKind::UNSIGNED_64 => hir::HirIntegerConstant::Unsigned64(raw_bits),
    };
    assert_eq!(value.raw_bits(), raw_bits & kind.width().raw_mask());
    expr(hir::ExprKind::IntegerLiteral(value), h.integer(kind))
}

fn bool_lit(h: &Harness, value: bool) -> hir::Expr {
    expr(hir::ExprKind::BoolLiteral(value), h.boolean)
}

fn str_lit(h: &Harness, value: &str) -> hir::Expr {
    expr(hir::ExprKind::StringLiteral(value.to_string()), h.string)
}

fn local_ref(id: hir::LocalId, ty: hir::TypeId) -> hir::Expr {
    expr(hir::ExprKind::Local(id), ty)
}

fn binary(op: hir::BinOp, lhs: hir::Expr, rhs: hir::Expr, ty: hir::TypeId) -> hir::Expr {
    expr(
        hir::ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty,
    )
}

fn primitive_binary(
    kind: hir::PrimitiveBinaryKind,
    lhs: hir::Expr,
    rhs: hir::Expr,
    ty: hir::TypeId,
) -> hir::Expr {
    expr(
        hir::ExprKind::PrimitiveBinary {
            kind,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty,
    )
}

fn primitive_unary(
    kind: hir::PrimitiveUnaryKind,
    operand: hir::Expr,
    ty: hir::TypeId,
) -> hir::Expr {
    expr(
        hir::ExprKind::PrimitiveUnary {
            kind,
            operand: Box::new(operand),
        },
        ty,
    )
}

fn integer_operation(
    h: &mut Harness,
    kind: hir::IntegerKind,
    operation: hir::NoGcIntegerOperation,
    arguments: hir::HirIntegerOperationArguments,
) -> hir::Expr {
    let owner = h.integer(kind);
    let result_ty = match operation {
        hir::NoGcIntegerOperation::CompareTo => h.long,
        hir::NoGcIntegerOperation::Equals => h.boolean,
        _ => owner,
    };
    let function = h.functions.alloc(hir::Function {
        origin: hir::DeclarationOrigin {
            provider: hir::IntrinsicProviderId::from_raw(0),
            file: 0,
        },
        name: format!("$testIntegerNoGc{}", h.functions.len()),
        access: hir::DeclarationAccess::public(),
        override_access: Vec::new(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: false,
        modifiers: hir::CallableModifiers::default(),
        params: Vec::new(),
        return_ty: result_ty,
        attributes: hir::FunctionAttributes {
            gc_effect: hir::GcEffect::NoGc,
            ..hir::FunctionAttributes::default()
        },
        kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
                kind,
                operation,
            }),
            provider: hir::IntrinsicProviderId::from_raw(0),
        }),
        method: Some(hir::Method {
            owner,
            modifier: hir::MethodModifier::Final,
            dispatch: hir::MethodDispatch::Direct,
        }),
        span: SPAN,
    });
    let target = hir::NoGcCallableRef::try_from_function(function, &h.functions)
        .expect("the test target carries the matching no-GC integer intrinsic effect");
    expr(
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind,
                operation,
                target,
            },
            arguments,
        },
        result_ty,
    )
}

fn integer_binary(
    h: &mut Harness,
    kind: hir::IntegerKind,
    operation: hir::NoGcIntegerOperation,
    lhs: hir::Expr,
    rhs: hir::Expr,
) -> hir::Expr {
    integer_operation(
        h,
        kind,
        operation,
        hir::HirIntegerOperationArguments::Binary {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

fn integer_unary(
    h: &mut Harness,
    kind: hir::IntegerKind,
    operation: hir::NoGcIntegerOperation,
    operand: hir::Expr,
) -> hir::Expr {
    integer_operation(
        h,
        kind,
        operation,
        hir::HirIntegerOperationArguments::Unary(Box::new(operand)),
    )
}

fn integer_div_rem(
    h: &mut Harness,
    kind: hir::IntegerKind,
    operation: hir::IntegerDivRem,
    lhs: hir::Expr,
    rhs: hir::Expr,
) -> hir::Expr {
    let owner = h.integer(kind);
    let function = h.functions.alloc(hir::Function {
        origin: hir::DeclarationOrigin {
            provider: hir::IntrinsicProviderId::from_raw(0),
            file: 0,
        },
        name: format!("$testIntegerManaged{}", h.functions.len()),
        access: hir::DeclarationAccess::public(),
        override_access: Vec::new(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: false,
        modifiers: hir::CallableModifiers::default(),
        params: Vec::new(),
        return_ty: owner,
        attributes: hir::FunctionAttributes::default(),
        kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::Integer(
                hir::IntegerIntrinsicKind::ManagedOperation { kind, operation },
            ),
            provider: hir::IntrinsicProviderId::from_raw(0),
        }),
        method: Some(hir::Method {
            owner,
            modifier: hir::MethodModifier::Final,
            dispatch: hir::MethodDispatch::Direct,
        }),
        span: SPAN,
    });
    let target = hir::ManagedCallableRef::try_from_function(function, &h.functions)
        .expect("the test target carries the matching managed integer intrinsic effect");
    expr(
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::Managed {
                kind,
                operation,
                target,
            },
            arguments: hir::HirIntegerOperationArguments::Binary {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        },
        owner,
    )
}

fn integer_conversion(
    h: &mut Harness,
    source: hir::IntegerKind,
    target_kind: hir::IntegerKind,
    operand: hir::Expr,
) -> hir::Expr {
    let owner = h.integer(source);
    let result_ty = h.integer(target_kind);
    let function = h.functions.alloc(hir::Function {
        origin: hir::DeclarationOrigin {
            provider: hir::IntrinsicProviderId::from_raw(0),
            file: 0,
        },
        name: format!("$testIntegerConversion{}", h.functions.len()),
        access: hir::DeclarationAccess::public(),
        override_access: Vec::new(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: false,
        modifiers: hir::CallableModifiers::default(),
        params: Vec::new(),
        return_ty: result_ty,
        attributes: hir::FunctionAttributes {
            gc_effect: hir::GcEffect::NoGc,
            ..hir::FunctionAttributes::default()
        },
        kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::Conversion {
                source,
                target_kind,
            }),
            provider: hir::IntrinsicProviderId::from_raw(0),
        }),
        method: Some(hir::Method {
            owner,
            modifier: hir::MethodModifier::Final,
            dispatch: hir::MethodDispatch::Direct,
        }),
        span: SPAN,
    });
    let target = hir::NoGcCallableRef::try_from_function(function, &h.functions)
        .expect("the test target carries a no-GC integer conversion effect");
    expr(
        hir::ExprKind::IntegerConversion {
            conversion: hir::IntegerConversion {
                source,
                target_kind,
                target,
            },
            operand: Box::new(operand),
        },
        result_ty,
    )
}

fn module_integer_type(module: &hir::Module, kind: hir::IntegerKind) -> hir::TypeId {
    module
        .types
        .iter()
        .find_map(|(id, ty)| {
            matches!(ty, hir::Type::Integer(found) if *found == kind).then_some(id)
        })
        .expect("the test module contains every canonical integer type")
}

fn call(h: &Harness, function: hir::FunctionId, args: Vec<hir::Expr>) -> hir::Expr {
    expr(
        hir::ExprKind::Call {
            callee: hir::Callable::Function(function),
            args,
        },
        h.unit,
    )
}

/// A call expression with an explicit result type (hir-lower
/// annotates every expression; core's functions call String-returning
/// formatting externs).
fn call_typed(function: hir::FunctionId, args: Vec<hir::Expr>, ty: hir::TypeId) -> hir::Expr {
    expr(
        hir::ExprKind::Call {
            callee: hir::Callable::Function(function),
            args,
        },
        ty,
    )
}

fn struct_init(h: &Harness, ty: hir::TypeId, args: Vec<hir::Expr>) -> hir::Expr {
    let hir::Type::Struct(application) = h.types[ty] else {
        panic!("struct construction requires a struct application type")
    };
    let constructor = h.structs[h.struct_applications[application].template].constructors[0];
    let application = h
        .struct_constructor_applications
        .iter()
        .find_map(|(id, candidate)| {
            (candidate.constructor == constructor && candidate.owner == application).then_some(id)
        })
        .expect("the harness creates the primary struct constructor application");
    expr(
        hir::ExprKind::StructInit {
            constructor: application,
            args,
        },
        ty,
    )
}

fn module_interface_application(
    module: &mut hir::Module,
    template: hir::InterfaceId,
    arguments: Vec<hir::TypeId>,
) -> hir::TypeId {
    let canonical_type = hir::TypeId::from_raw((module.types.len() as u32).into());
    let application = module
        .interface_applications
        .alloc(hir::InterfaceApplication {
            template,
            arguments,
            canonical_type,
        });
    let actual_type = module.types.alloc(hir::Type::Interface(application));
    assert_eq!(actual_type, canonical_type);
    canonical_type
}

/// `main` calls `println("hello, world")` then `helper()`, which
/// calls `print("!")`.
fn hello_world() -> hir::Module {
    let mut h = Harness::new();
    let print = h.print_string();
    let println = h.println_string();
    let helper = h.user_fn(
        "helper",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(call(&h, print, vec![str_lit(&h, "!")]))],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call(&h, println, vec![str_lit(&h, "hello, world")])),
                expr_stmt(call(&h, helper, vec![])),
            ],
        },
    );
    h.finish(main)
}

fn generic_call(
    resolved: hir::ResolvedGenericFunctionId,
    args: Vec<hir::Expr>,
    ty: hir::TypeId,
) -> hir::Expr {
    expr(
        hir::ExprKind::Call {
            callee: hir::Callable::Generic(resolved),
            args,
        },
        ty,
    )
}

fn param(name: &str, ty: hir::TypeId, local: hir::LocalId) -> hir::Param {
    hir::Param {
        name: name.to_string(),
        ty,
        local,
    }
}

/// `fun <T> name(x: T): T { return x }`.
fn identity_fn(h: &mut Harness, name: &str) -> hir::FunctionId {
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", t));
    h.user_fn_full(
        name,
        vec!["T".to_string()],
        vec![param("x", t, x)],
        t,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(x, t)),
            })],
        },
    )
}

fn instance_id(module: &mir::Module, function: mir::FunctionId) -> mir::MonomorphizedFunctionId {
    module
        .meta
        .instances
        .iter()
        .find_map(|(id, instance)| (instance.function == function).then_some(id))
        .expect("function must have monomorphization metadata")
}

fn empty_main(h: &mut Harness) -> hir::FunctionId {
    h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    )
}

fn class_index(raw: u32) -> mir::ClassId {
    la_arena::Idx::from_raw(raw.into())
}

// ---- M6: reference types ----

/// The symbol a vtable / itable slot points at.
fn slot_fn<'a>(module: &'a mir::Module, slot: &mir::TableSlot) -> &'a str {
    match slot {
        mir::TableSlot::Function(id) => &module.functions[*id].symbol,
        mir::TableSlot::Runtime(function) => function.symbol(),
    }
}
