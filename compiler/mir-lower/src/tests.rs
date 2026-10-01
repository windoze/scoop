use super::*;
use scoop_hir as hir;
use scoop_hir::Span;

mod basic_lowering;
mod callbacks;
mod control_flow;
mod coroutine_pending;
mod coroutines;
mod function_adapters;
mod generics;
mod harness_core;
mod harness_functions;
mod harness_gc;
mod harness_metadata;
mod harness_nominals;
mod operators;
mod overloads;
mod reference_types;
mod singletons;
mod snapshots;
mod source_exact_types;
use snapshots::check_mir_snapshot;
mod value_types;

trait TestExecutableEntry {
    type FunctionId: Copy;

    fn entry(&self) -> Self::FunctionId;
}

impl TestExecutableEntry for hir::ExportHirOutput {
    type FunctionId = hir::FunctionId;

    fn entry(&self) -> Self::FunctionId {
        let hir::ConeOutputKind::Executable { local_entry } = self.output_kind() else {
            panic!("test expected executable Export HIR")
        };
        local_entry.local_function().function()
    }
}

impl TestExecutableEntry for hir::LocalConcreteHirOutput {
    type FunctionId = hir::concrete::FunctionId;

    fn entry(&self) -> Self::FunctionId {
        let hir::LocalConeOutputKind::Executable { local_entry } = self.output_kind() else {
            panic!("test expected executable LocalConcrete HIR")
        };
        local_entry.local_function().function()
    }
}

fn lower(module: &hir::ExportHirOutput) -> mir::Module {
    let concrete =
        scoop_hir_lower::concretize_output(module).expect("concrete type applications are valid");
    let output_kind = concrete.output_kind().clone();
    let materialization = concrete.materialization().clone();
    let mut concrete_module = concrete.into_module();
    if concrete_module.initialization_units.is_empty() {
        let cycle_thrower = defined_concrete_core(&concrete_module)
            .exceptions
            .initialization_cycle_thrower;
        concrete_module
            .top_level
            .retain(|&function| function != cycle_thrower);
    }
    let concrete =
        hir::LocalConcreteHirOutput::try_new(concrete_module, output_kind, materialization)
            .expect("the filtered MIR fixture remains a valid LocalConcrete HIR output");
    let mut module = super::lower(&concrete)
        .expect("test LocalConcrete HIR carries locally defined core protocols");
    // Handcrafted unit modules use hidden, valid exception shells to satisfy
    // LocalConcreteHir's complete core contract. Keep their constructor
    // functions out of unrelated top-level ordering/dump assertions.
    let functions = &module.functions;
    module.top_level.retain(|id| {
        let name = &functions[*id].name;
        !(name.starts_with("init._") && name.contains("ExceptionProtocol.$c"))
            && !name.starts_with("init._ThrowableProtocol.$c")
    });
    module
}

fn defined_export_core(module: &hir::Module) -> &hir::DefinedCoreProtocols {
    let hir::CoreProtocols::Defined(protocols) = &module.core_protocols else {
        panic!("test Export HIR carries locally defined core protocols")
    };
    protocols
}

fn defined_concrete_core(
    module: &hir::concrete::Module,
) -> &hir::concrete::DefinedConcreteCoreProtocols {
    let hir::concrete::ConcreteCoreProtocols::Defined(protocols) = &module.core_protocols else {
        panic!("test LocalConcrete HIR carries locally defined core protocols")
    };
    protocols
}

fn assert_mir_foundation_projection(module: &mir::Module) -> mir::MirFoundationCounts {
    let foundation = mir::CanonicalMirFoundation::from_module(module)
        .expect("valid lowered MIR has a complete canonical identity foundation");
    let counts = foundation.counts();
    assert_eq!(counts.exact_types, module.meta.generated_exact_types.len());
    assert_eq!(
        counts.generated_callables,
        module.meta.generated_callables.len()
    );
    assert_eq!(
        counts.generated_types,
        module.meta.generated_exact_types.len()
    );
    assert_eq!(
        counts.callable_signatures,
        module.meta.callable_signatures.len()
    );
    assert_eq!(
        counts.callback_applications,
        module.foreign_callback_bridges.len()
    );
    assert_eq!(
        counts.callback_application_records,
        module.foreign_callback_bridges.len()
    );
    let mir_local_values = module
        .meta
        .local_values
        .iter()
        .filter(|entry| entry.authority() == mir::LocalValueIdentityAuthority::Mir)
        .map(|entry| entry.identity_record().id())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert_eq!(counts.local_values, mir_local_values);
    counts
}

fn executable_output(module: hir::Module, entry: hir::FunctionId) -> hir::ExportHirOutput {
    let local_entry = hir::LocalExecutableEntry::try_new(&module, entry)
        .expect("the MIR test keeps its executable entry structurally valid");
    hir::ExportHirOutput::try_new(
        module,
        hir::ConeOutputKind::Executable {
            local_entry: Box::new(local_entry),
        },
    )
    .expect("the MIR test keeps its executable output structurally valid")
}

