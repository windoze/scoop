
use super::*;
use scoop_ast::Span;
use scoop_hir as hir;

fn lower(module: &hir::Module) -> mir::Module {
    let concrete = scoop_hir_lower::concretize_export(module);
    super::lower(&concrete)
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

/// HIR module shell as hir-lower produces it: well-known types,
/// core's managed `write` extern, two conversion intrinsics, and the ordinary
/// `print` / `println` overloads (M7), plus core's `Option` enum
/// allocated first.
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
    int_to_string: hir::FunctionId,
    bool_to_string: hir::FunctionId,
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
        let mut functions = Arena::new();
        // scoop.core's managed output extern and conversion intrinsics:
        // `@Extern(name = "scoop_rt_write", abi = "scoop") fun write(...)`,
        // `@Intrinsic("rt_int_to_string") fun intToString(...)`,
        // `@Intrinsic("rt_bool_to_string") fun boolToString(...)`.
        let extern_functions = Arena::new();
        let int_to_string = functions.alloc(hir::Function {
            name: "intToString".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: string,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::IntToString,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
        let bool_to_string = functions.alloc(hir::Function {
            name: "boolToString".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: Vec::new(),
            return_ty: string,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::BoolToString,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
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
            top_level: vec![int_to_string, bool_to_string],
            unit,
            int,
            boolean,
            string,
            option_enum,
            write: None,
            int_to_string,
            bool_to_string,
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
        let (write, int_to_string) = (self.write(), self.int_to_string);
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
        let (write, bool_to_string) = (self.write(), self.bool_to_string);
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
        let int_to_string = self.int_to_string;
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
        let bool_to_string = self.bool_to_string;
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
        let interface_implementations = self.interface_implementation_shells(&interfaces);
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

    /// A concrete zero-argument exception shell used by tests that
    /// exercise compiler-generated exception edges.
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
                hir::ClassModifier::Abstract,
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
        function
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
/// annotates every expression; core's overloads call the
/// String-returning conversion intrinsics).
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
fn suspend_leaf_uses_typed_hidden_abi_and_completed_step() {
    let mut h = Harness::new();
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 42)),
            })],
        },
    );
    h.functions[leaf].is_suspend = true;
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );

    let module = lower(&h.finish_coroutines(main));
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .next()
        .expect("one transformed suspend function");
    let function = &module.functions[coroutine.function];
    assert_eq!(function.symbol, "scoop.leaf$suspend");
    assert_eq!(function.params.len(), 1);
    let mir::Type::Interface(continuation) = function.params[0].ty else {
        panic!("hidden completion must be a concrete Continuation<Int>")
    };
    assert_eq!(module.interfaces[continuation].name, "Continuation$I");

    let step = &module.meta.coroutine_steps[coroutine.step];
    assert_eq!(step.result, mir::Type::Int);
    let step_def = &module.enums[step.enum_id];
    assert!(step_def.gc_free);
    assert!(step_def.variants.iter().all(|variant| variant.gc_free));
    assert_eq!(
        function.return_ty,
        mir::Type::Enum(step.enum_id, Vec::new())
    );
    assert_eq!(step_def.variants[0].name, "Completed");
    let mir::Terminator::Return { value: Some(value) } =
        &function.body.blocks[function.body.entry].terminator
    else {
        panic!("leaf returns a completed step")
    };
    assert!(matches!(
        &value.kind,
        mir::ExprKind::VariantConstruct {
            variant: 0,
            fields,
            ..
        } if matches!(fields.as_slice(), [field] if matches!(field.kind, mir::ExprKind::IntLiteral(42)))
    ));
}

#[test]
fn suspend_call_generates_a_liveness_based_frame_and_resume_point() {
    let mut h = Harness::new();
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 41)),
            })],
        },
    );
    h.functions[leaf].is_suspend = true;
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", h.int));
    let caller = h.user_fn_full(
        "caller",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(value, call_typed(leaf, Vec::new(), h.int)),
                stmt(hir::StatementKind::Return {
                    value: Some(binary(
                        hir::BinOp::Add,
                        local_ref(value, h.int),
                        int_lit(&h, 1),
                        h.int,
                    )),
                }),
            ],
        },
    );
    h.functions[caller].is_suspend = true;
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );

    let module = lower(&h.finish_coroutines(main));
    let (_, caller) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == "caller")
        .expect("caller coroutine metadata");
    let mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points,
    } = &caller.lowering
    else {
        panic!("a suspend call requires a state machine")
    };
    assert_eq!(resume_points.len(), 1);
    let frame = &module.meta.coroutine_frames[*frame];
    let fields = module.classes[frame.class].declared_fields();
    assert_eq!(fields[0].name, "state");
    assert_eq!(fields[1].name, "completion");
    assert_eq!(
        fields
            .iter()
            .filter(|field| field.name == "local$value")
            .count(),
        1
    );
    assert!(
        module.functions[*driver]
            .body
            .blocks
            .iter()
            .any(|(_, block)| block.name == "coroutine.resume.1")
    );
    let int_slot = module
        .enums
        .iter()
        .find_map(|(_, definition)| (definition.name == "CoroutineSlot$I").then_some(definition))
        .expect("live Int local uses a concrete coroutine slot");
    assert!(int_slot.gc_free);
    assert!(int_slot.variants.iter().all(|variant| variant.gc_free));
    let throwable = module
        .classes
        .iter()
        .find_map(|(id, definition)| (definition.name == "Throwable").then_some(id))
        .expect("Throwable class");
    let throwable_slot = module
        .enums
        .iter()
        .find_map(|(_, definition)| {
            (definition.name.starts_with("CoroutineSlot$")
                && definition.variants.get(1).is_some_and(|variant| {
                    variant.fields.len() == 1 && variant.fields[0].ty == mir::Type::Class(throwable)
                }))
            .then_some(definition)
        })
        .expect("the failure latch uses a concrete Throwable slot");
    assert!(!throwable_slot.gc_free);
    assert!(throwable_slot.variants[0].gc_free);
    assert!(!throwable_slot.variants[1].gc_free);
    let point = &module.meta.coroutine_resume_points[resume_points[0]];
    assert_eq!(point.result, mir::Type::Int);
    assert_eq!(point.state, 1);
    assert_eq!(module.classes[point.adapter].interfaces.len(), 1);
}

