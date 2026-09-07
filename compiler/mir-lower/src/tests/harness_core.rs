use super::*;

impl Harness {
    pub(super) fn new() -> Self {
        let mut types = Arena::new();
        let unit = types.alloc(hir::Type::Unit);
        let integer_types = hir::IntegerKind::ALL.map(|kind| types.alloc(hir::Type::Integer(kind)));
        let integers = hir::IntegerTypeCore::new(integer_types)
            .expect("the eight integer kinds receive distinct canonical types");
        let int = integers.owner(hir::IntegerKind::SIGNED_32);
        let long = integers.owner(hir::IntegerKind::SIGNED_64);
        let uint = integers.owner(hir::IntegerKind::UNSIGNED_32);
        let ulong = integers.owner(hir::IntegerKind::UNSIGNED_64);
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
            origin: hir::DeclarationOrigin {
                provider: hir::IntrinsicProviderId::from_raw(0),
                file: 0,
            },
            owner: None,
            name: "Option".to_string(),
            access: hir::NominalAccess::public(),
            self_application: option_self_application,
            type_params: vec![type_param("T")],
            gc_free_pointee_requirements: Vec::new(),
            no_gc: false,
            variants: vec![
                hir::Variant {
                    name: "Some".to_string(),
                    fields: vec![hir::Field {
                        name: "_1".to_string(),
                        ty: t,
                    }],
                },
                hir::Variant {
                    name: "None".to_string(),
                    fields: Vec::new(),
                },
            ],
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
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
            struct_constructors: Arena::new(),
            struct_constructor_applications: Arena::new(),
            struct_applications: Arena::new(),
            struct_applications_by_key: HashMap::new(),
            enums,
            enum_applications,
            enum_applications_by_key,
            classes: Arena::new(),
            class_fields: Arena::new(),
            properties: Arena::new(),
            property_getters: Arena::new(),
            property_setters: Arena::new(),
            class_constructors: Arena::new(),
            class_constructor_applications: Arena::new(),
            class_applications: Arena::new(),
            class_applications_by_key: HashMap::new(),
            interfaces: Arena::new(),
            interface_applications: Arena::new(),
            interface_methods: Arena::new(),
            interface_applications_by_key: HashMap::new(),
            top_level: Vec::new(),
            unit,
            integers,
            int,
            long,
            uint,
            ulong,
            boolean,
            string,
            option_enum,
            needs_initialization_core: false,
            write: None,
            long_to_string: None,
            bool_to_string: None,
            print_string: None,
            print_int: None,
            print_boolean: None,
            println_string: None,
            println_int: None,
            println_boolean: None,
            instantiations: Arena::new(),
            gc_core: None,
            intrinsic_array: None,
            intrinsic_mutable_array: None,
            next_constructor_param: 0,
        }
    }

    pub(super) fn integer(&self, kind: hir::IntegerKind) -> hir::TypeId {
        self.integers.owner(kind)
    }

    /// Adds core's managed `write` extern on first use so tests unrelated
    /// to output keep their MIR dumps focused on the feature under test.
    pub(super) fn write(&mut self) -> hir::FunctionId {
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
            origin: hir::DeclarationOrigin {
                provider: hir::IntrinsicProviderId::from_raw(0),
                file: 0,
            },
            name: "write".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
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
    pub(super) fn long_to_string(&mut self) -> hir::FunctionId {
        if let Some(id) = self.long_to_string {
            return id;
        }
        let extern_id = self.extern_functions.alloc(hir::ExternFunction {
            source_name: "coreLongToString".to_string(),
            native_symbol: "scoop_rt_long_to_string".to_string(),
            library: String::new(),
            abi: hir::ExternAbi::Scoop,
            calling_convention: hir::CallingConvention::Cdecl,
            gc_effect: hir::GcEffect::Managed,
            safety: hir::Safety::Safe,
            params: vec![self.long],
            return_type: self.string,
        });
        let id = self.functions.alloc(hir::Function {
            origin: hir::DeclarationOrigin {
                provider: hir::IntrinsicProviderId::from_raw(0),
                file: 0,
            },
            name: "coreLongToString".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: self.string,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Extern(extern_id),
            method: None,
            span: SPAN,
        });
        self.top_level.push(id);
        self.long_to_string = Some(id);
        id
    }

    /// scoop.core's representation-level Boolean formatter. Like the integer
    /// formatter, this is an ordinary Scoop-ABI extern declaration.
    pub(super) fn bool_to_string(&mut self) -> hir::FunctionId {
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
            origin: hir::DeclarationOrigin {
                provider: hir::IntrinsicProviderId::from_raw(0),
                file: 0,
            },
            name: "coreBooleanToString".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
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
    pub(super) fn print_string(&mut self) -> hir::FunctionId {
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

    /// core's `fun print(message: Int) = write(coreLongToString(message.toInt64()))`.
    pub(super) fn print_int(&mut self) -> hir::FunctionId {
        if let Some(id) = self.print_int {
            return id;
        }
        let (unit, int, string) = (self.unit, self.int, self.string);
        let write = self.write();
        let long_to_string = self.long_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", int));
        let message_as_long = integer_conversion(
            self,
            hir::IntegerKind::SIGNED_32,
            hir::IntegerKind::SIGNED_64,
            local_ref(message, int),
        );
        let id = self.user_fn_full(
            "print",
            Vec::new(),
            vec![param("message", int, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    write,
                    vec![call_typed(long_to_string, vec![message_as_long], string)],
                    unit,
                ))],
            },
        );
        self.print_int = Some(id);
        id
    }

    /// core's `fun print(message: Boolean) = write(boolToString(message))`.
    pub(super) fn print_boolean(&mut self) -> hir::FunctionId {
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
    pub(super) fn println_string(&mut self) -> hir::FunctionId {
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

    /// core's `fun println(message: Int) = println(coreLongToString(message.toInt64()))`.
    pub(super) fn println_int(&mut self) -> hir::FunctionId {
        if let Some(id) = self.println_int {
            return id;
        }
        let println_string = self.println_string();
        let (unit, int, string) = (self.unit, self.int, self.string);
        let long_to_string = self.long_to_string();
        let mut locals = Arena::new();
        let message = locals.alloc(local("message", int));
        let message_as_long = integer_conversion(
            self,
            hir::IntegerKind::SIGNED_32,
            hir::IntegerKind::SIGNED_64,
            local_ref(message, int),
        );
        let id = self.user_fn_full(
            "println",
            Vec::new(),
            vec![param("message", int, message)],
            unit,
            hir::Body {
                locals,
                statements: vec![expr_stmt(call_typed(
                    println_string,
                    vec![call_typed(long_to_string, vec![message_as_long], string)],
                    unit,
                ))],
            },
        );
        self.println_int = Some(id);
        id
    }

    /// core's `fun println(message: Boolean) = println(boolToString(message))`.
    pub(super) fn println_boolean(&mut self) -> hir::FunctionId {
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
}