fn rebuild_type_identities(module: &hir::Module) -> hir::HirTypeIdentities {
    hir::HirTypeIdentities::from_types(hir::HirTypeIdentityInputs {
        types: &module.types,
        function_types: &module.function_types,
        structs: &module.structs,
        struct_applications: &module.struct_applications,
        enums: &module.enums,
        loaded_enum_definitions: &module.loaded_enum_definitions,
        loaded_struct_definitions: &module.loaded_struct_definitions,
        loaded_class_definitions: &module.loaded_class_definitions,
        loaded_interface_definitions: &module.loaded_interface_definitions,
        enum_applications: &module.enum_applications,
        classes: &module.classes,
        class_applications: &module.class_applications,
        interfaces: &module.interfaces,
        interface_applications: &module.interface_applications,
        objects: &module.objects,
        core_types: hir::HirCoreTypeIdentityAuthority::Defined(
            &defined_export_core(module).fundamental_types,
        ),
        nominal_identities: &module.nominal_identities,
    })
    .expect("the MIR test keeps its HIR type identities structurally complete")
}

fn rebuild_initialization_unit_identities(
    module: &hir::Module,
) -> hir::HirInitializationUnitIdentities {
    hir::HirInitializationUnitIdentities::from_declarations(
        &module.initialization_units,
        &module.initialization_failure_roots,
        &module.functions,
        &module.globals,
        &module.objects,
        &module.companion_relations,
        &module.singleton_values,
        &module.singleton_published_roots,
        &module.properties,
        &module.delegate_storages,
        &module.generic_delegate_templates,
        &module.nominal_identities,
        &module.property_identities,
    )
    .expect("the MIR test keeps its initialization-unit identities complete")
}

fn extend_function_identities(module: &mut hir::Module, preserved_functions: usize) {
    let preserved = module.function_identities.clone();
    let identities = module
        .functions
        .iter()
        .map(|(function, declaration)| {
            let index = function.into_raw().into_u32() as usize;
            if index < preserved_functions {
                return preserved[function].clone();
            }
            assert!(matches!(
                declaration.genericity,
                hir::FunctionGenericity::Plain
            ));
            let site = scoop_identity::SourceDeclarationSite::new(
                scoop_identity::ConeIdentity::SINGLE_FILE,
                scoop_identity::PackagePath::root(),
                scoop_identity::DefinitionOwnerChain::top_level(),
                scoop_identity::DeclarationScope::ConeWide,
            )
            .unwrap();
            let name = format!("extended_fixture_function_{index}");
            let declaration = scoop_identity::SourceDeclarationKey::function(
                site,
                scoop_identity::CanonicalIdentifier::new(&name).unwrap(),
                0,
                None,
                Vec::new(),
            );
            hir::HirFunctionIdentity::source(
                hir::HirSourceFunctionIdentity::from_declaration(declaration).unwrap(),
            )
        })
        .collect();
    module.function_identities = hir::HirFunctionIdentities::checked(
        hir::HirFunctionIdentityInputs {
            functions: &module.functions,
            lambdas: &module.lambdas,
            anonymous_functions: &module.anonymous_functions,
            local_functions: &module.local_functions,
            property_getters: &module.property_getters,
            property_setters: &module.property_setters,
            property_accessor_identities: &module.property_accessor_identities,
            initialization_units: &module.initialization_units,
            initialization_unit_identities: &module.initialization_unit_identities,
            derived_equality_applications: &module.derived_equality_applications,
            structs: &module.structs,
            enums: &module.enums,
            type_identities: &module.type_identities,
            struct_constructors: &module.struct_constructors,
            class_constructors: &module.class_constructors,
            constructor_identities: &module.constructor_identities,
            enum_member_identities: &module.enum_member_identities,
        },
        identities,
    )
    .expect("the extended MIR test fixture has a total function identity relation");
}