#[test]
fn start_coroutine_resumes_only_an_immediately_completed_task() {
    let mut h = Harness::new();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let mut hir_module = h.finish_coroutines(main);
    let result = hir_module.int;
    let suspend_task = hir_module.coroutine_core.suspend_task;
    let continuation = hir_module.coroutine_core.continuation;
    let task_ty = module_interface_application(&mut hir_module, suspend_task, vec![result]);
    let completion_ty = module_interface_application(&mut hir_module, continuation, vec![result]);
    let mut locals = Arena::new();
    let task = locals.alloc(local("task", task_ty));
    let completion = locals.alloc(local("completion", completion_ty));
    let Some(start_generic) =
        hir_module.functions[hir_module.coroutine_core.start_coroutine].generic_definition()
    else {
        panic!("startCoroutine is generic")
    };
    let start = hir_module
        .instantiations
        .alloc(hir::ResolvedGenericFunction {
            generic: start_generic,
            type_args: vec![result],
        });
    let launcher = hir_module.functions.alloc(hir::Function {
        name: "launcher".to_string(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: false,
        params: vec![
            param("task", task_ty, task),
            param("completion", completion_ty, completion),
        ],
        return_ty: hir_module.unit,
        attributes: hir::FunctionAttributes::default(),
        kind: hir::FunctionKind::User(hir::Body {
            locals,
            statements: vec![expr_stmt(expr(
                hir::ExprKind::Call {
                    callee: hir::Callable::Generic(start),
                    args: vec![
                        local_ref(task, task_ty),
                        local_ref(completion, completion_ty),
                    ],
                },
                hir_module.unit,
            ))],
        }),
        method: None,
        span: SPAN,
    });
    hir_module.top_level.push(launcher);

    let module = lower(&hir_module);
    let launcher = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "launcher").then_some(function))
        .expect("launcher is lowered");
    let launcher_entry = &launcher.body.blocks[launcher.body.entry];
    let (start, destination) = statement_call(&launcher_entry.statements[0]);
    assert!(destination.is_none(), "startCoroutine returns Unit");
    let mir::Callee::User(helper) = start.target.callee else {
        panic!("startCoroutine lowers to its concrete guarded helper")
    };
    let helper = &module.functions[helper];
    let entry = &helper.body.blocks[helper.body.entry];
    let (run, step_local) = statement_call(&entry.statements[0]);
    let mir::CallKind::Interface {
        interface: task_interface,
        slot: 0,
    } = run.target.kind
    else {
        panic!("startCoroutine must invoke SuspendTask<T>.run through interface dispatch")
    };
    assert_eq!(module.interfaces[task_interface].name, "SuspendTask$I");
    assert_eq!(run.args.len(), 2, "run receives task and hidden completion");
    let step_local = step_local.expect("run returns a CoroutineStep<T>");
    let mir::Terminator::Branch {
        then_block: completed,
        else_block: suspended,
        ..
    } = entry.terminator
    else {
        panic!("startCoroutine must distinguish Completed from Suspended")
    };

    let completed = &helper.body.blocks[completed];
    let (resume, destination) = statement_call(&completed.statements[0]);
    assert!(destination.is_none(), "Continuation.resume returns Unit");
    let mir::CallKind::Interface {
        interface: continuation_interface,
        slot: 0,
    } = resume.target.kind
    else {
        panic!("completed task must resume its outer continuation")
    };
    assert_eq!(
        module.interfaces[continuation_interface].name,
        "Continuation$I"
    );
    assert!(matches!(resume.args.as_slice(), [completion, field]
            if matches!(completion.kind, mir::ExprKind::Local(_))
                && matches!(&field.kind, mir::ExprKind::EnumField {
                    operand,
                    variant: 0,
                    index: 0
                } if matches!(operand.kind, mir::ExprKind::Local(local) if local == step_local))));

    let suspended = &helper.body.blocks[suspended];
    assert!(suspended.statements.is_empty());
    assert!(matches!(
        suspended.terminator,
        mir::Terminator::Return { value: None }
    ));
    let catch_pad = entry
        .unwind
        .expect("task body exceptions enter the guarded helper pad");
    assert!(
        helper.body.blocks[catch_pad]
            .statements
            .iter()
            .any(|statement| {
                matches!(
                    &statement.kind,
                    mir::StatementKind::Call(mir::CallEffect::Value { call, .. })
                        if call.target.callee
                            == mir::Callee::Runtime(mir::RuntimeFn::MaterializeException)
                )
            })
    );
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
fn typed_intrinsic_kinds_map_to_runtime_functions() {
    // core's `print` / `println` overloads are ordinary user
    // functions (their forwarding is locked by the golden dumps);
    // only the two conversion intrinsics map onto runtime
    // functions, by validated intrinsic kind.
    let mut h = Harness::new();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(h.int_to_string, vec![int_lit(&h, 1)], h.string)),
                expr_stmt(call_typed(
                    h.bool_to_string,
                    vec![bool_lit(&h, true)],
                    h.string,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let shims: Vec<mir::RuntimeFn> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::Runtime(function) = call.target.callee else {
                panic!("expected a runtime callee")
            };
            function
        })
        .collect();
    assert_eq!(
        shims,
        [mir::RuntimeFn::IntToString, mir::RuntimeFn::BoolToString,]
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
fn generic_structs_instantiate_per_argument_list() {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, uint) = (h.string, h.uint());
    // Two applications of one generic struct, one of them twice
    // (dedup), plus a struct whose field mentions its type
    // parameter (the general substitution path).
    let pinned_ptr_v = h.struct_app(gc.pinned_ptr, vec![uint]);
    let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let box2 = h.declare_struct("Box2", vec![type_param("T")], vec![t], &[("x", t)], &[]);
    let box2_s = h.struct_app(box2, vec![string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", pinned_ptr_v));
    let b = locals.alloc(local("b", pinned_ptr_s));
    let c = locals.alloc(local("c", pinned_ptr_s));
    let d = locals.alloc(local("d", box2_s));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(a, struct_init(&h, pinned_ptr_v, vec![int_lit(&h, 1)])),
                val_decl(b, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 2)])),
                val_decl(c, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 3)])),
                val_decl(d, struct_init(&h, box2_s, vec![str_lit(&h, "x")])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // One instance per (struct, args): `PinnedPtr$V` once,
    // `PinnedPtr$S` once despite two uses, `Box2$S` once — named
    // like the enum instances. Generic definitions themselves do
    // not survive into MIR: MIR contains no generic types.
    let defs = |name: &str| {
        module
            .structs
            .iter()
            .filter(|(_, def)| def.name == name)
            .map(|(_, def)| def)
            .collect::<Vec<_>>()
    };
    assert!(defs("PinnedPtr").is_empty());
    assert!(defs("Box2").is_empty());
    assert_eq!(defs("PinnedPtr$V").len(), 1);
    assert_eq!(
        defs("PinnedPtr$V")[0].declared_fields()[0].ty,
        mir::Type::UInt
    );
    assert!(defs("PinnedPtr$V")[0].gc_free);
    assert_eq!(defs("PinnedPtr$S").len(), 1);
    assert!(defs("PinnedPtr$S")[0].gc_free);
    assert_eq!(defs("Box2$S").len(), 1);
    // Field substitution: `Box2<String>`'s `x` is `String`.
    assert_eq!(defs("Box2$S")[0].declared_fields()[0].ty, mir::Type::String);
    assert!(!defs("Box2$S")[0].gc_free);

    // Locals and StructInits resolve to the instances.
    let body = &module.functions[module.entry].body;
    let instance_of = |local: mir::LocalId| {
        let mir::Type::Struct(id) = &body.locals[local].ty else {
            panic!("a struct local")
        };
        module.structs[*id].name.as_str()
    };
    let mir::StatementKind::ValDecl { local: la, .. } = entry_statements(body)[0].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: lb, .. } = entry_statements(body)[1].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: lc, .. } = entry_statements(body)[2].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: ld, .. } = entry_statements(body)[3].kind else {
        panic!()
    };
    assert_eq!(instance_of(la), "PinnedPtr$V");
    assert_eq!(instance_of(lb), "PinnedPtr$S");
    assert_eq!(instance_of(lc), "PinnedPtr$S");
    assert_eq!(instance_of(ld), "Box2$S");
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
        panic!()
    };
    let mir::ExprKind::StructInit { struct_id, .. } = init.kind else {
        panic!("a struct construction")
    };
    assert_eq!(module.structs[struct_id].name, "PinnedPtr$V");
}

#[test]
fn generic_interface_applications_get_distinct_mir_identities() {
    let mut h = Harness::new();
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let interface = h.declare_interface(
        "Channel",
        vec![hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: "T".to_string(),
            variance: hir::Variance::Out,
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        }],
        vec![t],
        Vec::new(),
    );
    let (int, string) = (h.int, h.string);
    let int_channel = h.interface_app(interface, vec![int]);
    let string_channel = h.interface_app(interface, vec![string]);
    h.declare_class(
        "Ints",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![int_channel],
    );
    h.declare_class(
        "Strings",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![string_channel],
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    let names: Vec<_> = module
        .interfaces
        .iter()
        .map(|(_, interface)| interface.name.as_str())
        .collect();
    assert_eq!(names, ["Channel$I", "Channel$S"]);
    let class_interfaces: Vec<_> = module
        .classes
        .iter()
        .filter(|(_, class)| class.name == "Ints" || class.name == "Strings")
        .map(|(_, class)| class.interfaces[0])
        .collect();
    assert_ne!(class_interfaces[0], class_interfaces[1]);
}

#[test]
fn print_overloads_are_ordinary_calls() {
    // M7: calls to core's `print` / `println` overloads resolve to
    // the overload's own MIR function (`Callee::User`); only the
    // intrinsic primitives inside their bodies are runtime calls.
    let mut h = Harness::new();
    let print_string = h.print_string();
    let print_int = h.print_int();
    let print_boolean = h.print_boolean();
    let println_string = h.println_string();
    let println_int = h.println_int();
    let println_boolean = h.println_boolean();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call(&h, print_string, vec![str_lit(&h, "s")])),
                expr_stmt(call(&h, print_int, vec![int_lit(&h, 1)])),
                expr_stmt(call(&h, print_boolean, vec![bool_lit(&h, true)])),
                expr_stmt(call(&h, println_string, vec![str_lit(&h, "t")])),
                expr_stmt(call(&h, println_int, vec![int_lit(&h, 2)])),
                expr_stmt(call(&h, println_boolean, vec![bool_lit(&h, false)])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let callees: Vec<mir::FunctionId> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::User(id) = call.target.callee else {
                panic!("print/println calls must be ordinary user calls")
            };
            id
        })
        .collect();
    // The overloads are the first six MIR functions (declaration
    // order: the three `print`s, then the three `println`s), and
    // each overload's symbol carries the parameter encoding.
    assert_eq!(callees, module.top_level[..6]);
    let symbols: Vec<&str> = callees
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        [
            "scoop.print.S",
            "scoop.print.I",
            "scoop.print.B",
            "scoop.println.S",
            "scoop.println.I",
            "scoop.println.B",
        ]
    );
}

/// `fun <name>(<params>): String = <text>` — one overload each.
fn string_fn(
    h: &mut Harness,
    name: &str,
    params: &[(&str, hir::TypeId)],
    text: &str,
) -> hir::FunctionId {
    let string = h.string;
    let mut locals = Arena::new();
    let params: Vec<hir::Param> = params
        .iter()
        .map(|(name, ty)| {
            let local_id = locals.alloc(local(name, *ty));
            param(name, *ty, local_id)
        })
        .collect();
    let init = str_lit(h, text);
    h.user_fn_full(
        name,
        Vec::new(),
        params,
        string,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return { value: Some(init) })],
        },
    )
}

fn top_level_symbols(module: &mir::Module) -> Vec<&str> {
    module
        .top_level
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect()
}

fn instance_id(module: &mir::Module, function: mir::FunctionId) -> mir::MonomorphizedFunctionId {
    module
        .meta
        .instances
        .iter()
        .find_map(|(id, instance)| (instance.function == function).then_some(id))
        .expect("function must have monomorphization metadata")
}

#[test]
fn overloads_mangle_with_param_encoding() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    string_fn(&mut h, "show", &[("v", int)], "int");
    string_fn(&mut h, "show", &[("v", string)], "string");
    string_fn(&mut h, "show", &[("v", int), ("extra", int)], "two");
    // A unique name keeps the plain `scoop.<name>` symbol.
    string_fn(&mut h, "helper", &[], "h");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        [
            "scoop.show.I",
            "scoop.show.S",
            "scoop.show.I_I",
            "scoop.helper",
            "scoop_main"
        ]
    );
}

#[test]
fn zero_parameter_overload_mangles_with_an_empty_encoding() {
    let mut h = Harness::new();
    let int = h.int;
    string_fn(&mut h, "f", &[], "none");
    string_fn(&mut h, "f", &[("v", int)], "one");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        ["scoop.f.", "scoop.f.I", "scoop_main"]
    );
}

