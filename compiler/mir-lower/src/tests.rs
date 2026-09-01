use super::*;
use scoop_ast::Span;
use scoop_hir as hir;

mod coroutines;
mod generics;
mod overloads;
mod reference_types;
mod value_types;

fn lower(module: &hir::Module) -> mir::Module {
    let concrete = scoop_hir_lower::concretize_export(module);
    let mut module = super::lower(&concrete);
    // Handcrafted unit modules use hidden, valid exception shells to satisfy
    // LocalConcreteHir's complete core contract. Keep their constructor
    // functions out of unrelated top-level ordering/dump assertions.
    let functions = &module.functions;
    module.top_level.retain(|id| {
        let name = &functions[*id].name;
        !(name.starts_with("ctor.$") && name.ends_with("Protocol"))
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

fn type_param(name: impl Into<String>) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id: hir::TypeParamId::from_raw(0),
        name: name.into(),
        variance: hir::Variance::Invariant,
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
    struct_applications: Arena<hir::StructApplication>,
    struct_applications_by_key:
        HashMap<(hir::StructId, Vec<hir::TypeId>), hir::StructApplicationId>,
    enums: Arena<hir::EnumDecl>,
    enum_applications: Arena<hir::EnumApplication>,
    enum_applications_by_key: HashMap<(hir::EnumId, Vec<hir::TypeId>), hir::EnumApplicationId>,
    classes: Arena<hir::ClassDecl>,
    class_applications: Arena<hir::ClassApplication>,
    class_applications_by_key: HashMap<(hir::ClassId, Vec<hir::TypeId>), hir::ClassApplicationId>,
    interfaces: Arena<hir::InterfaceDecl>,
    interface_applications: Arena<hir::InterfaceApplication>,
    interface_methods: Arena<hir::InterfaceMethod>,
    interface_applications_by_key:
        HashMap<(hir::InterfaceId, Vec<hir::TypeId>), hir::InterfaceApplicationId>,
    top_level: Vec<hir::FunctionId>,
    unit: hir::TypeId,
    int: hir::TypeId,
    boolean: hir::TypeId,
    string: hir::TypeId,
    option_enum: hir::EnumId,
    write: Option<hir::FunctionId>,
    int_to_string: Option<hir::FunctionId>,
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
    uint: Option<hir::TypeId>,
    gc_core: Option<GcCore>,
    intrinsic_array: Option<hir::ClassId>,
    intrinsic_mutable_array: Option<hir::ClassId>,
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

impl Harness {
    fn new() -> Self {
        let mut types = Arena::new();
        let unit = types.alloc(hir::Type::Unit);
        let int = types.alloc(hir::Type::Int);
        let boolean = types.alloc(hir::Type::Boolean);
        let string = types.alloc(hir::Type::String);
        let functions = Arena::new();
        let extern_functions = Arena::new();
        // scoop.core's `enum Option<T> { Some(T), None }`.
        let t = types.alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let mut enums = Arena::new();
        let mut enum_applications = Arena::new();
        let option_self_application = hir::EnumApplicationId::from_raw(0.into());
        let option_enum = enums.alloc(hir::EnumDecl {
            name: "Option".to_string(),
            self_application: option_self_application,
            type_params: vec![type_param("T")],
            no_gc: false,
            variants: vec![
                hir::Variant {
                    name: "Some".to_string(),
                    fields: vec![hir::Field {
                        name: "_1".to_string(),
                        ty: t,
                    }],
                    defaults: vec![None],
                },
                hir::Variant {
                    name: "None".to_string(),
                    fields: Vec::new(),
                    defaults: Vec::new(),
                },
            ],
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let option_self_type = hir::TypeId::from_raw((types.len() as u32).into());
        let actual_option_self_application = enum_applications.alloc(hir::EnumApplication {
            template: option_enum,
            arguments: vec![t],
            canonical_type: option_self_type,
        });
        assert_eq!(actual_option_self_application, option_self_application);
        let actual_option_self_type = types.alloc(hir::Type::Enum(actual_option_self_application));
        assert_eq!(actual_option_self_type, option_self_type);
        let mut enum_applications_by_key = HashMap::new();
        enum_applications_by_key.insert((option_enum, vec![t]), actual_option_self_application);
        Harness {
            types,
            functions,
            extern_functions,
            generic_functions: Arena::new(),
            method_applications: Arena::new(),
            method_applications_by_key: HashMap::new(),
            generic_methods: Arena::new(),
            generic_method_applications: Arena::new(),
            structs: Arena::new(),
            struct_applications: Arena::new(),
            struct_applications_by_key: HashMap::new(),
            enums,
            enum_applications,
            enum_applications_by_key,
            classes: Arena::new(),
            class_applications: Arena::new(),
            class_applications_by_key: HashMap::new(),
            interfaces: Arena::new(),
            interface_applications: Arena::new(),
            interface_methods: Arena::new(),
            interface_applications_by_key: HashMap::new(),
            top_level: Vec::new(),
            unit,
            int,
            boolean,
            string,
            option_enum,
            write: None,
            int_to_string: None,
            bool_to_string: None,
            print_string: None,
            print_int: None,
            print_boolean: None,
            println_string: None,
            println_int: None,
            println_boolean: None,
            instantiations: Arena::new(),
            uint: None,
            gc_core: None,
            intrinsic_array: None,
            intrinsic_mutable_array: None,
        }
    }

    /// Adds core's managed `write` extern on first use so tests unrelated
    /// to output keep their MIR dumps focused on the feature under test.
    fn write(&mut self) -> hir::FunctionId {
        if let Some(id) = self.write {
            return id;
        }
        let extern_id = self.extern_functions.alloc(hir::ExternFunction {
            source_name: "write".to_string(),
            native_symbol: "scoop_rt_write".to_string(),
            library: String::new(),
            abi: hir::ExternAbi::Scoop,
            calling_convention: hir::CallingConvention::Cdecl,
            gc_effect: hir::GcEffect::Managed,
            safety: hir::Safety::Safe,
            params: vec![self.string],
            return_type: self.unit,
        });
        let id = self.functions.alloc(hir::Function {
            name: "write".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Extern(extern_id),
            method: None,
            span: SPAN,
        });
        self.top_level.push(id);
        self.write = Some(id);
        id
    }

    /// scoop.core's representation-level integer formatter. This is an
    /// ordinary Scoop-ABI extern declaration, never an intrinsic/runtime
    /// function kind.
    fn int_to_string(&mut self) -> hir::FunctionId {
        if let Some(id) = self.int_to_string {
            return id;
        }
        let extern_id = self.extern_functions.alloc(hir::ExternFunction {
            source_name: "coreIntToString".to_string(),
            native_symbol: "scoop_rt_int_to_string".to_string(),
            library: String::new(),
            abi: hir::ExternAbi::Scoop,
            calling_convention: hir::CallingConvention::Cdecl,
            gc_effect: hir::GcEffect::Managed,
            safety: hir::Safety::Safe,
            params: vec![self.int],
            return_type: self.string,
        });
        let id = self.functions.alloc(hir::Function {
            name: "coreIntToString".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: self.string,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Extern(extern_id),
            method: None,
            span: SPAN,
        });
        self.top_level.push(id);
        self.int_to_string = Some(id);
        id
    }

    /// scoop.core's representation-level Boolean formatter. Like the integer
    /// formatter, this is an ordinary Scoop-ABI extern declaration.
    fn bool_to_string(&mut self) -> hir::FunctionId {
        if let Some(id) = self.bool_to_string {
            return id;
        }
        let extern_id = self.extern_functions.alloc(hir::ExternFunction {
            source_name: "coreBooleanToString".to_string(),
            native_symbol: "scoop_rt_bool_to_string".to_string(),
            library: String::new(),
            abi: hir::ExternAbi::Scoop,
            calling_convention: hir::CallingConvention::Cdecl,
            gc_effect: hir::GcEffect::Managed,
            safety: hir::Safety::Safe,
            params: vec![self.boolean],
            return_type: self.string,
        });
        let id = self.functions.alloc(hir::Function {
            name: "coreBooleanToString".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: self.string,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Extern(extern_id),
            method: None,
            span: SPAN,
        });
        self.top_level.push(id);
        self.bool_to_string = Some(id);
        id
    }

    /// core's `fun print(message: String) = write(message)`,
    /// created on first use (tests that never print keep core's
    /// overloads out of their MIR dumps).
    fn print_string(&mut self) -> hir::FunctionId {
        if let Some(id) = self.print_string {
            return id;
        }
        let (unit, string) = (self.unit, self.string);
        let write = self.write();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", string));
        let id = self.user_fn_full(
            "print",
            Vec::new(),
            vec![param("message", string, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    write,
                    vec![local_ref(message, string)],
                    unit,
                ))],
            },
        );
        self.print_string = Some(id);
        id
    }

    /// core's `fun print(message: Int) = write(intToString(message))`.
    fn print_int(&mut self) -> hir::FunctionId {
        if let Some(id) = self.print_int {
            return id;
        }
        let (unit, int, string) = (self.unit, self.int, self.string);
        let write = self.write();
        let int_to_string = self.int_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", int));
        let id = self.user_fn_full(
            "print",
            Vec::new(),
            vec![param("message", int, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    write,
                    vec![call_typed(
                        int_to_string,
                        vec![local_ref(message, int)],
                        string,
                    )],
                    unit,
                ))],
            },
        );
        self.print_int = Some(id);
        id
    }

    /// core's `fun print(message: Boolean) = write(boolToString(message))`.
    fn print_boolean(&mut self) -> hir::FunctionId {
        if let Some(id) = self.print_boolean {
            return id;
        }
        let (unit, boolean, string) = (self.unit, self.boolean, self.string);
        let write = self.write();
        let bool_to_string = self.bool_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", boolean));
        let id = self.user_fn_full(
            "print",
            Vec::new(),
            vec![param("message", boolean, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    write,
                    vec![call_typed(
                        bool_to_string,
                        vec![local_ref(message, boolean)],
                        string,
                    )],
                    unit,
                ))],
            },
        );
        self.print_boolean = Some(id);
        id
    }

    /// core's `fun println(message: String) { write(message); write("\n") }`.
    fn println_string(&mut self) -> hir::FunctionId {
        if let Some(id) = self.println_string {
            return id;
        }
        let (unit, string) = (self.unit, self.string);
        let write = self.write();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", string));
        let id = self.user_fn_full(
            "println",
            Vec::new(),
            vec![param("message", string, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![
                    expr_stmt(call_typed(write, vec![local_ref(message, string)], unit)),
                    expr_stmt(call_typed(
                        write,
                        vec![expr(hir::ExprKind::StringLiteral("\n".to_string()), string)],
                        unit,
                    )),
                ],
            },
        );
        self.println_string = Some(id);
        id
    }

    /// core's `fun println(message: Int) = println(intToString(message))`.
    fn println_int(&mut self) -> hir::FunctionId {
        if let Some(id) = self.println_int {
            return id;
        }
        let println_string = self.println_string();
        let (unit, int, string) = (self.unit, self.int, self.string);
        let int_to_string = self.int_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", int));
        let id = self.user_fn_full(
            "println",
            Vec::new(),
            vec![param("message", int, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    println_string,
                    vec![call_typed(
                        int_to_string,
                        vec![local_ref(message, int)],
                        string,
                    )],
                    unit,
                ))],
            },
        );
        self.println_int = Some(id);
        id
    }

    /// core's `fun println(message: Boolean) = println(boolToString(message))`.
    fn println_boolean(&mut self) -> hir::FunctionId {
        if let Some(id) = self.println_boolean {
            return id;
        }
        let println_string = self.println_string();
        let (unit, boolean, string) = (self.unit, self.boolean, self.string);
        let bool_to_string = self.bool_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", boolean));
        let id = self.user_fn_full(
            "println",
            Vec::new(),
            vec![param("message", boolean, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    println_string,
                    vec![call_typed(
                        bool_to_string,
                        vec![local_ref(message, boolean)],
                        string,
                    )],
                    unit,
                ))],
            },
        );
        self.println_boolean = Some(id);
        id
    }

    /// `Option<inner>` (core's enum applied to one argument).
    fn option(&mut self, inner: hir::TypeId) -> hir::TypeId {
        let application = self.enum_application(self.option_enum, vec![inner]);
        self.enum_applications[application].canonical_type
    }

    fn any(&mut self) -> hir::TypeId {
        self.types.alloc(hir::Type::Any)
    }

    fn class_ty(&mut self, id: hir::ClassId) -> hir::TypeId {
        let application = self.class_application(id, Vec::new());
        self.class_applications[application].canonical_type
    }

    fn interface_ty(&mut self, id: hir::InterfaceId) -> hir::TypeId {
        self.interface_app(id, Vec::new())
    }

    fn interface_app(&mut self, id: hir::InterfaceId, arguments: Vec<hir::TypeId>) -> hir::TypeId {
        assert_eq!(self.interfaces[id].type_params.len(), arguments.len());
        let application = self.interface_application(id, arguments);
        self.interface_applications[application].canonical_type
    }

    fn struct_ty(&mut self, id: hir::StructId) -> hir::TypeId {
        self.struct_app(id, Vec::new())
    }

    fn enum_ty(&mut self, id: hir::EnumId) -> hir::TypeId {
        assert!(self.enums[id].type_params.is_empty());
        let application = self.enum_application(id, Vec::new());
        self.enum_applications[application].canonical_type
    }

    fn struct_application_of(&self, ty: hir::TypeId) -> hir::StructApplicationId {
        let hir::Type::Struct(application) = self.types[ty] else {
            panic!("expected a struct application type")
        };
        application
    }

    fn enum_application_of(&self, ty: hir::TypeId) -> hir::EnumApplicationId {
        let hir::Type::Enum(application) = self.types[ty] else {
            panic!("expected an enum application type")
        };
        application
    }

    fn class_application_of(&self, ty: hir::TypeId) -> hir::ClassApplicationId {
        let hir::Type::Class(application) = self.types[ty] else {
            panic!("expected a class application type")
        };
        application
    }

    fn struct_application(
        &mut self,
        template: hir::StructId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::StructApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.struct_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.struct_applications.alloc(hir::StructApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Declared,
        });
        let actual_type = self.types.alloc(hir::Type::Struct(application));
        assert_eq!(actual_type, canonical_type);
        self.struct_applications_by_key.insert(key, application);
        application
    }

    fn enum_application(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::EnumApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.enum_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.enum_applications.alloc(hir::EnumApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
        });
        let actual_type = self.types.alloc(hir::Type::Enum(application));
        assert_eq!(actual_type, canonical_type);
        self.enum_applications_by_key.insert(key, application);
        application
    }

    fn declare_enum(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        variants: Vec<hir::Variant>,
    ) -> hir::EnumId {
        assert_eq!(type_params.len(), self_arguments.len());
        let self_application =
            hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
        let enumeration = self.enums.alloc(hir::EnumDecl {
            name: name.to_string(),
            self_application,
            type_params,
            no_gc: false,
            variants,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let actual = self.enum_application(enumeration, self_arguments);
        assert_eq!(actual, self_application);
        enumeration
    }

    fn class_application(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::ClassApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.class_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let representation = match self.classes[template].representation {
            hir::ClassRepresentation::Declared(_) => hir::ClassApplicationRepresentation::Declared,
            hir::ClassRepresentation::Intrinsic(declaration) => {
                hir::ClassApplicationRepresentation::Intrinsic(declaration.kind.application(&key.1))
            }
        };
        let application = self.class_applications.alloc(hir::ClassApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
            representation,
        });
        let actual_type = self.types.alloc(hir::Type::Class(application));
        assert_eq!(actual_type, canonical_type);
        self.class_applications_by_key.insert(key, application);
        application
    }

    fn interface_application(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::InterfaceApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.interface_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template,
                arguments: key.1.clone(),
                canonical_type,
            });
        let actual_type = self.types.alloc(hir::Type::Interface(application));
        assert_eq!(actual_type, canonical_type);
        self.interface_applications_by_key.insert(key, application);
        application
    }

    fn declare_interface(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        methods: Vec<hir::MethodSig>,
    ) -> hir::InterfaceId {
        assert_eq!(type_params.len(), self_arguments.len());
        let self_application = hir::InterfaceApplicationId::from_raw(
            (self.interface_applications.len() as u32).into(),
        );
        let interface = self.interfaces.alloc(hir::InterfaceDecl {
            name: name.to_string(),
            self_application,
            type_params,
            parents: Vec::new(),
            methods: Vec::new(),
            span: SPAN,
        });
        let actual = self.interface_application(interface, self_arguments);
        assert_eq!(actual, self_application);
        for method in methods {
            self.add_interface_method_signature(interface, method);
        }
        interface
    }

    fn add_interface_method_signature(
        &mut self,
        interface: hir::InterfaceId,
        method: hir::MethodSig,
    ) {
        assert!(method.type_params.is_empty());
        let declaration = self.interfaces[interface].clone();
        let owner = self.interface_applications[declaration.self_application].canonical_type;
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", owner));
        let mut params = vec![param("this", owner, this)];
        for source in method.params {
            let local = locals.alloc(local(&source.name, source.ty));
            params.push(param(&source.name, source.ty, local));
        }
        let function = self.functions.alloc(hir::Function {
            name: format!("{}.{}", declaration.name, method.name),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: method.is_suspend,
            params,
            return_ty: method.return_ty,
            attributes: method.attributes,
            kind: hir::FunctionKind::User(hir::Body {
                locals,
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
                operator: None,
            }),
            span: method.span,
        });
        if !declaration.type_params.is_empty() {
            self.functions[function].genericity =
                hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters: declaration.type_params,
                    no_gc_type_params: Vec::new(),
                };
        }
        let member = self.interface_methods.alloc(hir::InterfaceMethod {
            owner: interface,
            function,
        });
        self.functions[function].method.as_mut().unwrap().dispatch =
            hir::MethodDispatch::Interface(member);
        self.interfaces[interface].methods.push(member);
    }

    fn interface(&mut self, name: &str, methods: &[&str]) -> hir::InterfaceId {
        let unit = self.unit;
        let methods = methods
            .iter()
            .map(|name| hir::MethodSig {
                name: name.to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
                span: SPAN,
            })
            .collect();
        self.declare_interface(name, Vec::new(), Vec::new(), methods)
    }

    #[allow(clippy::too_many_arguments)]
    fn class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base: Option<(hir::ClassId, Vec<hir::Expr>)>,
        interfaces: &[hir::InterfaceId],
    ) -> hir::ClassId {
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let base = base.map(|(base, arguments)| (self.class_ty(base), arguments));
        self.declare_class(name, modifier, constructor, base, interfaces)
    }

    fn declare_class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base_class: Option<(hir::TypeId, Vec<hir::Expr>)>,
        interfaces: Vec<hir::TypeId>,
    ) -> hir::ClassId {
        let mut interface_implementations = base_class
            .as_ref()
            .map(|(base, _)| {
                let hir::Type::Class(application) = self.types[*base] else {
                    panic!("test harness class bases are class applications")
                };
                let base = self.class_applications[application].template;
                self.classes[base].interface_implementations.clone()
            })
            .unwrap_or_default();
        for implementation in self.interface_implementation_shells(&interfaces) {
            if let Some(existing) = interface_implementations
                .iter_mut()
                .find(|existing| existing.interface == implementation.interface)
            {
                *existing = implementation;
            } else {
                interface_implementations.push(implementation);
            }
        }
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let class = self.classes.alloc(hir::ClassDecl {
            modifier,
            name: name.to_string(),
            self_application,
            type_params: Vec::new(),
            representation: hir::ClassRepresentation::Declared(
                constructor
                    .iter()
                    .enumerate()
                    .map(|(index, (name, ty))| hir::ConstructorField {
                        parameter: hir::ConstructorParamId::from_raw(index as u32),
                        name: name.to_string(),
                        ty: *ty,
                        mutable: false,
                    })
                    .collect(),
            ),
            base_class,
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            span: SPAN,
        });
        let actual = self.class_application(class, Vec::new());
        assert_eq!(actual, self_application);
        class
    }

    /// A concrete zero-argument exception shell used by tests that exercise
    /// compiler-generated exception edges.
    fn exception(&mut self, name: &str) -> hir::ClassId {
        self.class(name, hir::ClassModifier::Final, &[], None, &[])
    }

    fn exception_target(&mut self, name: &str, include: bool) -> hir::CompilerException {
        let existing = self
            .classes
            .iter()
            .find_map(|(id, declaration)| (declaration.name == name).then_some(id));
        let class = if let Some(existing) = existing {
            existing
        } else if include {
            self.exception(name)
        } else {
            self.class(
                &format!("${name}Protocol"),
                // LocalConcreteHir's exception contract always includes a
                // real constructor callable. The protocol shell remains
                // hidden from unrelated dump assertions by the test helper.
                hir::ClassModifier::Final,
                &[],
                None,
                &[],
            )
        };
        hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor { class },
        }
    }

    fn test_exception_core(&mut self, include: bool) -> hir::CompilerExceptionCore {
        hir::CompilerExceptionCore {
            throwable: self.exception_target("Throwable", include),
            unwrap_exception: self.exception_target("UnwrapException", include),
            class_cast_exception: self.exception_target("ClassCastException", include),
            arithmetic_exception: self.exception_target("ArithmeticException", include),
            index_out_of_bounds_exception: self
                .exception_target("IndexOutOfBoundsException", include),
            illegal_state_exception: self.exception_target("IllegalStateException", include),
        }
    }

    /// A member function (kept out of `top_level`, as hir-lower
    /// does); member metadata carries the receiver type. The name is
    /// qualified `Owner.method`, as hir-lower names members.
    fn method_fn(
        &mut self,
        name: &str,
        method_of: hir::TypeId,
        params: Vec<hir::Param>,
        return_ty: hir::TypeId,
        body: hir::Body,
    ) -> hir::FunctionId {
        let genericity = match self.types[method_of] {
            hir::Type::Class(application) => {
                let parameters = self.classes[self.class_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Struct(application) => {
                let parameters = self.structs[self.struct_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Enum(application) => {
                let parameters = self.enums[self.enum_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Interface(application) => {
                let parameters = self.interfaces[self.interface_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Any => hir::FunctionGenericity::Plain,
            _ => panic!("test harness methods have nominal owners"),
        };
        let dispatch = match self.types[method_of] {
            hir::Type::Class(application) => {
                let class = self.class_applications[application].template;
                self.inherited_virtual_dispatch(class, name, &params, return_ty)
                    .unwrap_or_else(|| {
                        hir::MethodDispatch::Virtual(hir::VirtualMethodId::from_raw(
                            self.functions.len() as u32,
                        ))
                    })
            }
            hir::Type::Interface(application) => {
                let interface = self.interface_applications[application].template;
                let member = self.interfaces[interface]
                    .methods
                    .iter()
                    .copied()
                    .find(|member| {
                        self.same_method_shape(
                            self.interface_methods[*member].function,
                            name,
                            &params,
                            return_ty,
                        )
                    })
                    .expect("test interface method names an existing typed member");
                hir::MethodDispatch::Interface(member)
            }
            _ => hir::MethodDispatch::Direct,
        };
        let function = self.functions.alloc(hir::Function {
            name: name.to_string(),
            genericity,
            is_suspend: false,
            params,
            return_ty,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(body),
            method: Some(hir::Method {
                owner: method_of,
                modifier: hir::MethodModifier::Open,
                dispatch,
                operator: None,
            }),
            span: SPAN,
        });
        match self.types[method_of] {
            hir::Type::Class(application) => self.classes
                [self.class_applications[application].template]
                .methods
                .push(function),
            hir::Type::Struct(application) => self.structs
                [self.struct_applications[application].template]
                .methods
                .push(function),
            hir::Type::Enum(application) => self.enums
                [self.enum_applications[application].template]
                .methods
                .push(function),
            hir::Type::Interface(_) | hir::Type::Any => {}
            _ => unreachable!(),
        }
        self.bind_interface_implementation(method_of, function);
        function
    }

    fn inherited_virtual_dispatch(
        &self,
        class: hir::ClassId,
        name: &str,
        params: &[hir::Param],
        return_ty: hir::TypeId,
    ) -> Option<hir::MethodDispatch> {
        let mut base = self.classes[class].base_class.as_ref().map(|(base, _)| {
            let hir::Type::Class(application) = self.types[*base] else {
                panic!("test harness class bases are class applications")
            };
            self.class_applications[application].template
        });
        while let Some(class) = base {
            if let Some(dispatch) = self.classes[class]
                .methods
                .iter()
                .copied()
                .find_map(|method| {
                    self.same_method_shape(method, name, params, return_ty)
                        .then_some(self.functions[method].method?.dispatch)
                })
            {
                return Some(dispatch);
            }
            base = self.classes[class].base_class.as_ref().map(|(base, _)| {
                let hir::Type::Class(application) = self.types[*base] else {
                    panic!("test harness class bases are class applications")
                };
                self.class_applications[application].template
            });
        }
        None
    }

    fn bind_interface_implementation(&mut self, owner: hir::TypeId, function: hir::FunctionId) {
        let implementations = match self.types[owner] {
            hir::Type::Class(application) => {
                let owner = self.class_applications[application].template;
                self.classes[owner].interface_implementations.clone()
            }
            hir::Type::Struct(application) => {
                let owner = self.struct_applications[application].template;
                self.structs[owner].interface_implementations.clone()
            }
            hir::Type::Enum(application) => {
                let owner = self.enum_applications[application].template;
                self.enums[owner].interface_implementations.clone()
            }
            hir::Type::Interface(_) | hir::Type::Any => return,
            _ => unreachable!("test harness methods have nominal owners"),
        };
        let mut matches = Vec::new();
        for (implementation_index, implementation) in implementations.iter().enumerate() {
            for (method_index, implementation_method) in implementation.methods.iter().enumerate() {
                let declaration = self.interface_methods[implementation_method.member].function;
                if self.same_method_shape(
                    declaration,
                    &self.functions[function].name,
                    &self.functions[function].params,
                    self.functions[function].return_ty,
                ) {
                    matches.push((implementation_index, method_index));
                }
            }
        }
        if matches.is_empty() {
            return;
        }
        let application = self.method_application(function);
        let target = hir::InterfaceImplementationTarget::Method(application);
        match self.types[owner] {
            hir::Type::Class(owner_application) => {
                let owner = self.class_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.classes[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            hir::Type::Struct(owner_application) => {
                let owner = self.struct_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.structs[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            hir::Type::Enum(owner_application) => {
                let owner = self.enum_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.enums[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            _ => unreachable!(),
        }
    }

    fn same_method_shape(
        &self,
        candidate: hir::FunctionId,
        name: &str,
        params: &[hir::Param],
        return_ty: hir::TypeId,
    ) -> bool {
        let candidate = &self.functions[candidate];
        candidate.name.rsplit('.').next() == name.rsplit('.').next()
            && candidate.params.len() == params.len()
            && candidate
                .params
                .iter()
                .skip(1)
                .zip(params.iter().skip(1))
                .all(|(left, right)| left.ty == right.ty)
            && candidate.return_ty == return_ty
    }

    fn method_application(&mut self, function: hir::FunctionId) -> hir::MethodApplicationId {
        let owner_ty = self.functions[function]
            .method
            .expect("test harness method has metadata")
            .owner;
        let owner = match self.types[owner_ty] {
            hir::Type::Class(application) => hir::MethodOwnerApplication::Class(application),
            hir::Type::Struct(application) => hir::MethodOwnerApplication::Struct(application),
            hir::Type::Enum(application) => hir::MethodOwnerApplication::Enum(application),
            hir::Type::Interface(application) => {
                hir::MethodOwnerApplication::Interface(application)
            }
            hir::Type::Any => panic!("Any has no methods"),
            _ => panic!("test harness methods have nominal owners"),
        };
        let key = (function, owner);
        if let Some(&application) = self.method_applications_by_key.get(&key) {
            return application;
        }
        let application = self
            .method_applications
            .alloc(hir::MethodApplication { function, owner });
        self.method_applications_by_key.insert(key, application);
        application
    }

    fn strukt(&mut self, name: &str, fields: &[(&str, hir::TypeId)]) -> hir::StructId {
        self.strukt_with(name, fields, &[])
    }

    fn strukt_with(
        &mut self,
        name: &str,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        self.declare_struct(name, Vec::new(), Vec::new(), fields, interfaces)
    }

    fn declare_struct(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        assert_eq!(type_params.len(), self_arguments.len());
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let interface_implementations = self.interface_implementation_shells(&interfaces);
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            self_application,
            type_params,
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Declared(
                fields
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
            ),
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let actual = self.struct_application(strukt, self_arguments);
        assert_eq!(actual, self_application);
        strukt
    }

    fn declare_fixed_intrinsic_struct(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        canonical_type: hir::TypeId,
    ) -> hir::StructId {
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            self_application,
            type_params: Vec::new(),
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Intrinsic(declaration),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let representation = kind.application(&[]);
        let actual = self.struct_applications.alloc(hir::StructApplication {
            template: strukt,
            arguments: Vec::new(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        self.struct_applications_by_key
            .insert((strukt, Vec::new()), actual);
        strukt
    }

    fn declare_intrinsic_class(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        canonical_type_plan: CanonicalTypePlan,
    ) -> hir::ClassId {
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let class = self.classes.alloc(hir::ClassDecl {
            modifier: hir::ClassModifier::Final,
            name: name.to_string(),
            self_application,
            type_params,
            representation: hir::ClassRepresentation::Intrinsic(declaration),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: SPAN,
        });
        let representation = kind.application(&self_arguments);
        let (canonical_type, allocate_canonical_type) = match canonical_type_plan {
            CanonicalTypePlan::Existing(canonical_type) => (canonical_type, false),
            CanonicalTypePlan::Allocate => (
                hir::TypeId::from_raw((self.types.len() as u32).into()),
                true,
            ),
        };
        let actual = self.class_applications.alloc(hir::ClassApplication {
            template: class,
            arguments: self_arguments.clone(),
            canonical_type,
            representation: hir::ClassApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        if allocate_canonical_type {
            let allocated = self.types.alloc(hir::Type::Class(actual));
            assert_eq!(allocated, canonical_type);
        }
        self.class_applications_by_key
            .insert((class, self_arguments), actual);
        class
    }

    fn interface_implementation_shells(
        &self,
        interfaces: &[hir::TypeId],
    ) -> Vec<hir::InterfaceImplementation> {
        interfaces
            .iter()
            .map(|&interface| {
                let hir::Type::Interface(application) = self.types[interface] else {
                    panic!("test harness interface lists are fully applied")
                };
                let template = self.interface_applications[application].template;
                hir::InterfaceImplementation {
                    interface: application,
                    methods: self.interfaces[template]
                        .methods
                        .iter()
                        .map(|&member| hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// The `UInt` well-known type (M9, spec 11.2), allocated on
    /// first use.
    fn uint(&mut self) -> hir::TypeId {
        if let Some(ty) = self.uint {
            return ty;
        }
        let ty = self.types.alloc(hir::Type::UInt);
        self.uint = Some(ty);
        ty
    }

    /// Intern a generic struct application type.
    fn struct_app(&mut self, struct_id: hir::StructId, args: Vec<hir::TypeId>) -> hir::TypeId {
        assert_eq!(self.structs[struct_id].type_params.len(), args.len());
        let application = self.struct_application(struct_id, args);
        self.struct_applications[application].canonical_type
    }

    /// core's GC facilities (M12): `PinnedPtr<T>` / `GcHandle<T>`
    /// and the six low-level runtime intrinsics, created on first use.
    fn gc_core(&mut self) -> GcCore {
        if let Some(core) = self.gc_core {
            return core;
        }
        let uint = self.uint();
        let unit = self.unit;
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let pinned_ptr = self.declare_struct(
            "PinnedPtr",
            vec![type_param("T")],
            vec![t],
            &[("raw", uint)],
            &[],
        );
        let gc_handle = self.declare_struct(
            "GcHandle",
            vec![type_param("T")],
            vec![t],
            &[("raw", uint)],
            &[],
        );
        let mut dummy_locals = Arena::new();
        let mut intrinsic = |name: &str,
                             intrinsic: &str,
                             type_params: Vec<String>,
                             params: Vec<(&str, hir::TypeId)>,
                             return_ty: hir::TypeId| {
            let generic = !type_params.is_empty();
            let type_params = type_params.into_iter().map(type_param).collect();
            let id = self.functions.alloc(hir::Function {
                name: name.to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: params
                    .into_iter()
                    .map(|(name, ty)| hir::Param {
                        name: name.to_string(),
                        ty,
                        local: dummy_locals.alloc(hir::Local {
                            binding: hir::BindingId::from_raw(dummy_locals.len() as u32),
                            name: name.to_string(),
                            ty,
                            mutable: false,
                        }),
                    })
                    .collect(),
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::intrinsic_spec(intrinsic)
                        .expect("test intrinsic is registered")
                        .kind,
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            if generic {
                self.register_generic(id, type_params);
            }
            self.top_level.push(id);
            id
        };
        let type_params = vec!["T".to_string()];
        let pin_raw = intrinsic(
            "_pin",
            "gc_pin_raw",
            type_params.clone(),
            vec![("v", t)],
            uint,
        );
        let unpin_raw = intrinsic(
            "_unpin",
            "gc_unpin_raw",
            type_params.clone(),
            vec![("raw", uint)],
            t,
        );
        let get_handle_raw = intrinsic(
            "_getGcHandle",
            "gc_get_handle_raw",
            type_params.clone(),
            vec![("v", t)],
            uint,
        );
        let release_handle_raw = intrinsic(
            "_releaseGcHandle",
            "gc_release_handle_raw",
            type_params,
            vec![("raw", uint)],
            t,
        );
        let gc_collect = intrinsic("gcCollect", "rt_gc_collect", vec![], vec![], unit);
        let gc_stats = intrinsic("gcStats", "rt_gc_stats", vec![], vec![], uint);
        let core = GcCore {
            pinned_ptr,
            gc_handle,
            pin_raw,
            unpin_raw,
            get_handle_raw,
            release_handle_raw,
            gc_collect,
            gc_stats,
        };
        self.gc_core = Some(core);
        core
    }

    fn tuple(&mut self, elements: &[hir::TypeId]) -> hir::TypeId {
        self.types.alloc(hir::Type::Tuple(elements.to_vec()))
    }

    fn intrinsic_array_class(&mut self, kind: hir::IntrinsicTypeKind) -> hir::ClassId {
        let existing = match kind {
            hir::IntrinsicTypeKind::Array => self.intrinsic_array,
            hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array,
            _ => unreachable!("array helper accepts only intrinsic array families"),
        };
        if let Some(class) = existing {
            return class;
        }
        let parameter = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let class = self.declare_intrinsic_class(
            kind.source_name(),
            kind,
            vec![type_param("T")],
            vec![parameter],
            CanonicalTypePlan::Allocate,
        );
        match kind {
            hir::IntrinsicTypeKind::Array => self.intrinsic_array = Some(class),
            hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array = Some(class),
            _ => unreachable!("array helper accepts only intrinsic array families"),
        }
        class
    }

    fn array(&mut self, element: hir::TypeId) -> hir::TypeId {
        let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
        let application = self.class_application(class, vec![element]);
        self.class_applications[application].canonical_type
    }

    fn mutable_array(&mut self, element: hir::TypeId) -> hir::TypeId {
        let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
        let application = self.class_application(class, vec![element]);
        self.class_applications[application].canonical_type
    }

    fn user_fn(&mut self, name: &str, body: hir::Body) -> hir::FunctionId {
        let unit = self.unit;
        self.user_fn_full(name, Vec::new(), Vec::new(), unit, body)
    }

    fn register_generic(
        &mut self,
        function: hir::FunctionId,
        parameters: Vec<hir::TypeParamDecl>,
    ) -> hir::GenericFunctionId {
        if let hir::FunctionGenericity::Generic {
            definition,
            parameters: existing,
        } = &self.functions[function].genericity
        {
            assert_eq!(existing, &parameters);
            return *definition;
        }
        let generic = self.generic_functions.alloc(hir::GenericFunction {
            function,
            no_gc_type_params: Vec::new(),
        });
        self.functions[function].genericity = hir::FunctionGenericity::Generic {
            definition: generic,
            parameters,
        };
        generic
    }

    fn user_fn_full(
        &mut self,
        name: &str,
        type_params: Vec<String>,
        params: Vec<hir::Param>,
        return_ty: hir::TypeId,
        body: hir::Body,
    ) -> hir::FunctionId {
        let generic = !type_params.is_empty();
        let type_params = type_params.into_iter().map(type_param).collect();
        let id = self.functions.alloc(hir::Function {
            name: name.to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params,
            return_ty,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(body),
            method: None,
            span: SPAN,
        });
        if generic {
            self.register_generic(id, type_params);
        }
        self.top_level.push(id);
        id
    }

    fn instantiate(
        &mut self,
        function: hir::FunctionId,
        type_args: Vec<hir::TypeId>,
    ) -> hir::ResolvedGenericFunctionId {
        let Some(generic) = self.functions[function].generic_definition() else {
            panic!("generic test function must be registered")
        };
        if let Some((id, _)) = self
            .instantiations
            .iter()
            .find(|(_, resolved)| resolved.generic == generic && resolved.type_args == type_args)
        {
            return id;
        }
        self.instantiations
            .alloc(hir::ResolvedGenericFunction { generic, type_args })
    }

    fn test_coroutine_core(&mut self, throwable: hir::ClassId) -> hir::CoroutineCore {
        let throwable_ty = self.class_ty(throwable);
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let type_param = || hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: "T".to_string(),
            variance: hir::Variance::Invariant,
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        };
        let continuation =
            self.declare_interface("Continuation", vec![type_param()], vec![t], Vec::new());
        let continuation_ty = self.interface_applications
            [self.interfaces[continuation].self_application]
            .canonical_type;
        let mut resume_locals = Arena::new();
        let resume_value = resume_locals.alloc(local("value", t));
        let continuation_resume = self.functions.alloc(hir::Function {
            name: "Continuation.resume".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: vec![param("value", t, resume_value)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: resume_locals,
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: continuation_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
                operator: None,
            }),
            span: SPAN,
        });
        let mut failure_locals = Arena::new();
        let failure = failure_locals.alloc(local("exception", throwable_ty));
        let continuation_resume_with_exception = self.functions.alloc(hir::Function {
            name: "Continuation.resumeWithException".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: vec![param("exception", throwable_ty, failure)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: failure_locals,
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: continuation_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
                operator: None,
            }),
            span: SPAN,
        });
        for method in [
            hir::MethodSig {
                name: "resume".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("value", t, resume_value)],
                return_ty: self.unit,
                span: SPAN,
            },
            hir::MethodSig {
                name: "resumeWithException".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("exception", throwable_ty, failure)],
                return_ty: self.unit,
                span: SPAN,
            },
        ] {
            self.add_interface_method_signature(continuation, method);
        }
        self.functions[continuation_resume]
            .method
            .as_mut()
            .expect("compiler-core declarations are interface methods")
            .dispatch = hir::MethodDispatch::Interface(self.interfaces[continuation].methods[0]);
        self.functions[continuation_resume_with_exception]
            .method
            .as_mut()
            .expect("compiler-core declarations are interface methods")
            .dispatch = hir::MethodDispatch::Interface(self.interfaces[continuation].methods[1]);

        let suspend_task = self.declare_interface(
            "SuspendTask",
            vec![type_param()],
            vec![t],
            vec![hir::MethodSig {
                name: "run".to_string(),
                is_suspend: true,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: t,
                span: SPAN,
            }],
        );
        let suspend_task_ty = self.interface_applications
            [self.interfaces[suspend_task].self_application]
            .canonical_type;
        let suspend_task_run = self.functions.alloc(hir::Function {
            name: "SuspendTask.run".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            params: Vec::new(),
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: suspend_task_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Interface(self.interfaces[suspend_task].methods[0]),
                operator: None,
            }),
            span: SPAN,
        });

        let suspend_registration = self.declare_interface(
            "SuspendRegistration",
            vec![type_param()],
            vec![t],
            vec![hir::MethodSig {
                name: "register".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: self.unit,
                span: SPAN,
            }],
        );
        let suspend_registration_ty = self.interface_applications
            [self.interfaces[suspend_registration].self_application]
            .canonical_type;
        let suspend_registration_register = self.functions.alloc(hir::Function {
            name: "SuspendRegistration.register".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: suspend_registration_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Interface(
                    self.interfaces[suspend_registration].methods[0],
                ),
                operator: None,
            }),
            span: SPAN,
        });

        for function in [
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task_run,
            suspend_registration_register,
        ] {
            self.functions[function].genericity =
                hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters: vec![type_param()],
                    no_gc_type_params: Vec::new(),
                };
        }
        let start_coroutine = self.functions.alloc(hir::Function {
            name: "startCoroutine".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::CoroutineStart,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
        let suspend_coroutine = self.functions.alloc(hir::Function {
            name: "suspendCoroutine".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            params: Vec::new(),
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::CoroutineSuspend,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
        for function in [start_coroutine, suspend_coroutine] {
            self.register_generic(function, vec![type_param()]);
            self.top_level.push(function);
        }
        hir::CoroutineCore {
            continuation,
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        }
    }

    fn finish(self, entry: hir::FunctionId) -> hir::Module {
        self.finish_with_coroutine_core(entry, false)
    }

    fn finish_coroutines(self, entry: hir::FunctionId) -> hir::Module {
        self.finish_with_coroutine_core(entry, true)
    }

    fn finish_with_coroutine_core(
        mut self,
        entry: hir::FunctionId,
        include_exceptions: bool,
    ) -> hir::Module {
        let exception_core = self.test_exception_core(include_exceptions);
        let coroutine_core = self.test_coroutine_core(exception_core.throwable.class());
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let ptr = self.declare_struct("Ptr", vec![type_param("T")], vec![t], &[], &[]);
        let fun_ptr = self.declare_struct("FunPtr", vec![type_param("F")], vec![t], &[], &[]);
        let pinned_ptr = self.declare_struct("PinnedPtr", vec![type_param("T")], vec![t], &[], &[]);
        let gc_handle = self.declare_struct("GcHandle", vec![type_param("T")], vec![t], &[], &[]);
        let ffi_core = hir::FfiCore {
            ptr,
            fun_ptr,
            pinned_ptr,
            gc_handle,
            ptr_to_uint: entry,
            ptr_cast: entry,
            ptr_load: entry,
            ptr_load_offset: entry,
            ptr_store: entry,
            ptr_store_offset: entry,
            ptr_plus: entry,
            ptr_minus: entry,
            address_of: entry,
            size_of: entry,
            align_of: entry,
            gc_pin_raw: entry,
            gc_unpin_raw: entry,
            gc_get_handle_raw: entry,
            gc_release_handle_raw: entry,
        };
        let unit_variants = |variants: &[&str]| {
            variants
                .iter()
                .map(|variant| hir::Variant {
                    name: (*variant).to_string(),
                    fields: Vec::new(),
                    defaults: Vec::new(),
                })
                .collect()
        };
        let callback_mode = self.declare_enum(
            "ForeignCallbackMode",
            Vec::new(),
            Vec::new(),
            unit_variants(&["Reusable", "OneShot"]),
        );
        let callback_state = self.declare_enum(
            "ForeignCallbackState",
            Vec::new(),
            Vec::new(),
            unit_variants(&["Registered", "Active", "Completed", "Failed"]),
        );
        let uint = self.uint();
        let intrinsic_int =
            self.declare_fixed_intrinsic_struct("Int", hir::IntrinsicTypeKind::Int, self.int);
        let intrinsic_uint =
            self.declare_fixed_intrinsic_struct("UInt", hir::IntrinsicTypeKind::UInt, uint);
        let intrinsic_boolean = self.declare_fixed_intrinsic_struct(
            "Boolean",
            hir::IntrinsicTypeKind::Boolean,
            self.boolean,
        );
        let intrinsic_string = self.declare_intrinsic_class(
            "String",
            hir::IntrinsicTypeKind::String,
            Vec::new(),
            Vec::new(),
            CanonicalTypePlan::Existing(self.string),
        );
        let intrinsic_array = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
        let intrinsic_mutable_array =
            self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
        hir::Module {
            types: self.types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            callable_references: Arena::new(),
            bound_callable_refs: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: Arena::new(),
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: Arena::new(),
            structs: self.structs,
            struct_applications: self.struct_applications,
            enums: self.enums,
            enum_applications: self.enum_applications,
            classes: self.classes,
            class_applications: self.class_applications,
            interfaces: self.interfaces,
            interface_applications: self.interface_applications,
            interface_methods: self.interface_methods,
            top_level: self.top_level,
            unit: self.unit,
            int: self.int,
            boolean: self.boolean,
            string: self.string,
            option_enum: self.option_enum,
            exception_core,
            coroutine_core,
            ffi_core,
            foreign_callback_core: hir::ForeignCallbackCore {
                callback: ptr,
                mode: callback_mode,
                state: callback_state,
                register: entry,
                retain: entry,
                release: entry,
                query_state: entry,
                failure: entry,
            },
            intrinsic_type_core: hir::IntrinsicTypeCore {
                int: intrinsic_int,
                uint: intrinsic_uint,
                boolean: intrinsic_boolean,
                string: intrinsic_string,
                array: intrinsic_array,
                mutable_array: intrinsic_mutable_array,
            },
            entry,
            instantiations: self.instantiations,
        }
    }
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

fn int_lit(h: &Harness, value: i64) -> hir::Expr {
    expr(hir::ExprKind::IntLiteral(value), h.int)
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
    expr(hir::ExprKind::StructInit { application, args }, ty)
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

#[test]
fn lowers_hello_world() {
    let module = lower(&hello_world());

    // Intrinsics are excluded from `top_level`; declaration order
    // kept: the two core overloads the test uses, then the user
    // functions.
    assert_eq!(module.top_level.len(), 4);
    let helper = &module.functions[module.top_level[2]];
    let main = &module.functions[module.top_level[3]];
    assert_eq!(helper.name, "helper");
    assert_eq!(main.name, "main");

    // Mangling: entry is the fixed `scoop_main`, others `scoop.<name>`.
    assert_eq!(main.symbol, mir::ENTRY_SYMBOL);
    assert_eq!(helper.symbol, "scoop.helper");
    assert_eq!(module.entry, module.top_level[3]);

    // String literals became numbered global constants (in lowering
    // order: function bodies are lowered in declaration order, so
    // core's `println(String)` contributes its `"\n"` first).
    let strings: Vec<(&str, &str)> = module
        .strings
        .iter()
        .map(|(_, s)| (s.value.as_str(), s.symbol.as_str()))
        .collect();
    assert_eq!(
        strings,
        [
            ("\n", "scoop.str.0"),
            ("!", "scoop.str.1"),
            ("hello, world", "scoop.str.2")
        ]
    );

    // M2 meta exists but is empty.
    assert!(module.meta.dispatch_tables.is_empty());

    // Golden dump locks the output structure.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  fun print @scoop.print(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun helper @scoop.helper() -> Unit
    bb0 entry
      call @scoop.print direct
        Type String
        StringConst @scoop.str.1
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      call @scoop.helper direct
      return
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"!\"
  str @scoop.str.2 \"hello, world\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn repeated_literals_get_separate_constants_deterministically() {
    let mut hir_module = hello_world();
    // Add another `println("hello, world")` to `main`. The
    // `println(String)` overload is the second function in
    // `top_level` (after `print(String)`).
    let println = hir_module.top_level[1];
    let string = hir_module.string;
    let unit = hir_module.unit;
    let main_id = hir_module.entry;
    let hir::FunctionKind::User(body) = &mut hir_module.functions[main_id].kind else {
        unreachable!()
    };
    body.statements.push(hir::Statement {
        kind: hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee: hir::Callable::Function(println),
                args: vec![hir::Expr {
                    kind: hir::ExprKind::StringLiteral("hello, world".to_string()),
                    ty: string,
                    span: SPAN,
                }],
            },
            ty: unit,
            span: SPAN,
        }),
        span: SPAN,
    });

    let module = lower(&hir_module);
    let symbols: Vec<&str> = module
        .strings
        .iter()
        .map(|(_, s)| s.symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        ["scoop.str.0", "scoop.str.1", "scoop.str.2", "scoop.str.3"]
    );
}

#[test]
fn formatting_helpers_are_ordinary_extern_calls() {
    let mut h = Harness::new();
    let int_to_string = h.int_to_string();
    let bool_to_string = h.bool_to_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(int_to_string, vec![int_lit(&h, 1)], h.string)),
                expr_stmt(call_typed(
                    bool_to_string,
                    vec![bool_lit(&h, true)],
                    h.string,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let symbols: Vec<&str> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::Extern(function) = call.target.callee else {
                panic!("formatting helpers must remain ordinary extern callees")
            };
            module.extern_functions[function].native_symbol.as_str()
        })
        .collect();
    assert_eq!(
        symbols,
        ["scoop_rt_int_to_string", "scoop_rt_bool_to_string"]
    );
}

// ---- M9: GC intrinsics and generic structs ----

/// The source-level bodies of `pin` / `unpin` / handle operations after
/// inlining their ordinary wrappers: raw runtime call plus explicit
/// handle construction or field extraction.
fn gc_shapes() -> (Harness, hir::FunctionId) {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, uint) = (h.string, h.uint());
    let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
    let gc_handle_s = h.struct_app(gc.gc_handle, vec![string]);
    let pinned_ptr_s_application = h.struct_application_of(pinned_ptr_s);
    let gc_handle_s_application = h.struct_application_of(gc_handle_s);
    let pin_raw = h.instantiate(gc.pin_raw, vec![string]);
    let unpin_raw = h.instantiate(gc.unpin_raw, vec![string]);
    let get_handle_raw = h.instantiate(gc.get_handle_raw, vec![string]);
    let release_handle_raw = h.instantiate(gc.release_handle_raw, vec![string]);
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", string));
    let pin_word = locals.alloc(local("pinWord", uint));
    let ph = locals.alloc(local("ph", pinned_ptr_s));
    let r = locals.alloc(local("r", string));
    let handle_word = locals.alloc(local("handleWord", uint));
    let gh = locals.alloc(local("gh", gc_handle_s));
    let r2 = locals.alloc(local("r2", string));
    let n = locals.alloc(local("n", uint));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    pin_word,
                    generic_call(pin_raw, vec![local_ref(s, string)], uint),
                ),
                val_decl(
                    ph,
                    struct_init(&h, pinned_ptr_s, vec![local_ref(pin_word, uint)]),
                ),
                val_decl(
                    r,
                    generic_call(
                        unpin_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(ph, pinned_ptr_s)),
                                field: hir::FieldRef::StructField {
                                    application: pinned_ptr_s_application,
                                    index: 0,
                                },
                            },
                            uint,
                        )],
                        string,
                    ),
                ),
                val_decl(
                    handle_word,
                    generic_call(get_handle_raw, vec![local_ref(s, string)], uint),
                ),
                val_decl(
                    gh,
                    struct_init(&h, gc_handle_s, vec![local_ref(handle_word, uint)]),
                ),
                val_decl(
                    r2,
                    generic_call(
                        release_handle_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(gh, gc_handle_s)),
                                field: hir::FieldRef::StructField {
                                    application: gc_handle_s_application,
                                    index: 0,
                                },
                            },
                            uint,
                        )],
                        string,
                    ),
                ),
                expr_stmt(call(&h, gc.gc_collect, vec![])),
                val_decl(n, call_typed(gc.gc_stats, vec![], uint)),
            ],
        },
    );
    (h, main)
}

#[test]
fn gc_wrappers_use_explicit_raw_word_marshalling() {
    let (h, main) = gc_shapes();
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    // `_pin(s)` produces the raw word used by `PinnedPtr(raw)`.
    let (call, pin_result) = statement_call(&entry_statements(body)[0]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::Pin)
    ));
    assert!(matches!(call.args.as_slice(), [arg] if matches!(arg.kind, mir::ExprKind::Local(_))));
    let pin_result = pin_result.expect("_pin returns a raw word");
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[1].kind else {
        panic!("PinnedPtr construction is a val decl")
    };
    let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
        panic!("the raw result is wrapped into PinnedPtr")
    };
    assert_eq!(module.structs[*struct_id].name, "PinnedPtr$S");
    assert_eq!(
        module.structs[*struct_id].declared_fields()[0].ty,
        mir::Type::UInt
    );
    assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == pin_result)));

    // `_unpin(ph.raw)` directly initializes the source result local.
    let (call, hidden) = statement_call(&entry_statements(body)[2]);
    let hidden = hidden.expect("_unpin produces the typed result");
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::Unpin)
    ));
    let [arg] = call.args.as_slice() else {
        panic!("unpin takes one argument")
    };
    let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
        panic!("unpin's argument is the handle's raw field")
    };
    assert_eq!(body.locals[hidden].name, "r");

    // `getGcHandle` / `releaseGcHandle` share the wrap / unwrap
    // shapes with their own runtime symbols and handle struct.
    let (call, handle_result) = statement_call(&entry_statements(body)[3]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GetHandle)
    ));
    let handle_result = handle_result.expect("getGcHandle returns a raw word");
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[4].kind else {
        panic!("getGcHandle's statement is a val decl")
    };
    let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
        panic!("getGcHandle's result is wrapped into the handle struct")
    };
    assert_eq!(module.structs[*struct_id].name, "GcHandle$S");
    assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == handle_result)));
    let (call, _) = statement_call(&entry_statements(body)[5]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::ReleaseHandle)
    ));
    let [arg] = call.args.as_slice() else {
        panic!("releaseGcHandle takes one argument")
    };
    let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
        panic!("releaseGcHandle's argument is the handle's raw field")
    };

    // `gcCollect()` is a plain void runtime call; `gcStats()`
    // yields the raw word (`UInt`).
    let (call, destination) = statement_call(&entry_statements(body)[6]);
    assert!(destination.is_none());
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GcCollect)
    ));
    let (call, destination) = statement_call(&entry_statements(body)[7]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GcStats)
    ));
    assert_eq!(
        destination,
        Some(
            *body
                .locals
                .iter()
                .find(|(_, local)| local.name == "n")
                .map(|(id, _)| id)
                .as_ref()
                .expect("n local")
        )
    );
}