fn rebuild_callback_identities(module: &hir::Module) -> hir::HirCallbackRegistrationIdentities {
    hir::HirCallbackRegistrationIdentities::from_registrations(
        hir::HirCallbackRegistrationIdentityInputs {
            registrations: &module.foreign_callback_registrations,
            functions: &module.functions,
            lambdas: &module.lambdas,
            anonymous_functions: &module.anonymous_functions,
            local_functions: &module.local_functions,
            class_constructors: &module.class_constructors,
            struct_constructors: &module.struct_constructors,
            function_identities: &module.function_identities,
            property_accessor_identities: &module.property_accessor_identities,
            constructor_identities: &module.constructor_identities,
            enum_member_identities: &module.enum_member_identities,
            callback_modes: defined_export_core(module).foreign_callbacks.modes,
            type_inputs: hir::HirTypeIdentityInputs {
                types: &module.types,
                function_types: &module.function_types,
                structs: &module.structs,
                struct_applications: &module.struct_applications,
                enums: &module.enums,
                loaded_enum_definitions: &module.loaded_enum_definitions,
                loaded_struct_definitions: &module.loaded_struct_definitions,
                loaded_class_definitions: &module.loaded_class_definitions,
                loaded_interface_definitions: &module.loaded_interface_definitions,
                enum_applications: &module.enum_applications,
                classes: &module.classes,
                class_applications: &module.class_applications,
                interfaces: &module.interfaces,
                interface_applications: &module.interface_applications,
                objects: &module.objects,
                core_types: hir::HirCoreTypeIdentityAuthority::Defined(
                    &defined_export_core(module).fundamental_types,
                ),
                nominal_identities: &module.nominal_identities,
            },
            unit: module.unit,
        },
    )
    .expect("the MIR test keeps its callback identities structurally complete")
}

fn dump(module: &mir::Module) -> String {
    mir::dump(module)
        .lines()
        .filter(|line| {
            !(line.contains("class _") && line.contains("ExceptionProtocol"))
                && !line.contains("class _ThrowableProtocol")
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
    function_types: Arena<hir::FunctionType>,
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

fn test_local_selector(ordinal: u32) -> scoop_identity::LocalValueSelector {
    scoop_identity::LocalValueSelector::Synthetic {
        path: scoop_identity::StructuralDefinitionPath::from_first(
            scoop_identity::StructuralPathSegment::new(
                scoop_identity::StructuralDefinitionSiteRole::SyntheticValue,
                ordinal,
            ),
            [],
        ),
        role: scoop_identity::SyntheticLocalRole::Temporary,
    }
}

fn test_local_ordinal(name: &str, ty: hir::TypeId, line: u32, column: u32) -> u32 {
    let name = name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    name ^ ty.into_raw().into_u32().rotate_left(7) ^ line.rotate_left(13) ^ column.rotate_left(21)
}

#[track_caller]
fn local(name: &str, ty: hir::TypeId) -> hir::Local {
    let caller = std::panic::Location::caller();
    let ordinal = test_local_ordinal(name, ty, caller.line(), caller.column());
    hir::Local {
        binding: hir::BindingId::from_raw(ordinal),
        selector: test_local_selector(ordinal),
        definition: hir::LocalValueDefinitionSite::Synthetic,
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
    expr(
        hir::ExprKind::StringLiteral {
            value: value.to_string(),
            owner: hir::StringConstantOwner::CurrentDefinition,
        },
        h.string,
    )
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
    expr(
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc { kind, operation },
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
    expr(
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::Managed { kind, operation },
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
    let result_ty = h.integer(target_kind);
    expr(
        hir::ExprKind::IntegerConversion {
            conversion: hir::IntegerConversion {
                source,
                target_kind,
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
            binding: None,
            receiver: scoop_hir::SourceCallReceiver::NoReceiver,
            callee: (hir::Callable::Function(function)).into(),
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
            binding: None,
            receiver: scoop_hir::SourceCallReceiver::NoReceiver,
            callee: (hir::Callable::Function(function)).into(),
            args,
        },
        ty,
    )
}

fn struct_init(h: &Harness, ty: hir::TypeId, args: Vec<hir::Expr>) -> hir::Expr {
    let hir::Type::Struct(application) = h.types[ty] else {
        panic!("struct construction requires a struct application type")
    };
    let constructor =
        h.structs[h.struct_id(h.struct_applications[application].template)].constructors[0];
    let application = h
        .struct_constructor_applications
        .iter()
        .find_map(|(id, candidate)| {
            (candidate.constructor == scoop_hir::StructConstructorDefinition::Local(constructor)
                && candidate.owner == application)
                .then_some(id)
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
            template: module.nominal_identities[template].declaration_id(),
            arguments,
            canonical_type,
        });
    let actual_type = module.types.alloc(hir::Type::Interface(application));
    assert_eq!(actual_type, canonical_type);
    canonical_type
}

/// `main` calls `println("hello, world")` then `helper()`, which
/// calls `print("!")`.
fn hello_world() -> hir::ExportHirOutput {
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
            binding: None,
            receiver: scoop_hir::SourceCallReceiver::NoReceiver,
            callee: (hir::Callable::Generic(resolved)).into(),
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

/// The display name a vtable / itable slot points at.
fn slot_fn<'a>(module: &'a mir::Module, slot: &mir::TableSlot) -> &'a str {
    match slot {
        mir::TableSlot::Function(id) => &module.functions[*id].name,
        mir::TableSlot::External(_) => "external",
        mir::TableSlot::Runtime(function) => function.symbol(),
    }
}