#[test]
fn overload_symbols_do_not_collide_with_instance_symbols() {
    // `show(Int)` / `show(String)` overloads plus a generic
    // `show<T>` instantiated with `Int`: `.` vs `$` keep the
    // symbols distinct.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    string_fn(&mut h, "show", &[("v", int)], "int");
    string_fn(&mut h, "show", &[("v", string)], "string");
    let generic = identity_fn(&mut h, "show");
    let generic_int = h.instantiate(generic, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(
                generic_int,
                vec![int_lit(&h, 1)],
                int,
            ))],
        },
    );
    let module = lower(&h.finish(main));

    let symbols = top_level_symbols(&module);
    for expected in ["scoop.show.I", "scoop.show.S", "scoop.show$I"] {
        assert!(
            symbols.contains(&expected),
            "missing {expected} in {symbols:?}"
        );
    }
}

#[test]
fn method_overloads_mangle_with_param_encoding() {
    // `class Doc { fun describe(v: Int); fun describe(v: String) }`:
    // the receiver is not part of the overload encoding.
    let mut h = Harness::new();
    let (int, string, unit) = (h.int, h.string, h.unit);
    let doc = h.class("Doc", hir::ClassModifier::Final, &[], None, &[]);
    let doc_ty = h.class_ty(doc);
    for ty in [int, string] {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", doc_ty));
        let v = locals.alloc(local("v", ty));
        h.method_fn(
            "Doc.describe",
            doc_ty,
            vec![param("this", doc_ty, this), param("v", ty, v)],
            unit,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        );
    }
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let symbols: std::collections::HashSet<&str> = module
        .functions
        .iter()
        .map(|(_, f)| f.symbol.as_str())
        .collect();
    assert!(symbols.contains("scoop.Doc.describe.I"));
    assert!(symbols.contains("scoop.Doc.describe.S"));
    // Each overload gets its own vtable slot (keyed by signature),
    // referencing the final (overload-encoded) symbol by id.
    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe.I");
    assert_eq!(slot_fn(&module, &doc_def.vtable[1]), "scoop.Doc.describe.S");
}

/// A class method returning an Int constant:
/// `fun <owner>.<name>(v: <param_ty>): Int = <value>` (param
/// optional). Returns the HIR function id.
fn int_method(
    h: &mut Harness,
    qualified: &str,
    receiver: hir::TypeId,
    param_ty: Option<hir::TypeId>,
    value: i64,
) -> hir::FunctionId {
    let int = h.int;
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", receiver));
    let mut params = vec![param("this", receiver, this)];
    if let Some(ty) = param_ty {
        let v = locals.alloc(local("v", ty));
        params.push(param("v", ty, v));
    }
    h.method_fn(
        qualified,
        receiver,
        params,
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(h, value)),
            })],
        },
    )
}

#[test]
fn overridden_overload_replaces_the_base_slot_in_place() {
    // open class A { fun s(v: Int): Int = 1; fun s(v: String): Int = 2 }
    // class B : A() { override fun s(v: Int): Int = 3 }
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
    let a_ty = h.class_ty(a);
    int_method(&mut h, "A.s", a_ty, Some(int), 1);
    int_method(&mut h, "A.s", a_ty, Some(string), 2);
    let b = h.class(
        "B",
        hir::ClassModifier::Final,
        &[],
        Some((a, Vec::new())),
        &[],
    );
    let b_ty = h.class_ty(b);
    int_method(&mut h, "B.s", b_ty, Some(int), 3);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    // A: one slot per overload.
    let a_def = &module.classes[class_index(0)];
    assert_eq!(a_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &a_def.vtable[0]), "scoop.A.s.I");
    assert_eq!(slot_fn(&module, &a_def.vtable[1]), "scoop.A.s.S");
    // B: the `s(Int)` override replaces slot 0 in place; the
    // inherited `s(String)` keeps slot 1. (`B.s` is a unique name
    // in the module, so it keeps the plain symbol.)
    let b_def = &module.classes[class_index(1)];
    assert_eq!(b_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &b_def.vtable[0]), "scoop.B.s");
    assert_eq!(slot_fn(&module, &b_def.vtable[1]), "scoop.A.s.S");
}

#[test]
fn virtual_calls_annotate_the_overloads_own_slot() {
    // `val a: A = ...; a.s(1); a.s("x")` — the callee is the
    // signature resolved on the static type; each call annotates
    // its own overload's slot.
    let mut h = Harness::new();
    let (int, string, unit) = (h.int, h.string, h.unit);
    let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
    let a_ty = h.class_ty(a);
    let s_int = int_method(&mut h, "A.s", a_ty, Some(int), 1);
    let s_string = int_method(&mut h, "A.s", a_ty, Some(string), 2);
    let s_int = h.method_application(s_int);
    let s_string = h.method_application(s_string);
    let method_call =
        |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                    args: vec![arg],
                },
                unit,
            )
        };
    let mut locals = Arena::new();
    let av = locals.alloc(local("a", a_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(av, a_ty), s_int, int_lit(&h, 1))),
                expr_stmt(method_call(local_ref(av, a_ty), s_string, str_lit(&h, "x"))),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
    assert!(matches!(call_kind(1), mir::CallKind::Virtual { slot: 1 }));
}

/// `interface <name> { fun m(v: T)... }` — one `MethodSig` per
/// `(name, param type)` entry, as hir-lower materializes them
/// (interface methods carry no `this` in the signature).
fn overloaded_interface(
    h: &mut Harness,
    name: &str,
    methods: &[(&str, hir::TypeId)],
) -> hir::InterfaceId {
    let unit = h.unit;
    let mut locals = Arena::new();
    let methods = methods
        .iter()
        .map(|(name, ty)| {
            let v = locals.alloc(local("v", *ty));
            hir::MethodSig {
                name: name.to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("v", *ty, v)],
                return_ty: unit,
                span: SPAN,
            }
        })
        .collect();
    h.declare_interface(name, Vec::new(), Vec::new(), methods)
}

#[test]
fn overloaded_interface_methods_get_one_itable_slot_each() {
    // interface Multi { fun m(v: Int); fun m(v: String) }
    // class C : Multi implements both overloads.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let c = h.class("C", hir::ClassModifier::Final, &[], None, &[multi]);
    let c_ty = h.class_ty(c);
    int_method(&mut h, "C.m", c_ty, Some(int), 1);
    int_method(&mut h, "C.m", c_ty, Some(string), 2);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let c_def = &module.classes[class_index(0)];
    assert_eq!(c_def.itables.len(), 1);
    let record = &c_def.itables[0];
    assert_eq!(record.slots.len(), 2);
    assert_eq!(slot_fn(&module, &record.slots[0]), "scoop.C.m.I");
    assert_eq!(slot_fn(&module, &record.slots[1]), "scoop.C.m.S");
}

#[test]
fn interface_calls_annotate_the_overloads_own_slot() {
    // `val i: Multi = ...; i.m(1); i.m("x")` — interface dispatch
    // locates the slot by the callee's signature.
    let mut h = Harness::new();
    let (int, string, unit) = (h.int, h.string, h.unit);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let multi_ty = h.interface_ty(multi);
    // Interface method shells, as hir-lower materializes them
    // (params include `this`).
    let shell = |h: &mut Harness, ty: hir::TypeId| {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", multi_ty));
        let v = locals.alloc(local("v", ty));
        h.method_fn(
            "Multi.m",
            multi_ty,
            vec![param("this", multi_ty, this), param("v", ty, v)],
            unit,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        )
    };
    let m_int = shell(&mut h, int);
    let m_string = shell(&mut h, string);
    let m_int = h.method_application(m_int);
    let m_string = h.method_application(m_string);
    let method_call =
        |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                    args: vec![arg],
                },
                unit,
            )
        };
    let mut locals = Arena::new();
    let i = locals.alloc(local("i", multi_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(i, multi_ty), m_int, int_lit(&h, 1))),
                expr_stmt(method_call(
                    local_ref(i, multi_ty),
                    m_string,
                    str_lit(&h, "x"),
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    let is_iface_slot = |kind: &mir::CallKind, slot: u32| matches!(kind, mir::CallKind::Interface { slot: s, .. } if *s == slot);
    assert!(is_iface_slot(call_kind(0), 0));
    assert!(is_iface_slot(call_kind(1), 1));
}

#[test]
fn boxed_thunks_of_overloaded_interface_methods_are_disambiguated() {
    // struct S : Multi implements both `m` overloads; boxing to
    // `Multi` generates one thunk per signature.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let multi_ty = h.interface_ty(multi);
    let s = h.strukt_with("S", &[("x", int)], &[multi]);
    let s_ty = h.struct_ty(s);
    int_method(&mut h, "S.m", s_ty, Some(int), 1);
    int_method(&mut h, "S.m", s_ty, Some(string), 2);
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", multi_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                d,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    multi_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.slots.len(), 2);
    assert_eq!(
        slot_fn(&module, &record.slots[0]),
        "scoop.thunk.D1_SX.Multi.m.I"
    );
    assert_eq!(
        slot_fn(&module, &record.slots[1]),
        "scoop.thunk.D1_SX.Multi.m.S"
    );
    // Each thunk tail-calls its own overload.
    let thunk_target = |slot: &mir::TableSlot| {
        let symbol = slot_fn(&module, slot);
        let thunk = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == symbol)
            .expect("the thunk is a MIR function");
        let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
        let mir::Callee::User(target) = call.target.callee else {
            panic!("the thunk calls a user function")
        };
        module.functions[target].symbol.clone()
    };
    assert_eq!(thunk_target(&record.slots[0]), "scoop.S.m.I");
    assert_eq!(thunk_target(&record.slots[1]), "scoop.S.m.S");
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

#[test]
fn params_and_return_translate() {
    let mut h = Harness::new();
    let int = h.int;
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", int));
    let y = locals.alloc(local("y", int));
    // fun add(x: Int, y: Int): Int { return x + y }
    let add = h.user_fn_full(
        "add",
        Vec::new(),
        vec![param("x", int, x), param("y", int, y)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(binary(
                    hir::BinOp::Add,
                    local_ref(x, int),
                    local_ref(y, int),
                    int,
                )),
            })],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(call(
                &h,
                add,
                vec![int_lit(&h, 1), int_lit(&h, 2)],
            ))],
        },
    );
    let module = lower(&h.finish(main));

    let add_fn = &module.functions[module.top_level[0]];
    assert_eq!(add_fn.symbol, "scoop.add");
    assert_eq!(add_fn.params.len(), 2);
    assert_eq!(add_fn.params[0].ty, mir::Type::Int);
    assert_eq!(add_fn.params[1].ty, mir::Type::Int);
    assert_eq!(add_fn.return_ty, mir::Type::Int);
    // Parameters are (the first) locals of the body.
    let px = add_fn.params[0].local;
    assert_eq!(add_fn.body.locals[px].name, "x");
    assert!(matches!(
        &add_fn.body.blocks[add_fn.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Binary { op: mir::BinOp::IntAdd, .. })
    ));
}