#[test]
fn string_plus_lowers_to_runtime_concat() {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", h.string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                s,
                binary(
                    hir::BinOp::Add,
                    str_lit(&h, "a"),
                    str_lit(&h, "b"),
                    h.string,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let (call, destination) = statement_call(&entry_statements(body)[0]);
    assert_eq!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::StringConcat)
    );
    let destination = destination.expect("String concatenation returns String");
    assert_eq!(body.locals[destination].name, "s");
    assert!(matches!(call.args.as_slice(), [left, right]
            if matches!(left.kind, mir::ExprKind::StringConst(_))
                && matches!(right.kind, mir::ExprKind::StringConst(_))));
}

#[test]
fn primitive_operators_map_to_primitive_mir_ops() {
    let mut h = Harness::new();
    let mut statements = Vec::new();
    // Division is not here: its divisor check (M8) makes it a
    // statement sequence — see
    // `division_by_zero_throws_arithmetic_exception`.
    let int_cases = [
        (hir::BinOp::Add, mir::BinOp::IntAdd),
        (hir::BinOp::Sub, mir::BinOp::IntSub),
        (hir::BinOp::Mul, mir::BinOp::IntMul),
        (hir::BinOp::Lt, mir::BinOp::IntLt),
        (hir::BinOp::Le, mir::BinOp::IntLe),
        (hir::BinOp::Gt, mir::BinOp::IntGt),
        (hir::BinOp::Ge, mir::BinOp::IntGe),
    ];
    for (hir_op, _) in &int_cases {
        let ty = if matches!(hir_op, hir::BinOp::Add | hir::BinOp::Sub | hir::BinOp::Mul) {
            h.int
        } else {
            h.boolean
        };
        statements.push(expr_stmt(binary(
            *hir_op,
            int_lit(&h, 1),
            int_lit(&h, 2),
            ty,
        )));
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements,
        },
    );
    let module = lower(&h.finish(main));

    let expected: Vec<mir::BinOp> = int_cases.iter().map(|(_, mir_op)| *mir_op).collect();
    let body = &module.functions[module.entry].body;
    let ops: Vec<mir::BinOp> = entry_statements(body)
        .iter()
        .map(|statement| {
            let mir::StatementKind::Expr(expr) = &statement.kind else {
                panic!("expected an expression statement")
            };
            let mir::ExprKind::Binary { op, .. } = &expr.kind else {
                panic!("expected a binary expression")
            };
            *op
        })
        .collect();
    assert_eq!(ops, expected);
}

#[test]
fn short_circuit_rhs_calls_stay_on_rhs_edges() {
    let mut h = Harness::new();
    let boolean = h.boolean;
    let rhs = h.user_fn_full(
        "rhs",
        Vec::new(),
        Vec::new(),
        boolean,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(bool_lit(&h, true)),
            })],
        },
    );
    let mut locals = Arena::new();
    let and_result = locals.alloc(local("and_result", boolean));
    let or_result = locals.alloc(local("or_result", boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    and_result,
                    binary(
                        hir::BinOp::And,
                        bool_lit(&h, false),
                        call_typed(rhs, Vec::new(), boolean),
                        boolean,
                    ),
                ),
                val_decl(
                    or_result,
                    binary(
                        hir::BinOp::Or,
                        bool_lit(&h, true),
                        call_typed(rhs, Vec::new(), boolean),
                        boolean,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    let rhs_blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|(_, block)| block)
        .filter(|block| block.name.starts_with("logic.rhs"))
        .collect();
    assert_eq!(rhs_blocks.len(), 2);
    for block in rhs_blocks {
        let (call, destination) = statement_call(&block.statements[0]);
        assert_eq!(call.target.callee, mir::Callee::User(module.top_level[0]));
        assert!(destination.is_some());
    }
    assert!(body.blocks.iter().all(|(_, block)| {
        block.name.starts_with("logic.rhs")
            || block
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
    }));

    let mir::Terminator::Branch {
        then_block,
        else_block,
        ..
    } = body.blocks[body.entry].terminator
    else {
        panic!("`&&` must branch to its RHS or short-circuit block")
    };
    assert!(body.blocks[then_block].name.starts_with("logic.rhs"));
    assert!(body.blocks[else_block].name.starts_with("logic.short"));
}

#[test]
fn nested_calls_are_normalized_left_to_right() {
    let mut h = Harness::new();
    let int = h.int;
    let first_result = int_lit(&h, 1);
    let first = h.user_fn_full(
        "first",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(first_result),
            })],
        },
    );
    let second_result = int_lit(&h, 2);
    let second = h.user_fn_full(
        "second",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(second_result),
            })],
        },
    );
    let mut outer_locals = Arena::new();
    let a = outer_locals.alloc(local("a", int));
    let b = outer_locals.alloc(local("b", int));
    let outer = h.user_fn_full(
        "outer",
        Vec::new(),
        vec![param("a", int, a), param("b", int, b)],
        int,
        hir::Body {
            locals: outer_locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(a, int)),
            })],
        },
    );
    let mut locals = Arena::new();
    let result = locals.alloc(local("result", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                result,
                call_typed(
                    outer,
                    vec![
                        call_typed(first, Vec::new(), int),
                        call_typed(second, Vec::new(), int),
                    ],
                    int,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let statements = entry_statements(body);
    assert_eq!(statements.len(), 3);

    let (first_call, first_destination) = statement_call(&statements[0]);
    let first_destination = first_destination.expect("first returns Int");
    assert_eq!(
        first_call.target.callee,
        mir::Callee::User(module.top_level[0])
    );
    let (second_call, second_destination) = statement_call(&statements[1]);
    let second_destination = second_destination.expect("second returns Int");
    assert_eq!(
        second_call.target.callee,
        mir::Callee::User(module.top_level[1])
    );
    let (outer_call, outer_destination) = statement_call(&statements[2]);
    assert_eq!(
        outer_call.target.callee,
        mir::Callee::User(module.top_level[2])
    );
    assert!(matches!(outer_call.args.as_slice(), [first, second]
            if matches!(first.kind, mir::ExprKind::Local(local) if local == first_destination)
                && matches!(second.kind, mir::ExprKind::Local(local) if local == second_destination)));
    let outer_destination = outer_destination.expect("outer returns Int");
    assert_eq!(body.locals[outer_destination].name, "result");
}

#[test]
fn division_by_zero_throws_arithmetic_exception() {
    // val q = 10 / 2 — M8: both operands are evaluated once into
    // hidden locals (left to right), the zero check precedes the
    // division, and a zero divisor throws `ArithmeticException`.
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let int = h.int;
    let mut locals = Arena::new();
    let q = locals.alloc(local("q", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                q,
                binary(hir::BinOp::Div, int_lit(&h, 10), int_lit(&h, 2), int),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  class ArithmeticException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val $div.1: Int
        Type Int
        IntLiteral 10
      val $div.2: Int
        Type Int
        IntLiteral 2
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          Local $div.2
          Type Int
          IntLiteral 0
    bb1 if.then.1
      call $call.1: ArithmeticException = @scoop.ctor.ArithmeticException direct
      throw
        Type ArithmeticException
        Local $call.1
    bb2 if.merge.2
      val q: Int
        Type Int
        Binary IntDiv
          Type Int
          Local $div.1
          Type Int
          Local $div.2
      return
  fun ctor.ArithmeticException @scoop.ctor.ArithmeticException() -> ArithmeticException
    bb0 entry
      return
        Type ArithmeticException
        ClassInit ArithmeticException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn try_and_throw_become_explicit_cfg() {
    // try { throw MyError() } catch (e: MyError) { 1 } finally { 2 }
    // — MIR keeps the structured form (DESIGN 3.3); the
    // control-flow expansion is LIR's job.
    let mut h = Harness::new();
    let my_error = h.exception("MyError");
    let error_ty = h.class_ty(my_error);
    let error_application = h.class_application_of(error_ty);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", error_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Throw(expr(
                    hir::ExprKind::ClassInit {
                        application: error_application,
                        args: Vec::new(),
                    },
                    error_ty,
                )))],
                catches: vec![hir::CatchClause {
                    local: e,
                    ty: error_ty,
                    body: vec![expr_stmt(int_lit(&h, 1))],
                    span: SPAN,
                }],
                finally_body: Some(vec![expr_stmt(int_lit(&h, 2))]),
            }))],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  class MyError vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      goto bb8
    bb1 try.unwind.1
      landing_pad cleanup=false
      goto bb2
    bb2 try.dispatch.2
      begin_catch
      branch bb9 bb10
        Type Boolean
        IsInstance MyError
          Type Any
          CaughtException
    bb3 try.handler_pad.3
      landing_pad cleanup=true
      goto bb4
    bb4 try.handler_cleanup.4
      end_catch
      Type Int
      IntLiteral 2
      resume
    bb5 try.exit_pad.5
      landing_pad cleanup=true
      goto bb6
    bb6 try.exit_cleanup.6
      end_catch
      resume
    bb7 try.end.7
      return
    bb8 try.body.8 unwind bb1
      call $call.1: MyError = @scoop.ctor.MyError direct
      throw unwind bb1
        Type MyError
        Local $call.1
    bb9 try.catch.9
      val e: MyError
        Type MyError
        Retype MyError
          Type Any
          CaughtException
      goto bb11
    bb10 try.next.10 unwind bb5
      Type Int
      IntLiteral 2
      rethrow unwind bb5
    bb11 scope.11 unwind bb3
      Type Int
      IntLiteral 1
      goto bb12
    bb12 scope.12
      end_catch
      Type Int
      IntLiteral 2
      goto bb7
  fun ctor.MyError @scoop.ctor.MyError() -> MyError
    bb0 entry
      return
        Type MyError
        ClassInit MyError
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn unary_operators_map_to_mir_unops() {
    let mut h = Harness::new();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(expr(
                    hir::ExprKind::Unary {
                        op: hir::UnOp::Neg,
                        operand: Box::new(int_lit(&h, 1)),
                    },
                    h.int,
                )),
                expr_stmt(expr(
                    hir::ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(bool_lit(&h, true)),
                    },
                    h.boolean,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let ops: Vec<mir::UnOp> = entry_statements(body)
        .iter()
        .map(|statement| {
            let mir::StatementKind::Expr(expr) = &statement.kind else {
                panic!("expected an expression statement")
            };
            let mir::ExprKind::Unary { op, .. } = &expr.kind else {
                panic!("expected a unary expression")
            };
            *op
        })
        .collect();
    assert_eq!(ops, [mir::UnOp::IntNeg, mir::UnOp::BoolNot]);
}

#[test]
fn field_access_uses_zero_based_indices() {
    let mut h = Harness::new();
    let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
    let point_ty = h.struct_ty(point);
    let point_application = h.struct_application_of(point_ty);
    let pair = h.tuple(&[h.int, h.string]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let t = locals.alloc(local("t", pair));
    let y = locals.alloc(local("y", h.int));
    let s = locals.alloc(local("s", h.string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                // `p.y`
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(local_ref(p, point_ty)),
                            field: hir::FieldRef::StructField {
                                application: point_application,
                                index: 1,
                            },
                        },
                        h.int,
                    ),
                ),
                // `t._2`
                val_decl(
                    s,
                    expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(local_ref(t, pair)),
                            field: hir::FieldRef::TupleIndex(1),
                        },
                        h.string,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    for statement in entry_statements(body) {
        let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(
            init.kind,
            mir::ExprKind::FieldAccess { index: 1, .. }
        ));
    }
}

#[test]
fn control_flow_becomes_cfg() {
    let mut h = Harness::new();
    let println = h.println_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::If {
                    cond: bool_lit(&h, true),
                    then_body: vec![expr_stmt(call(&h, println, vec![str_lit(&h, "a")]))],
                    else_body: Some(vec![expr_stmt(call(&h, println, vec![str_lit(&h, "b")]))]),
                }),
                stmt(hir::StatementKind::While {
                    cond: bool_lit(&h, false),
                    body: vec![],
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Branch { .. }
    ));
    assert_eq!(block_named(body, "if.then").statements.len(), 1);
    assert_eq!(block_named(body, "if.else").statements.len(), 1);
    assert!(matches!(
        block_named(body, "while.cond").terminator,
        mir::Terminator::Branch { .. }
    ));
    assert!(block_named(body, "while.body").statements.is_empty());
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