#[test]
fn monomorphizes_generic_functions() {
    let mut h = Harness::new();
    let identity = identity_fn(&mut h, "identity");
    let (int, string) = (h.int, h.string);
    let identity_int = h.instantiate(identity, vec![int]);
    let identity_string = h.instantiate(identity, vec![string]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 41)], int)),
                expr_stmt(generic_call(
                    identity_string,
                    vec![str_lit(&h, "hi")],
                    string,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // main first (declaration order), then the instances in
    // creation order. The generic function itself has no MIR body.
    assert_eq!(module.top_level.len(), 3);
    let int_instance = &module.functions[module.top_level[1]];
    let string_instance = &module.functions[module.top_level[2]];
    assert_eq!(int_instance.symbol, "scoop.identity$I");
    assert_eq!(string_instance.symbol, "scoop.identity$S");

    // The instance signature, locals and body are fully
    // substituted — no `Param` survives.
    assert_eq!(int_instance.params.len(), 1);
    assert_eq!(int_instance.params[0].ty, mir::Type::Int);
    assert_eq!(int_instance.return_ty, mir::Type::Int);
    let x = int_instance.params[0].local;
    assert_eq!(int_instance.body.locals[x].ty, mir::Type::Int);
    assert!(matches!(
        &int_instance.body.blocks[int_instance.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Local(local) if local == x)
    ));
    assert_eq!(string_instance.params[0].ty, mir::Type::String);
    assert_eq!(string_instance.return_ty, mir::Type::String);

    // MIR gives every materialized body its own typed identity and
    // records symbol -> generic source provenance in the meta.
    assert_eq!(module.meta.instances.len(), 2);
    let int_meta = &module.meta.instances[instance_id(&module, module.top_level[1])];
    assert_eq!(int_meta.symbol, "scoop.identity$I");
    assert_eq!(int_meta.source, "identity");
    assert_eq!(int_meta.type_args, vec![mir::Type::Int]);

    // The calls in main resolve to the two instances.
    let main_fn = &module.functions[module.entry];
    for (statement, instance) in entry_statements(&main_fn.body)
        .iter()
        .zip([module.top_level[1], module.top_level[2]])
    {
        let (call, _) = statement_call(statement);
        assert_eq!(
            call.target.callee,
            mir::Callee::Monomorphized(instance_id(&module, instance))
        );
    }
}

#[test]
fn duplicate_requests_produce_one_instance() {
    let mut h = Harness::new();
    let identity = identity_fn(&mut h, "identity");
    let int = h.int;
    let identity_int = h.instantiate(identity, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 1)], int)),
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 2)], int)),
            ],
        },
    );
    // HIR dedups its list, but be robust: the same request listed
    // twice, plus two calls with the same type arguments.
    assert_eq!(h.instantiate(identity, vec![int]), identity_int);
    let module = lower(&h.finish(main));

    assert_eq!(module.top_level.len(), 2);
    let instance = module.top_level[1];
    let main_fn = &module.functions[module.entry];
    for statement in entry_statements(&main_fn.body) {
        let (call, _) = statement_call(statement);
        assert_eq!(
            call.target.callee,
            mir::Callee::Monomorphized(instance_id(&module, instance))
        );
    }
}

#[test]
fn nested_generic_calls_extend_the_worklist() {
    let mut h = Harness::new();
    // fun <T> inner(x: T): T { return x }
    let inner = identity_fn(&mut h, "inner");
    // fun <T> forward(x: T): T { return inner(x) }
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", t));
    let inner_t = h.instantiate(inner, vec![t]);
    let forward = h.user_fn_full(
        "forward",
        vec!["T".to_string()],
        vec![param("x", t, x)],
        t,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(generic_call(inner_t, vec![local_ref(x, t)], t)),
            })],
        },
    );
    let int = h.int;
    let forward_int = h.instantiate(forward, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(
                forward_int,
                vec![int_lit(&h, 1)],
                int,
            ))],
        },
    );
    // The nested request is parameterized in export HIR's list;
    // local-concrete HIR resolves it while materializing forward$I.
    let module = lower(&h.finish(main));

    // main, forward$I, then inner$I (discovered via the worklist).
    assert_eq!(module.top_level.len(), 3);
    let forward_i = &module.functions[module.top_level[1]];
    let inner_i = &module.functions[module.top_level[2]];
    assert_eq!(forward_i.symbol, "scoop.forward$I");
    assert_eq!(inner_i.symbol, "scoop.inner$I");
    let (call, destination) = statement_call(&entry_statements(&forward_i.body)[0]);
    let destination = destination.expect("inner$I returns Int");
    assert_eq!(
        call.target.callee,
        mir::Callee::Monomorphized(instance_id(&module, module.top_level[2]))
    );
    assert!(matches!(
        &forward_i.body.blocks[forward_i.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Local(local) if local == destination)
    ));
    assert_eq!(inner_i.params[0].ty, mir::Type::Int);
    assert_eq!(inner_i.return_ty, mir::Type::Int);
}

#[test]
fn instance_symbols_encode_enum_and_tuple_arguments() {
    let mut h = Harness::new();
    let f = identity_fn(&mut h, "f");
    let (int, string) = (h.int, h.string);
    let option_int = h.option(int);
    let pair = h.tuple(&[int, string]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.instantiate(f, vec![option_int]);
    h.instantiate(f, vec![pair]);
    let module = lower(&h.finish(main));

    let symbols: Vec<&str> = module.top_level[1..]
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    // An enum argument encodes the category, length-delimited
    // instance name, and complete argument list (`mir::encode_type`).
    assert_eq!(symbols, ["scoop.f$E8_Option$IAIX", "scoop.f$TI_SX"]);
    // Substitution recurses into enum / tuple types.
    let option_instance = &module.functions[module.top_level[1]];
    let mir::Type::Enum(enum_id, args) = &option_instance.params[0].ty else {
        panic!("the Option<Int> instance parameter must be an enum type")
    };
    assert_eq!(module.enums[*enum_id].name, "Option$I");
    assert_eq!(args.as_slice(), &[mir::Type::Int]);
    let tuple_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        tuple_instance.return_ty,
        mir::Type::Tuple(vec![mir::Type::Int, mir::Type::String])
    );
}

#[test]
fn enum_instances_are_created_once_with_substituted_fields() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    // A non-generic enum.
    let color_variants = ["Red", "Green", "Blue"]
        .iter()
        .map(|name| hir::Variant {
            name: name.to_string(),
            fields: Vec::new(),
            defaults: Vec::new(),
        })
        .collect();
    let color = h.declare_enum("Color", Vec::new(), Vec::new(), color_variants);
    let color_ty = h.enum_ty(color);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let option_int = h.option(int);
    let option_string = h.option(string);
    let option_s = h.option(s_ty);
    // f1 holds Option<Int> and Color; f2 holds Option<Int> again
    // (a duplicate request) and Option<String>.
    let mut locals1 = Arena::new();
    locals1.alloc(local("o", option_int));
    locals1.alloc(local("c", color_ty));
    let _f1 = h.user_fn(
        "f1",
        hir::Body {
            locals: locals1,
            statements: Vec::new(),
        },
    );
    let mut locals2 = Arena::new();
    locals2.alloc(local("o", option_int));
    locals2.alloc(local("s", option_string));
    let _f2 = h.user_fn(
        "f2",
        hir::Body {
            locals: locals2,
            statements: Vec::new(),
        },
    );
    let mut locals3 = Arena::new();
    locals3.alloc(local("s", option_s));
    let _f3 = h.user_fn(
        "f3",
        hir::Body {
            locals: locals3,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    // One definition per (enum, type args), in creation order; the
    // duplicate Option<Int> request was deduplicated by enum identity.
    let names: Vec<&str> = module
        .enums
        .iter()
        .map(|(_, def)| def.name.as_str())
        .collect();
    assert_eq!(names, ["Option$I", "Color", "Option$S", "Option$D1_SX"]);

    // The variant field types are substituted with the instance's
    // type arguments.
    let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
    assert_eq!(option_int_def.variants[0].name, "Some");
    assert_eq!(option_int_def.variants[0].fields[0].ty, mir::Type::Int);
    assert!(option_int_def.gc_free);
    assert!(
        option_int_def
            .variants
            .iter()
            .all(|variant| variant.gc_free)
    );
    let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
    assert_eq!(
        option_string_def.variants[0].fields[0].ty,
        mir::Type::String
    );
    let option_s_def = &module.enums[la_arena::Idx::from_raw(3.into())];
    assert_eq!(
        option_s_def.variants[0].fields[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_ne!(option_string_def.name, option_s_def.name);
    assert!(!option_string_def.gc_free);
    assert!(!option_string_def.variants[0].gc_free);
    assert!(option_string_def.variants[1].gc_free);
    // Color's variants are all unit variants.
    let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
    assert!(color_def.gc_free);
    assert_eq!(color_def.variants.len(), 3);
    assert!(
        color_def
            .variants
            .iter()
            .all(|variant| variant.fields.is_empty() && variant.gc_free)
    );
}

#[test]
fn option_nodes_become_generic_enum_operations() {
    let mut h = Harness::new();
    let (int, boolean) = (h.int, h.boolean);
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let n = locals.alloc(local("n", option_int));
    let b = locals.alloc(local("b", boolean));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 41))),
                        option_int,
                    ),
                ),
                val_decl(n, expr(hir::ExprKind::NoneLiteral, option_int)),
                val_decl(
                    b,
                    expr(
                        hir::ExprKind::IsSome(Box::new(local_ref(o, option_int))),
                        boolean,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: false,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 41
      val n: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v1
      val b: Boolean
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local o
          Type Int
          IntLiteral 0
      val y: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local o
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn trapping_unwrap_becomes_a_guarded_extraction() {
    // val o = Some(1); val y = o!!
    let mut h = Harness::new();
    h.exception("UnwrapException");
    let int = h.int;
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: true,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The operand is evaluated once into `$opt.1`; the tag test
    // guards the extraction, and the else branch throws
    // `UnwrapException()` (M8) — an ordinary constructor call.
    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  class UnwrapException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $opt.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $opt.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val $uw.2: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $opt.1
      goto bb3
    bb2 if.else.2
      call $call.1: UnwrapException = @scoop.ctor.UnwrapException direct
      throw
        Type UnwrapException
        Local $call.1
    bb3 if.merge.3
      val y: Int
        Type Int
        Local $uw.2
      return
  fun ctor.UnwrapException @scoop.ctor.UnwrapException() -> UnwrapException
    bb0 entry
      return
        Type UnwrapException
        ClassInit UnwrapException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

/// `val a: Option<Int> = None; val b = Some(1); val r = a <op> b`
/// — the shared shell of the enum equality tests.
fn when_stmt(
    subject: hir::Expr,
    arms: Vec<hir::WhenArm>,
    else_body: Option<Vec<hir::Statement>>,
) -> hir::Statement {
    stmt(hir::StatementKind::When(hir::When {
        subject,
        arms,
        else_body,
    }))
}

fn arm(pattern: hir::Pattern, guard: Option<hir::Expr>, body: Vec<hir::Statement>) -> hir::WhenArm {
    hir::WhenArm {
        pattern,
        guard,
        body,
        span: SPAN,
    }
}

#[test]
fn when_lowers_to_a_decision_sequence() {
    // val o = Some(1); when (o) { Some(x) -> print(x); None -> println("none") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                when_stmt(
                    local_ref(o, option_int),
                    vec![
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 0,
                                fields: vec![(0, hir::Pattern::Binding { local: x })],
                            },
                            None,
                            vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                        ),
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 1,
                                fields: Vec::new(),
                            },
                            None,
                            vec![expr_stmt(call(
                                &h,
                                println_string,
                                vec![str_lit(&h, "none")],
                            ))],
                        ),
                    ],
                    None,
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subject is evaluated once into `$when.1`; each arm is a
    // tag comparison, then the field bindings, then the body; a
    // failed tag test falls through to the next arm. (`print` /
    // `println` are ordinary core functions — M7 — so the arms
    // call the overloads, not runtime shims.)
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
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
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      call @scoop.print direct
        Type Int
        Local x
      goto bb3
    bb2 if.else.2
      branch bb4 bb5
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 1
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb5
    bb5 if.merge.5
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn a_failed_guard_falls_through_to_the_next_arm() {
    // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let else_body = || {
        vec![expr_stmt(call(
            &h,
            println_string,
            vec![str_lit(&h, "neg")],
        ))]
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(o, option_int),
                vec![arm(
                    hir::Pattern::Variant {
                        application: option_application,
                        variant: 0,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    Some(binary(
                        hir::BinOp::Gt,
                        local_ref(x, int),
                        int_lit(&h, 0),
                        h.boolean,
                    )),
                    vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                )],
                Some(else_body()),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // The guard nests inside the tag test's then branch; failing
    // it falls through to the next arm — the `else` body here,
    // which is lowered once per fallthrough edge.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
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
  fun main @scoop_main() -> Unit
    bb0 entry
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      branch bb4 bb5
        Type Boolean
        Binary IntGt
          Type Int
          Local x
          Type Int
          IntLiteral 0
    bb2 if.else.2
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      goto bb3
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.print direct
        Type Int
        Local x
      goto bb6
    bb5 if.else.5
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb6
    bb6 if.merge.6
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"neg\"
  str @scoop.str.2 \"neg\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn literal_patterns_match_by_equality() {
    // when (n) { 1 -> println("one"); else -> println("other") }
    let mut h = Harness::new();
    let println = h.println_string();
    let int = h.int;
    let boolean = h.boolean;
    let mut equals_locals = Arena::new();
    let left = equals_locals.alloc(local("left", int));
    let right = equals_locals.alloc(local("right", int));
    let equals = h.user_fn_full(
        "Int.equals",
        Vec::new(),
        vec![param("left", int, left), param("right", int, right)],
        boolean,
        hir::Body {
            locals: equals_locals,
            statements: vec![hir::Statement {
                kind: hir::StatementKind::Return {
                    value: Some(bool_lit(&h, true)),
                },
                span: SPAN,
            }],
        },
    );
    let mut locals = Arena::new();
    let n = locals.alloc(local("n", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(n, int),
                vec![arm(
                    hir::Pattern::Literal {
                        value: int_lit(&h, 1),
                        equals: hir::Callable::Function(equals),
                        subject_ty: int,
                    },
                    None,
                    vec![expr_stmt(call(&h, println, vec![str_lit(&h, "one")]))],
                )],
                Some(vec![expr_stmt(call(
                    &h,
                    println,
                    vec![str_lit(&h, "other")],
                ))]),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let (call, result) = entry_statements(body)
        .iter()
        .find_map(|statement| {
            matches!(statement.kind, mir::StatementKind::Call(_)).then(|| statement_call(statement))
        })
        .expect("literal pattern calls its HIR-selected equality target");
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    assert!(matches!(call.args.as_slice(), [scrutinee, literal]
            if matches!(scrutinee.kind, mir::ExprKind::Local(_))
                && matches!(literal.kind, mir::ExprKind::IntLiteral(1))));
    let result = result.expect("equals returns Boolean");
    let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
        panic!("literal equality result controls the pattern branch")
    };
    assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == result));
}

#[test]
fn destructuring_val_declarations_extract_bindings() {
    // val (a, b) = (1, "x"); val Point { x, .. } = p
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let point = h.strukt("Point", &[("x", int), ("y", int)]);
    let point_ty = h.struct_ty(point);
    let point_application = h.struct_application_of(point_ty);
    let pair = h.tuple(&[int, string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", int));
    let b = locals.alloc(local("b", string));
    let p = locals.alloc(local("p", point_ty));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Tuple(vec![
                        hir::Pattern::Binding { local: a },
                        hir::Pattern::Binding { local: b },
                    ]),
                    init: expr(
                        hir::ExprKind::TupleLiteral(vec![int_lit(&h, 1), str_lit(&h, "x")]),
                        pair,
                    ),
                }),
                val_decl(
                    p,
                    struct_init(&h, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                ),
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Struct {
                        application: point_application,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    init: local_ref(p, point_ty),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // Each destructuring declaration evaluates its init once into
    // a hidden local, then binds the extracted fields.
    let expected = "\
Module
  struct Point (x: Int, y: Int)
  fun main @scoop_main() -> Unit
    bb0 entry
      val $bind.1: (Int, String)
        Type (Int, String)
        TupleLiteral
          Type Int
          IntLiteral 1
          Type String
          StringConst @scoop.str.0
      val a: Int
        Type Int
        FieldAccess 0
          Type (Int, String)
          Local $bind.1
      val b: String
        Type String
        FieldAccess 1
          Type (Int, String)
          Local $bind.1
      val p: Point
        Type Point
        StructInit Point
          Type Int
          IntLiteral 3
          Type Int
          IntLiteral 4
      val $bind.2: Point
        Type Point
        Local p
      val x: Int
        Type Int
        FieldAccess 0
          Type Point
          Local $bind.2
      return
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn array_nodes_translate_one_to_one() {
    // val a = [1, 2, 3]; val x = a[0]; val n = a.size
    // val m: MutableArray<Int> = MutableArray(a); m[0] = 40
    let mut h = Harness::new();
    h.exception("IndexOutOfBoundsException");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", array_int));
    let x = locals.alloc(local("x", int));
    let n = locals.alloc(local("n", int));
    let m = locals.alloc(local("m", mutable_int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    a,
                    expr(
                        hir::ExprKind::ArrayLiteral(vec![
                            int_lit(&h, 1),
                            int_lit(&h, 2),
                            int_lit(&h, 3),
                        ]),
                        array_int,
                    ),
                ),
                val_decl(
                    x,
                    expr(
                        hir::ExprKind::Index {
                            receiver: Box::new(local_ref(a, array_int)),
                            index: Box::new(int_lit(&h, 0)),
                        },
                        int,
                    ),
                ),
                val_decl(
                    n,
                    expr(
                        hir::ExprKind::ArrayLen(Box::new(local_ref(a, array_int))),
                        int,
                    ),
                ),
                val_decl(
                    m,
                    expr(
                        hir::ExprKind::ArrayClone(Box::new(local_ref(a, array_int))),
                        mutable_int,
                    ),
                ),
                stmt(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Index {
                        array: local_ref(m, mutable_int),
                        index: int_lit(&h, 0),
                    },
                    value: int_lit(&h, 40),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subscript read and the indexed store both get the M8
    // bounds check: array and index evaluated once into hidden
    // locals, then `IndexOutOfBoundsException` on failure.
    let expected = "\
Module
  class IndexOutOfBoundsException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val a: Array<Int>
        Type Array<Int>
        ArrayLiteral Array$I
          Type Int
          IntLiteral 1
          Type Int
          IntLiteral 2
          Type Int
          IntLiteral 3
      val $arr.1: Array<Int>
        Type Array<Int>
        Local a
      val $idx.2: Int
        Type Int
        IntLiteral 0
      branch bb2 bb1
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.2
          Type Int
          IntLiteral 0
    bb1 logic.rhs.1
      assign $logic.1
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.2
          Type Int
          ArrayLen Array$I
            Type Array<Int>
            Local $arr.1
      goto bb3
    bb2 logic.short.2
      assign $logic.1
        Type Boolean
        BoolLiteral true
      goto bb3
    bb3 logic.merge.3
      branch bb4 bb5
        Type Boolean
        Local $logic.1
    bb4 if.then.4
      call $call.2: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.2
    bb5 if.merge.5
      val x: Int
        Type Int
        ArrayGet Array$I
          Type Array<Int>
          Local $arr.1
          Type Int
          Local $idx.2
      val n: Int
        Type Int
        ArrayLen Array$I
          Type Array<Int>
          Local a
      val m: MutableArray<Int>
        Type MutableArray<Int>
        ArrayClone Array$I -> MutableArray$I
          Type Array<Int>
          Local a
      val $arr.3: MutableArray<Int>
        Type MutableArray<Int>
        Local m
      val $idx.4: Int
        Type Int
        IntLiteral 0
      branch bb7 bb6
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.4
          Type Int
          IntLiteral 0
    bb6 logic.rhs.6
      assign $logic.3
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.4
          Type Int
          ArrayLen MutableArray$I
            Type MutableArray<Int>
            Local $arr.3
      goto bb8
    bb7 logic.short.7
      assign $logic.3
        Type Boolean
        BoolLiteral true
      goto bb8
    bb8 logic.merge.8
      branch bb9 bb10
        Type Boolean
        Local $logic.3
    bb9 if.then.9
      call $call.4: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.4
    bb10 if.merge.10
      array_set MutableArray$I
        Type MutableArray<Int>
        Local $arr.3
        Type Int
        Local $idx.4
        Type Int
        IntLiteral 40
      return
  fun ctor.IndexOutOfBoundsException @scoop.ctor.IndexOutOfBoundsException() -> IndexOutOfBoundsException
    bb0 entry
      return
        Type IndexOutOfBoundsException
        ClassInit IndexOutOfBoundsException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn instance_symbols_encode_array_arguments() {
    let mut h = Harness::new();
    let f = identity_fn(&mut h, "f");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.instantiate(f, vec![array_int]);
    h.instantiate(f, vec![mutable_int]);
    let module = lower(&h.finish(main));

    let symbols: Vec<&str> = module.top_level[1..]
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    // `mir::encode_type`: `A<element>X` / `M<element>X`.
    assert_eq!(symbols, ["scoop.f$AIX", "scoop.f$MIX"]);
    // Substitution recurses into the array element types.
    let array_instance = &module.functions[module.top_level[1]];
    assert_eq!(
        mir::array_type(&module, &array_instance.params[0].ty),
        Some((mir::ArrayKind::Immutable, &mir::Type::Int))
    );
    let mutable_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        mir::array_type(&module, &mutable_instance.return_ty),
        Some((mir::ArrayKind::Mutable, &mir::Type::Int))
    );
}

// ---- M6: reference types ----

/// The symbol a vtable / itable slot points at.
fn slot_fn<'a>(module: &'a mir::Module, slot: &mir::TableSlot) -> &'a str {
    match slot {
        mir::TableSlot::Function(id) => &module.functions[*id].symbol,
        mir::TableSlot::Runtime(function) => function.symbol(),
    }
}

/// A `this`-taking method with an empty body, as hir-lower
/// produces it for `fun m() {}`-style declarations; the name is
/// qualified `Owner.method` like hir-lower qualifies members.
fn empty_method(
    h: &mut Harness,
    owner: &str,
    name: &str,
    receiver: hir::TypeId,
) -> hir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", receiver));
    let unit = h.unit;
    h.method_fn(
        &format!("{owner}.{name}"),
        receiver,
        vec![param("this", receiver, this)],
        unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    )
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

#[test]
fn no_gc_effect_is_preserved_in_mir() {
    let mut h = Harness::new();
    let main = empty_main(&mut h);
    h.functions[main].attributes.gc_effect = hir::GcEffect::NoGc;
    let module = lower(&h.finish(main));
    assert_eq!(
        module.functions[module.entry].gc_effect,
        mir::GcEffect::NoGc
    );
    assert!(mir::dump(&module).contains("-> Unit <no-gc>"));
}

fn class_index(raw: u32) -> mir::ClassId {
    la_arena::Idx::from_raw(raw.into())
}

#[test]
fn class_fields_are_base_prefix_then_own() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let base = h.class("Base", hir::ClassModifier::Open, &[("a", int)], None, &[]);
    let derived = h.class(
        "Derived",
        hir::ClassModifier::Final,
        &[("b", string)],
        Some((base, vec![int_lit(&h, 0)])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let derived_application = h.class_application_of(derived_ty);
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", derived_ty));
    let b = locals.alloc(local("b", string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                b,
                expr(
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(local_ref(d, derived_ty)),
                        field: hir::FieldRef::ClassField {
                            application: derived_application,
                            index: 1,
                        },
                    },
                    string,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    assert_eq!(visible_class_count(&module), 2);
    let base_def = &module.classes[class_index(0)];
    let derived_def = &module.classes[class_index(1)];
    let field_names = |def: &mir::ClassDef| {
        def.declared_fields()
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(field_names(base_def), ["a"]);
    // The base prefix comes first; HIR's `ClassField` indices
    // follow the same flattened order.
    assert_eq!(field_names(derived_def), ["a", "b"]);
    assert_eq!(derived_def.declared_fields()[1].ty, mir::Type::String);
    assert_eq!(derived_def.base_class(), Some(class_index(0)));
    assert_eq!(derived_def.modifier, mir::ClassModifier::Final);
    assert_eq!(base_def.modifier, mir::ClassModifier::Open);

    // The field access keeps its 0-based index into the flattened
    // layout.
    let body = &module.functions[module.entry].body;
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
        panic!("expected a val declaration")
    };
    assert!(matches!(
        init.kind,
        mir::ExprKind::FieldAccess { index: 1, .. }
    ));
}

#[test]
fn vtable_layout_copies_the_base_prefix_and_replaces_overrides() {
    let mut h = Harness::new();
    let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let _m1 = empty_method(&mut h, "Base", "m1", base_ty);
    let _m2 = empty_method(&mut h, "Base", "m2", base_ty);
    let derived = h.class(
        "Derived",
        hir::ClassModifier::Open,
        &[],
        Some((base, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    // `m2` overrides the base method (same slot), `m3` is new
    // (appended after the base's slots).
    let _m2_derived = empty_method(&mut h, "Derived", "m2", derived_ty);
    let _m3 = empty_method(&mut h, "Derived", "m3", derived_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let vtable_symbols = |def: &mir::ClassDef| {
        def.vtable
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect::<Vec<_>>()
    };
    // Ordinary member functions are the whole vtable; Any does not
    // reserve compiler-owned slots.
    assert_eq!(
        vtable_symbols(&module.classes[class_index(0)]),
        ["scoop.Base.m1", "scoop.Base.m2"]
    );
    // The base prefix is preserved; the override replaces slot 1
    // in place; the new method appends at slot 2.
    assert_eq!(
        vtable_symbols(&module.classes[class_index(1)]),
        ["scoop.Base.m1", "scoop.Derived.m2", "scoop.Derived.m3"]
    );
}

#[test]
fn itables_follow_the_interface_method_order() {
    let mut h = Harness::new();
    let iface = h.interface("Describable", &["a", "b"]);
    let class = h.class("C", hir::ClassModifier::Final, &[], None, &[iface]);
    let class_ty = h.class_ty(class);
    // The implementations are declared in reverse order: the
    // itable slots follow the interface's declaration order.
    let _impl_b = empty_method(&mut h, "C", "b", class_ty);
    let _impl_a = empty_method(&mut h, "C", "a", class_ty);
    // The derived class inherits `a` and overrides `b`; the
    // interface is covered without being redeclared.
    let derived = h.class(
        "D",
        hir::ClassModifier::Final,
        &[],
        Some((class, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let _impl_b_d = empty_method(&mut h, "D", "b", derived_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let class_def = &module.classes[class_index(0)];
    assert_eq!(class_def.itables.len(), 1);
    let record = &class_def.itables[0];
    assert_eq!(record.interface, la_arena::Idx::from_raw(0.into()));
    let slots: Vec<&str> = record
        .slots
        .iter()
        .map(|slot| slot_fn(&module, slot))
        .collect();
    assert_eq!(slots, ["scoop.C.a", "scoop.C.b"]);

    let derived_def = &module.classes[class_index(1)];
    assert_eq!(derived_def.itables.len(), 1);
    let record = &derived_def.itables[0];
    let slots: Vec<&str> = record
        .slots
        .iter()
        .map(|slot| slot_fn(&module, slot))
        .collect();
    // The override dispatches to the derived implementation; the
    // inherited method keeps the base's.
    assert_eq!(slots, ["scoop.C.a", "scoop.D.b"]);
}

#[test]
fn method_calls_are_annotated_by_the_receiver_static_type() {
    let mut h = Harness::new();
    let iface = h.interface("Describable", &["describe", "label"]);
    let iface_ty = h.interface_ty(iface);
    let class = h.class("C", hir::ClassModifier::Open, &[], None, &[iface]);
    let class_ty = h.class_ty(class);
    let class_describe = empty_method(&mut h, "C", "describe", class_ty);
    let _class_label = empty_method(&mut h, "C", "label", class_ty);
    // Interface method shells, as hir-lower materializes them.
    let _iface_describe = empty_method(&mut h, "Describable", "describe", iface_ty);
    let iface_label = empty_method(&mut h, "Describable", "label", iface_ty);
    // A value type method.
    let int = h.int;
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let s_describe = empty_method(&mut h, "S", "describe", s_ty);
    let class_describe = h.method_application(class_describe);
    let iface_label = h.method_application(iface_label);
    let s_describe = h.method_application(s_describe);

    let unit = h.unit;
    let mut locals = Arena::new();
    let c = locals.alloc(local("c", class_ty));
    let i = locals.alloc(local("i", iface_ty));
    let sv = locals.alloc(local("sv", s_ty));
    let method_call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
        expr(
            hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                args: Vec::new(),
            },
            unit,
        )
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(c, class_ty), class_describe)),
                expr_stmt(method_call(local_ref(i, iface_ty), iface_label)),
                expr_stmt(method_call(local_ref(sv, s_ty), s_describe)),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        // The receiver becomes argument 0 (`this`).
        assert!(!call.args.is_empty());
        &call.target.kind
    };
    // Class receiver: virtual through its ordinary vtable.
    assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
    // Interface receiver: the method's declaration index is the
    // itable slot.
    assert!(matches!(
        call_kind(1),
        mir::CallKind::Interface { interface, slot: 1 } if *interface == la_arena::Idx::from_raw(0.into())
    ));
    // Value type receiver: direct.
    assert!(matches!(call_kind(2), mir::CallKind::Direct));
}

#[test]
fn final_methods_are_direct_while_final_overrides_keep_the_base_slot() {
    let mut h = Harness::new();
    let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let base_open = empty_method(&mut h, "Base", "openMethod", base_ty);
    let base_final = empty_method(&mut h, "Base", "finalMethod", base_ty);
    h.functions[base_final]
        .method
        .as_mut()
        .expect("method")
        .modifier = hir::MethodModifier::Final;

    let derived = h.class(
        "Derived",
        hir::ClassModifier::Final,
        &[],
        Some((base, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let derived_override = empty_method(&mut h, "Derived", "openMethod", derived_ty);
    h.functions[derived_override]
        .method
        .as_mut()
        .expect("method")
        .modifier = hir::MethodModifier::Final;
    let base_open = h.method_application(base_open);
    let base_final = h.method_application(base_final);
    let derived_override = h.method_application(derived_override);

    let unit = h.unit;
    let mut locals = Arena::new();
    let as_base = locals.alloc(local("asBase", base_ty));
    let as_derived = locals.alloc(local("asDerived", derived_ty));
    let call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
        expr(
            hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                args: Vec::new(),
            },
            unit,
        )
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(call(local_ref(as_base, base_ty), base_open)),
                expr_stmt(call(local_ref(as_base, base_ty), base_final)),
                expr_stmt(call(local_ref(as_derived, derived_ty), derived_override)),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let base_vtable = &module.classes[class_index(0)].vtable;
    assert_eq!(base_vtable.len(), 1);
    assert_eq!(slot_fn(&module, &base_vtable[0]), "scoop.Base.openMethod");
    let derived_vtable = &module.classes[class_index(1)].vtable;
    assert_eq!(derived_vtable.len(), 1);
    assert_eq!(
        slot_fn(&module, &derived_vtable[0]),
        "scoop.Derived.openMethod"
    );

    let body = &module.functions[module.entry].body;
    let kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    assert!(matches!(kind(0), mir::CallKind::Virtual { slot: 0 }));
    assert!(matches!(kind(1), mir::CallKind::Direct));
    assert!(matches!(kind(2), mir::CallKind::Direct));
}

#[test]
fn boxing_only_materializes_the_payload_class() {
    let mut h = Harness::new();
    let int = h.int;
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                a,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    any,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.declared_fields().len(), 1);
    assert_eq!(boxed.declared_fields()[0].name, "value");
    assert_eq!(
        boxed.declared_fields()[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert!(boxed.vtable.is_empty());
    assert!(boxed.itables.is_empty());
    assert_eq!(module.meta.boxed_types.len(), 1);
    let boxed_meta = &module.meta.boxed_types[0];
    assert_eq!(
        boxed_meta.payload,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_eq!(module.classes[boxed_meta.class].name, "box$D1_SX");
    assert!(module.functions.iter().all(|(_, function)| {
        !function.symbol.starts_with("scoop.eq.") && !function.symbol.starts_with("scoop.tostring.")
    }));
}

#[test]
fn boxed_interface_implementations_dispatch_through_adjust_thunks() {
    let mut h = Harness::new();
    let int = h.int;
    let iface = h.interface("Describable", &["describe"]);
    let iface_ty = h.interface_ty(iface);
    let s = h.strukt_with("S", &[("x", int)], &[iface]);
    let s_ty = h.struct_ty(s);
    let _describe = empty_method(&mut h, "S", "describe", s_ty);
    // `val d: Describable = S(1)` — a Box whose target is the
    // interface.
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", iface_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                d,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    iface_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.interfaces.len(), 1);
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.interface, boxed.interfaces[0]);
    assert_eq!(record.slots.len(), 1);
    let thunk_symbol = slot_fn(&module, &record.slots[0]);
    assert_eq!(thunk_symbol, "scoop.thunk.D1_SX.Describable.describe");

    // The thunk takes the boxed object as `this`, unboxes it and
    // tail-calls the value method.
    let thunk = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == thunk_symbol)
        .expect("the thunk is a MIR function");
    assert_eq!(thunk.params.len(), 1);
    assert_eq!(thunk.params[0].ty, mir::Type::Any);
    assert_eq!(thunk.params[0].name, "this");
    let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    let mir::Callee::User(impl_id) = call.target.callee else {
        panic!("the thunk calls a user function")
    };
    assert_eq!(module.functions[impl_id].symbol, "scoop.S.describe");
    assert_eq!(call.args.len(), 1);
    assert!(matches!(&call.args[0].kind, mir::ExprKind::Unbox(operand)
            if matches!(operand.kind, mir::ExprKind::Local(local) if local == thunk.params[0].local)));
}

#[test]
fn is_instance_and_casts_lower_to_runtime_checks() {
    let mut h = Harness::new();
    h.exception("ClassCastException");
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let option_s = h.option(s_ty);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let is_s = locals.alloc(local("is_s", boolean));
    let s2 = locals.alloc(local("s2", s_ty));
    let maybe = locals.alloc(local("maybe", option_s));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    is_s,
                    expr(
                        hir::ExprKind::IsInstance {
                            operand: Box::new(local_ref(a, any)),
                            check_ty: s_ty,
                        },
                        boolean,
                    ),
                ),
                val_decl(
                    s2,
                    // Mirror hir-lower's real shape: a value-typed
                    // `as` arrives as `Unbox(Cast)`; mir-lower's
                    // cast expansion only performs the check.
                    expr(
                        hir::ExprKind::Unbox(Box::new(expr(
                            hir::ExprKind::Cast {
                                operand: Box::new(local_ref(a, any)),
                                optional: false,
                            },
                            s_ty,
                        ))),
                        s_ty,
                    ),
                ),
                val_decl(
                    maybe,
                    expr(
                        hir::ExprKind::Cast {
                            operand: Box::new(local_ref(a, any)),
                            optional: true,
                        },
                        option_s,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // `is` stays a dedicated node; `as` throws
    // `ClassCastException` on failure (M8); `as?` wraps in
    // Some / None. The value-type checks registered the boxed
    // payload class. Capabilities are not synthesized from boxing.
    let expected = "\
Module
  struct S (x: Int)
  enum Option$D1_SX
    Some(_1: S)
    None()
  class ClassCastException vtable=0 itables=0
  class box$D1_SX vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val is_s: Boolean
        Type Boolean
        IsInstance S
          Type Any
          Local a
      val $cast.1: Any
        Type Any
        Local a
      branch bb1 bb2
        Type Boolean
        Unary BoolNot
          Type Boolean
          IsInstance S
            Type Any
            Local $cast.1
    bb1 if.then.1
      call $call.1: ClassCastException = @scoop.ctor.ClassCastException direct
      throw
        Type ClassCastException
        Local $call.1
    bb2 if.merge.2
      val $ub.2: S
        Type S
        Unbox
          Type Any
          Local $cast.1
      val s2: S
        Type S
        Local $ub.2
      val $cast.3: Any
        Type Any
        Local a
      branch bb3 bb4
        Type Boolean
        IsInstance S
          Type Any
          Local $cast.3
    bb3 if.then.3
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v0
          Type S
          Unbox
            Type Any
            Local $cast.3
      goto bb5
    bb4 if.else.4
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v1
      goto bb5
    bb5 if.merge.5
      val maybe: Option$D1_SX<S>
        Type Option$D1_SX<S>
        Local $cast.4
      return
  fun ctor.ClassCastException @scoop.ctor.ClassCastException() -> ClassCastException
    bb0 entry
      return
        Type ClassCastException
        ClassInit ClassCastException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn constructor_functions_initialize_the_flattened_fields() {
    // open class Root(val label: String)
    // open class Base(val name: String) : Root("root")
    // class Point(val x: Int) : Base("point")
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let root = h.class(
        "Root",
        hir::ClassModifier::Open,
        &[("label", string)],
        None,
        &[],
    );
    let base = h.class(
        "Base",
        hir::ClassModifier::Open,
        &[("name", string)],
        Some((root, vec![str_lit(&h, "root")])),
        &[],
    );
    let point = h.class(
        "Point",
        hir::ClassModifier::Final,
        &[("x", int)],
        Some((base, vec![str_lit(&h, "point")])),
        &[],
    );
    let point_ty = h.class_ty(point);
    let point_application = h.class_application_of(point_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                p,
                expr(
                    hir::ExprKind::ClassInit {
                        application: point_application,
                        args: vec![int_lit(&h, 1)],
                    },
                    point_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // One ctor per class; the use site is a plain direct call.
    // Each ctor returns a raw ClassInit over the flattened field
    // values: the base delegation arguments (re-evaluated in each
    // derived ctor — hence the repeated "root" constant), then the
    // own properties. No base ctor is called.
    let expected = "\
Module
  class Root vtable=0 itables=0
  class Base vtable=0 itables=0
  class Point vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      call p: Point = @scoop.ctor.Point direct
        Type Int
        IntLiteral 1
      return
  fun ctor.Root @scoop.ctor.Root(label: String) -> Root
    bb0 entry
      return
        Type Root
        ClassInit Root
          Type String
          Local label
  fun ctor.Base @scoop.ctor.Base(name: String) -> Base
    bb0 entry
      return
        Type Base
        ClassInit Base
          Type String
          StringConst @scoop.str.0
          Type String
          Local name
  fun ctor.Point @scoop.ctor.Point(x: Int) -> Point
    bb0 entry
      return
        Type Point
        ClassInit Point
          Type String
          StringConst @scoop.str.2
          Type String
          StringConst @scoop.str.1
          Type Int
          Local x
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  str @scoop.str.2 \"root\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn abstract_classes_get_no_constructor() {
    let mut h = Harness::new();
    let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert!(
        !module
            .functions
            .iter()
            .any(|(_, f)| f.symbol.starts_with("scoop.ctor."))
    );
}

#[test]
fn field_assignment_lowers_to_field_set() {
    // `p.y = 3` on a class with two properties (index 1 in the
    // flattened layout).
    let mut h = Harness::new();
    let int = h.int;
    let c = h.class(
        "C",
        hir::ClassModifier::Final,
        &[("x", int), ("y", int)],
        None,
        &[],
    );
    let c_ty = h.class_ty(c);
    let c_application = h.class_application_of(c_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", c_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Field {
                    receiver: Box::new(local_ref(p, c_ty)),
                    field: hir::FieldRef::ClassField {
                        application: c_application,
                        index: 1,
                    },
                },
                value: int_lit(&h, 3),
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let mir::StatementKind::FieldSet {
        object,
        index: 1,
        value,
    } = &entry_statements(body)[0].kind
    else {
        panic!("a class property assignment must lower to FieldSet")
    };
    assert!(matches!(object.kind, mir::ExprKind::Local(_)));
    assert!(matches!(value.kind, mir::ExprKind::IntLiteral(3)));
}

#[test]
fn boxed_interfaces_come_from_the_declaration() {
    // `struct S(val x: Int) : Describable` boxed to `Any` — the
    // boxed itable covers the declared interface even though the
    // box target is not the interface.
    let mut h = Harness::new();
    let int = h.int;
    let iface = h.interface("Describable", &["describe"]);
    let s = h.strukt_with("S", &[("x", int)], &[iface]);
    let s_ty = h.struct_ty(s);
    let _describe = empty_method(&mut h, "S", "describe", s_ty);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                a,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    any,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.interfaces.len(), 1);
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.interface, boxed.interfaces[0]);
    assert_eq!(
        slot_fn(&module, &record.slots[0]),
        "scoop.thunk.D1_SX.Describable.describe"
    );
}

#[test]
fn ref_equality_maps_to_a_primitive_pointer_comparison() {
    // `===` / `!==` are reference identity: the primitive
    // comparison on the two pointers.
    let mut h = Harness::new();
    let boolean = h.boolean;
    let c = h.class("C", hir::ClassModifier::Final, &[], None, &[]);
    let c_ty = h.class_ty(c);
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", c_ty));
    let y = locals.alloc(local("y", c_ty));
    let same = locals.alloc(local("same", boolean));
    let other = locals.alloc(local("other", boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    same,
                    binary(
                        hir::BinOp::RefEq,
                        local_ref(x, c_ty),
                        local_ref(y, c_ty),
                        boolean,
                    ),
                ),
                val_decl(
                    other,
                    binary(
                        hir::BinOp::RefNe,
                        local_ref(x, c_ty),
                        local_ref(y, c_ty),
                        boolean,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let op_of = |index: usize| {
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[index].kind else {
            panic!("expected a val declaration")
        };
        let mir::ExprKind::Binary { op, .. } = &init.kind else {
            panic!("expected a binary expression")
        };
        *op
    };
    assert_eq!(op_of(0), mir::BinOp::IntEq);
    assert_eq!(op_of(1), mir::BinOp::IntNe);
}

#[test]
fn abstract_methods_lower_to_trap_stubs() {
    // `abstract class Base { abstract fun id(): Int }` — hir-lower
    // materializes the abstract method as a params-only bodiless
    // function (`Base.id`, no statements).
    let mut h = Harness::new();
    let int = h.int;
    let base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", base_ty));
    let id = h.method_fn(
        "Base.id",
        base_ty,
        vec![param("this", base_ty, this)],
        int,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    h.functions[id].method.as_mut().expect("a method").modifier = hir::MethodModifier::Abstract;
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    // The abstract method is emitted (the abstract class's vtable
    // slot references it) and traps like a pure-virtual stub.
    let base_def = &module.classes[class_index(0)];
    assert_eq!(slot_fn(&module, &base_def.vtable[0]), "scoop.Base.id");
    let stub = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == "scoop.Base.id")
        .expect("the abstract method is emitted");
    assert!(
        module
            .top_level
            .iter()
            .any(|&id| module.functions[id].symbol == "scoop.Base.id")
    );
    assert!(matches!(
        stub.body.blocks[stub.body.entry].terminator,
        mir::Terminator::Trap { .. }
    ));
}

#[test]
fn interface_implementations_resolve_qualified_method_names() {
    // `class Doc(val title: String) : Describable { override fun
    // describe() }` — hir-lower names the member `Doc.describe`;
    // the itable / vtable resolve it by its short name.
    let mut h = Harness::new();
    let string = h.string;
    let iface = h.interface("Describable", &["describe"]);
    let doc = h.class(
        "Doc",
        hir::ClassModifier::Final,
        &[("title", string)],
        None,
        &[iface],
    );
    let doc_ty = h.class_ty(doc);
    let _describe = empty_method(&mut h, "Doc", "describe", doc_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.itables.len(), 1);
    assert_eq!(
        slot_fn(&module, &doc_def.itables[0].slots[0]),
        "scoop.Doc.describe"
    );
    // The implementing method is a vtable method too.
    assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe");
}

#[test]
fn smart_cast_unboxes_bind_typed_hidden_locals() {
    // `if (a is S) { println(a.v) }` — the narrowed read arrives as
    // `FieldAccess { receiver: Unbox(Local a) }` (hir-lower's smart
    // cast). The unbox must be bound to a typed hidden local so LIR
    // never has to reconstruct its type from the `Any` operand.
    let mut h = Harness::new();
    let println_int = h.println_int();
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("v", int)]);
    let s_ty = h.struct_ty(s);
    let s_application = h.struct_application_of(s_ty);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let print_call = expr(
        hir::ExprKind::Call {
            callee: hir::Callable::Function(println_int),
            args: vec![expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(expr(
                        hir::ExprKind::Unbox(Box::new(local_ref(a, any))),
                        s_ty,
                    )),
                    field: hir::FieldRef::StructField {
                        application: s_application,
                        index: 0,
                    },
                },
                int,
            )],
        },
        h.unit,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::If {
                cond: expr(
                    hir::ExprKind::IsInstance {
                        operand: Box::new(local_ref(a, any)),
                        check_ty: s_ty,
                    },
                    boolean,
                ),
                then_body: vec![expr_stmt(print_call)],
                else_body: None,
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let mir::Terminator::Branch { then_block, .. } = body.blocks[body.entry].terminator else {
        panic!("expected a conditional branch")
    };
    let then_body = &body.blocks[then_block].statements;
    let mir::StatementKind::ValDecl { local: ub, init } = &then_body[0].kind else {
        panic!("the unbox must be a val declaration")
    };
    let mir::ExprKind::Unbox(_) = init.kind else {
        panic!("the unbox must be bound to a typed hidden local")
    };
    assert_eq!(
        body.locals[*ub].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    let (call, _) = statement_call(&then_body[1]);
    assert!(
        matches!(&call.args[0].kind, mir::ExprKind::FieldAccess { receiver, .. }
            if matches!(receiver.kind, mir::ExprKind::Local(local) if local == *ub))
    );
}
