use super::*;
use scoop_ast::Span;

const SPAN: Span = Span { start: 0, end: 0 };

/// MIR module shell as mir-lower produces it.
struct Builder {
    functions: Arena<mir::Function>,
    extern_functions: Arena<mir::ExternFunction>,
    strings: Arena<mir::StringConst>,
    structs: Arena<mir::StructDef>,
    enums: Arena<mir::EnumDef>,
    classes: Arena<mir::ClassDef>,
    interfaces: Arena<mir::InterfaceDef>,
    top_level: Vec<mir::FunctionId>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            functions: Arena::new(),
            extern_functions: Arena::new(),
            strings: Arena::new(),
            structs: Arena::new(),
            enums: Arena::new(),
            classes: Arena::new(),
            interfaces: Arena::new(),
            top_level: Vec::new(),
        }
    }

    /// `enum Option<T> { Some(T), None }` instantiated at
    /// `payload`, named as mir-lower names its instances.
    fn option_enum(&mut self, name: &str, payload: mir::Type) -> mir::EnumId {
        let payload_gc_free = self.type_gc_free(&payload);
        self.enums.alloc(mir::EnumDef {
            name: name.to_string(),
            gc_free: payload_gc_free,
            variants: vec![
                mir::VariantDef {
                    name: "Some".to_string(),
                    gc_free: payload_gc_free,
                    fields: vec![mir::Field {
                        name: "_1".to_string(),
                        ty: payload,
                    }],
                },
                mir::VariantDef {
                    name: "None".to_string(),
                    gc_free: true,
                    fields: Vec::new(),
                },
            ],
        })
    }

    fn type_gc_free(&self, ty: &mir::Type) -> bool {
        match ty {
            mir::Type::Unit
            | mir::Type::Int
            | mir::Type::UInt
            | mir::Type::Boolean
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_) => true,
            mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Any
            | mir::Type::Function(_) => false,
            mir::Type::Struct(id) => self.structs[*id].gc_free,
            mir::Type::Enum(id, _) => self.enums[*id].gc_free,
            mir::Type::Tuple(elements) => elements.iter().all(|element| self.type_gc_free(element)),
        }
    }

    fn string(&mut self, value: &str) -> mir::StringConstId {
        let symbol = format!("scoop.str.{}", self.strings.len());
        self.strings.alloc(mir::StringConst {
            value: value.to_string(),
            symbol,
        })
    }

    fn managed_scoop_extern(
        &mut self,
        source_name: &str,
        native_symbol: &str,
        params: Vec<mir::Type>,
        return_type: mir::Type,
    ) -> mir::ExternFunctionId {
        self.extern_functions.alloc(mir::ExternFunction {
            source_name: source_name.to_string(),
            native_symbol: native_symbol.to_string(),
            library: String::new(),
            abi: mir::ExternAbi::Scoop,
            calling_convention: mir::CallingConvention::Cdecl,
            gc_effect: mir::GcEffect::Managed,
            params,
            return_type,
        })
    }

    fn strukt(&mut self, name: &str, fields: &[(&str, mir::Type)]) -> mir::StructId {
        let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
        self.structs.alloc(mir::StructDef {
            name: name.to_string(),
            gc_free,
            representation: mir::StructRepresentation::Declared {
                c_layout: None,
                interior_mutable: false,
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
            },
        })
    }

    fn c_strukt(
        &mut self,
        name: &str,
        aligned: u8,
        packed: u8,
        interior_mutable: bool,
        fields: &[(&str, mir::Type)],
    ) -> mir::StructId {
        let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
        self.structs.alloc(mir::StructDef {
            name: name.to_string(),
            gc_free,
            representation: mir::StructRepresentation::Declared {
                c_layout: Some(mir::CLayout { aligned, packed }),
                interior_mutable,
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
            },
        })
    }

    fn interface(&mut self, name: &str, methods: &[&str]) -> mir::InterfaceId {
        self.interfaces.alloc(mir::InterfaceDef {
            name: name.to_string(),
            methods: methods.iter().map(|m| m.to_string()).collect(),
        })
    }

    fn class(
        &mut self,
        name: &str,
        base: Option<mir::ClassId>,
        fields: &[(&str, mir::Type)],
        vtable: Vec<mir::TableSlot>,
        itables: Vec<mir::ItableRecord>,
    ) -> mir::ClassId {
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.to_string(),
            representation: mir::ClassRepresentation::Declared {
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
                base_class: base,
            },
            interfaces: Vec::new(),
            vtable,
            itables,
        })
    }

    fn array_class(
        &mut self,
        name: &str,
        kind: mir::ArrayKind,
        element: mir::Type,
    ) -> mir::ClassId {
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.to_string(),
            representation: mir::ClassRepresentation::Intrinsic(match kind {
                mir::ArrayKind::Immutable => mir::IntrinsicTypeRepresentation::Array { element },
                mir::ArrayKind::Mutable => {
                    mir::IntrinsicTypeRepresentation::MutableArray { element }
                }
            }),
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        })
    }

    fn array(&mut self, name: &str, element: mir::Type) -> mir::Type {
        mir::Type::Class(self.array_class(name, mir::ArrayKind::Immutable, element))
    }

    fn mutable_array(&mut self, name: &str, element: mir::Type) -> mir::Type {
        mir::Type::Class(self.array_class(name, mir::ArrayKind::Mutable, element))
    }

    /// A function that exists only as a signature (e.g. an
    /// interface method shell): not pushed to `top_level`, so it
    /// is never emitted.
    fn decl_fn(
        &mut self,
        name: &str,
        symbol: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
    ) -> mir::FunctionId {
        self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            symbol: symbol.to_string(),
            params,
            return_ty,
            body: mir::Body::unreachable(Arena::new()),
        })
    }

    fn user_fn(
        &mut self,
        name: &str,
        symbol: &str,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        self.user_fn_full(
            name,
            symbol,
            Vec::new(),
            mir::Type::Unit,
            locals,
            statements,
        )
    }

    fn user_fn_full(
        &mut self,
        name: &str,
        symbol: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        let mut blocks = Arena::new();
        let terminator = if return_ty == mir::Type::Unit {
            mir::Terminator::Return { value: None }
        } else {
            mir::Terminator::Unreachable
        };
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements,
            terminator,
            unwind: None,
        });
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            symbol: symbol.to_string(),
            params,
            return_ty,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        self.top_level.push(id);
        id
    }

    fn user_fn_body(
        &mut self,
        name: &str,
        symbol: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
        body: mir::Body,
    ) -> mir::FunctionId {
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            symbol: symbol.to_string(),
            params,
            return_ty,
            body,
        });
        self.top_level.push(id);
        id
    }

    fn main(
        &mut self,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        self.user_fn("main", mir::ENTRY_SYMBOL, locals, statements)
    }

    fn finish(mut self, entry: mir::FunctionId) -> mir::Module {
        for (name, representation) in [
            ("Int", mir::IntrinsicTypeRepresentation::Int),
            ("UInt", mir::IntrinsicTypeRepresentation::UInt),
            ("Boolean", mir::IntrinsicTypeRepresentation::Boolean),
        ] {
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                gc_free: true,
                representation: mir::StructRepresentation::Intrinsic(representation),
            });
        }
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: "String".to_string(),
            representation: mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::String,
            ),
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        mir::Module {
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: Arena::new(),
            callback_bridges: Arena::new(),
            foreign_callback_adapters: Arena::new(),
            foreign_callback_bridges: Arena::new(),
            function_types: Arena::new(),
            closure_classes: Arena::new(),
            closure_invoke_functions: Arena::new(),
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            interfaces: self.interfaces,
            entry,
            meta: mir::MirMeta::default(),
        }
    }
}

/// The reference-field offsets of a plain (non-enum) layout.
fn plain_refs(layout: &lir::Layout) -> &[u64] {
    let lir::LayoutKind::Plain { scan } = &layout.kind else {
        panic!("expected a plain layout")
    };
    match scan {
        lir::RefScan::None => &[],
        lir::RefScan::References(offsets) => offsets,
        other => panic!("expected a flat plain scan, found {other:?}"),
    }
}

fn layout_values(module: &lir::Module) -> impl Iterator<Item = &lir::Layout> {
    module.meta.layouts.iter().map(|(_, layout)| layout)
}

fn descriptor_values(module: &lir::Module) -> impl Iterator<Item = &lir::TypeDescriptor> {
    module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor)
}

fn descriptor(module: &lir::Module, reference: lir::TypeDescriptorRef) -> &lir::TypeDescriptor {
    let lir::TypeDescriptorRef::Local(id) = reference else {
        panic!("tests expect a local descriptor")
    };
    &module.meta.type_descriptors[id]
}

fn fixed_scan(descriptor: &lir::TypeDescriptor) -> &lir::RefScan {
    let lir::TypeDescriptorScan::Fixed(scan) = &descriptor.scan else {
        panic!("expected a fixed descriptor scan")
    };
    scan
}

fn array_scan(descriptor: &lir::TypeDescriptor) -> &lir::RefScan {
    let lir::TypeDescriptorScan::ArrayElement { scan, .. } = &descriptor.scan else {
        panic!("expected an array descriptor scan")
    };
    scan
}

fn array_metadata<'a>(module: &'a lir::Module, name: &str) -> &'a lir::ArrayType {
    module
        .meta
        .arrays
        .iter()
        .find_map(|(_, array)| {
            (descriptor(module, array.type_descriptor).name == name).then_some(array)
        })
        .unwrap_or_else(|| panic!("missing array metadata for {name}"))
}

fn local(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: false,
    }
}

fn var(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: true,
    }
}

fn stmt(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement { kind, span: SPAN }
}

fn val_decl(local: mir::LocalId, init: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::ValDecl { local, init })
}

fn assign(local: mir::LocalId, value: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::Assign { local, value })
}

fn expr_stmt(expr: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::Expr(expr))
}

fn expr(ty: mir::Type, kind: mir::ExprKind) -> mir::Expr {
    mir::Expr::new(ty, kind)
}

fn local_expr(local: mir::LocalId, ty: mir::Type) -> mir::Expr {
    mir::Expr::local(local, ty)
}

fn string_expr(id: mir::StringConstId) -> mir::Expr {
    expr(mir::Type::String, mir::ExprKind::StringConst(id))
}

fn binary(op: mir::BinOp, lhs: mir::Expr, rhs: mir::Expr, ty: mir::Type) -> mir::Expr {
    expr(
        ty,
        mir::ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

fn call_symbol<'a>(
    module: &'a lir::Module,
    function: &lir::Function,
    site: &lir::CallSite,
) -> &'a str {
    match site.destination(&function.call_targets) {
        lir::CallDestination::Local(id) => &module.functions[id.into_u32() as usize].symbol,
        lir::CallDestination::Runtime(runtime) => runtime.symbol(),
        lir::CallDestination::Extern(id) => &module.extern_functions[id].native_symbol,
        lir::CallDestination::Dispatch { .. } => panic!("dispatch calls have no symbol"),
    }
}

#[test]
fn no_gc_effect_is_preserved_in_lir() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    builder.functions[main].gc_effect = mir::GcEffect::NoGc;
    let module = lower(&builder.finish(main));
    assert_eq!(module.functions[0].gc_effect, lir::GcEffect::NoGc);
    assert!(lir::dump(&module).contains("-> void <no-gc>"));
}

fn runtime_call(function: mir::RuntimeFn, args: Vec<mir::Expr>) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::Runtime(function),
        },
        args,
    }
}

fn extern_call(function: mir::ExternFunctionId, args: Vec<mir::Expr>) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::Extern(function),
        },
        args,
    }
}

fn user_call(function: mir::FunctionId) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::User(function),
        },
        args: Vec::new(),
    }
}

fn call_stmt(call: mir::Call) -> mir::Statement {
    stmt(mir::StatementKind::Call(mir::CallEffect::Unit(call)))
}

fn call_value(destination: mir::LocalId, call: mir::Call) -> mir::Statement {
    stmt(mir::StatementKind::Call(mir::CallEffect::Value {
        destination,
        call,
    }))
}

/// `main` writes `"hello, world"` (core's managed `write` extern)
/// then calls `helper()`, which writes `"!"`.
fn hello_world() -> mir::Module {
    let mut b = Builder::new();
    let hello = b.string("hello, world");
    let bang = b.string("!");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let helper = b.user_fn(
        "helper",
        "scoop.helper",
        Arena::new(),
        vec![call_stmt(extern_call(write, vec![string_expr(bang)]))],
    );
    let main = b.main(
        Arena::new(),
        vec![
            call_stmt(extern_call(write, vec![string_expr(hello)])),
            call_stmt(user_call(helper)),
        ],
    );
    b.finish(main)
}

#[test]
fn lowers_hello_world() {
    let module = lower(&hello_world());

    // Globals: one per MIR string constant, same symbol and value.
    let globals: Vec<(&str, &str)> = module
        .globals
        .iter()
        .map(|(_, g)| match &g.init {
            lir::GlobalInit::StringConst(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::CString(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::Storage { .. } => unreachable!("hello has no storage globals"),
        })
        .collect();
    assert_eq!(
        globals,
        [("scoop.str.0", "hello, world"), ("scoop.str.1", "!")]
    );

    // Functions keep their mangled symbols; the entry symbol is the
    // fixed `scoop_main`.
    let symbols: Vec<&str> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
    assert_eq!(symbols, ["scoop.helper", mir::ENTRY_SYMBOL]);
    assert_eq!(module.entry_symbol, mir::ENTRY_SYMBOL);

    // The source declaration's typed intrinsic identity survives through
    // MIR and LIR. String metadata is a required singleton, not a layout
    // or descriptor that codegen has to rediscover by name.
    let string_layout = &module.meta.layouts[module.meta.well_known_layouts.string];
    let string_descriptor = descriptor(&module, module.meta.well_known_type_descriptors.string);
    assert_eq!(
        string_layout.kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(string_descriptor.symbol, lir::STRING_TD_SYMBOL);
    assert_eq!(string_descriptor.runtime_type_id, 1);
    assert!(string_descriptor.vtable.is_empty());
    assert_eq!(
        descriptor_values(&module)
            .filter(|descriptor| descriptor.symbol == lir::STRING_TD_SYMBOL)
            .count(),
        1
    );
    for representation in [
        lir::IntrinsicTypeRepresentation::Int,
        lir::IntrinsicTypeRepresentation::UInt,
        lir::IntrinsicTypeRepresentation::Boolean,
    ] {
        assert!(
            layout_values(&module).any(|layout| {
                layout.kind == lir::LayoutKind::Intrinsic(representation.clone())
            })
        );
    }

    // Golden dump locks the output structure.
    let expected = "\
Module
  global @scoop.str.0 = \"hello, world\"
  global @scoop.str.1 = \"!\"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop.helper() -> void
  block entry
    native_call void-target0 sig=void0 (ptr<managed>) effect=native-borrowed extern0(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop_main() -> void
  block entry
    native_call void-target0 sig=void0 (ptr<managed>) effect=native-borrowed extern0(global0)
    t0 = aggregate () : {}
    call void-target1 sig=void1 () effect=managed-safepoint local-fn0()
    t1 = aggregate () : {}
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn if_else_becomes_basic_blocks() {
    let mut b = Builder::new();
    let ok = b.string("ok");
    let ng = b.string("ng");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let else_block = cfg_block(&mut blocks, "if.else.2");
    let merge = cfg_block(&mut blocks, "if.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
            then_block,
            else_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ok)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ng)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  global @scoop.str.0 = \"ok\"
  global @scoop.str.1 = \"ng\"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop_main() -> void
  block entry
    cbr true then @if.then.1 else @if.else.2
  block if.then.1
    native_call void-target0 sig=void0 (ptr<managed>) effect=native-borrowed extern0(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.else.2
    native_call void-target1 sig=void1 (ptr<managed>) effect=native-borrowed extern0(global1)
    t1 = aggregate () : {}
    br @if.merge.3
  block if.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn while_becomes_basic_blocks() {
    // var n = 0; while (n < 3) { n = n + 1 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let n = locals.alloc(var("n", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let cond = cfg_block(&mut blocks, "while.cond.1");
    let body = cfg_block(&mut blocks, "while.body.2");
    let exit = cfg_block(&mut blocks, "while.exit.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![val_decl(n, mir::Expr::int(0))],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        cond,
        Vec::new(),
        mir::Terminator::Branch {
            cond: binary(
                mir::BinOp::IntLt,
                local_expr(n, mir::Type::Int),
                mir::Expr::int(3),
                mir::Type::Boolean,
            ),
            then_block: body,
            else_block: exit,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        body,
        vec![assign(
            n,
            binary(
                mir::BinOp::IntAdd,
                local_expr(n, mir::Type::Int),
                mir::Expr::int(1),
                mir::Type::Int,
            ),
        )],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 n: i64
  block entry
    store 0 -> local0
    br @while.cond.1
  block while.cond.1
    t0 = Lt local0, 3 : i1
    cbr t0 then @while.body.2 else @while.exit.3
  block while.body.2
    t1 = Add local0, 1 : i64
    store t1 -> local0
    br @while.cond.1
  block while.exit.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn and_short_circuits_through_blocks() {
    // val b = eq(s0, s1) && eq(s2, s3): the second comparison call
    // sits in its own block, executed only when the first is true.
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let s2 = b.string("c");
    let s3 = b.string("d");
    let string_eq = b.managed_scoop_extern(
        "coreStringEquals",
        "scoop_rt_string_eq",
        vec![mir::Type::String, mir::Type::String],
        mir::Type::Boolean,
    );
    let string_eq = |l, r| extern_call(string_eq, vec![string_expr(l), string_expr(r)]);
    let mut locals = Arena::new();
    let lhs = locals.alloc(local("$call.1", mir::Type::Boolean));
    let rhs = locals.alloc(local("$call.2", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs_block = cfg_block(&mut blocks, "logic.rhs.1");
    let short_block = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![call_value(lhs, string_eq(s0, s1))],
        mir::Terminator::Branch {
            cond: local_expr(lhs, mir::Type::Boolean),
            then_block: rhs_block,
            else_block: short_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs_block,
        vec![
            call_value(rhs, string_eq(s2, s3)),
            assign(result, local_expr(rhs, mir::Type::Boolean)),
        ],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short_block,
        vec![assign(result, mir::Expr::bool(false))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  global @scoop.str.0 = \"a\"
  global @scoop.str.1 = \"b\"
  global @scoop.str.2 = \"c\"
  global @scoop.str.3 = \"d\"
  extern ef0 coreStringEquals @scoop_rt_string_eq(ptr<managed>, ptr<managed>) -> i1 <scoop managed nounwind>
  fun @scoop_main() -> void
    local %0 $call.1: i1
    local %1 $call.2: i1
    local %2 b: i1
  block entry
    native_call t0 = direct-target0 sig=direct0 (ptr<managed>, ptr<managed>) -> i1 effect=native-borrowed extern0(global0, global1)
    store t0 -> local0
    cbr local0 then @logic.rhs.1 else @logic.short.2
  block logic.rhs.1
    native_call t1 = direct-target1 sig=direct1 (ptr<managed>, ptr<managed>) -> i1 effect=native-borrowed extern0(global2, global3)
    store t1 -> local1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store false -> local2
    br @logic.merge.3
  block logic.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn or_short_circuits_through_blocks() {
    // val b = x || y: when x is true, y is never evaluated.
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Boolean));
    let y = locals.alloc(local("y", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs = cfg_block(&mut blocks, "logic.rhs.1");
    let short = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![
            val_decl(x, mir::Expr::bool(true)),
            val_decl(y, mir::Expr::bool(false)),
        ],
        mir::Terminator::Branch {
            cond: local_expr(x, mir::Type::Boolean),
            then_block: short,
            else_block: rhs,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs,
        vec![assign(result, local_expr(y, mir::Type::Boolean))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short,
        vec![assign(result, mir::Expr::bool(true))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
  block entry
    store true -> local0
    store false -> local1
    cbr local0 then @logic.short.2 else @logic.rhs.1
  block logic.rhs.1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store true -> local2
    br @logic.merge.3
  block logic.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn compiler_runtime_calls_with_results_produce_typed_temps() {
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![
            call_value(
                s,
                runtime_call(
                    mir::RuntimeFn::StringConcat,
                    vec![string_expr(s0), string_expr(s1)],
                ),
            ),
            call_stmt(user_call(helper)),
        ],
    );
    let module = lower(&b.finish(main));

    // top_level order: helper first, then main.
    let function = &module.functions[1];
    let instructions = &function.blocks[function.entry].instructions;

    let lir::Instruction::Call { site } = &instructions[0] else {
        panic!("string concat must produce a value")
    };
    let concat_out = site.direct_out().expect("string concat result");
    assert_eq!(
        call_symbol(&module, function, site),
        "scoop_rt_string_concat"
    );
    assert_eq!(function.temps[concat_out].ty, lir::MANAGED_PTR);
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));

    // User calls return void; the Unit value is a fresh empty
    // aggregate.
    let lir::Instruction::Call { site } = &instructions[2] else {
        panic!("user calls must return void")
    };
    assert!(matches!(site, lir::CallSite::Void { .. }));
    assert_eq!(call_symbol(&module, function, site), "scoop.helper");
    let lir::Instruction::MakeAggregate { out, elements } = &instructions[3] else {
        panic!("a void call's Unit value must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));
}

#[test]
fn pointer_nulls_preserve_raw_and_code_provenance_in_lir() {
    assert!(matches!(
        lower_constant(&mir::ConstantValue::NullPtr),
        lir::ConstantValue::NullPointer(lir::PointerKind::Raw)
    ));
    assert!(matches!(
        lower_constant(&mir::ConstantValue::NullFunPtr),
        lir::ConstantValue::NullPointer(lir::PointerKind::Code)
    ));

    let mut blocks = Arena::new();
    let entry = blocks.alloc(lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: lir::Terminator::Return { value: None },
    });
    let function = lir::Function {
        gc_effect: lir::GcEffect::NoGc,
        symbol: "null_provenance".to_string(),
        params: Vec::new(),
        return_ty: lir::LirType::Void,
        call_targets: lir::CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    let globals = Arena::new();
    for kind in [
        lir::PointerKind::Managed,
        lir::PointerKind::Raw,
        lir::PointerKind::Code,
        lir::PointerKind::Metadata,
    ] {
        assert_eq!(
            function.value_ty(&globals, lir::Value::NullPointer(kind)),
            lir::LirType::Ptr(kind)
        );
    }
}

#[test]
fn arithmetic_and_comparison_ops_map_to_lir_ops() {
    let mut b = Builder::new();
    let int_cases = [
        (mir::BinOp::IntAdd, lir::BinOp::Add, lir::LirType::I64),
        (mir::BinOp::IntSub, lir::BinOp::Sub, lir::LirType::I64),
        (mir::BinOp::IntMul, lir::BinOp::Mul, lir::LirType::I64),
        (mir::BinOp::IntDiv, lir::BinOp::SDiv, lir::LirType::I64),
        (mir::BinOp::IntLt, lir::BinOp::Lt, lir::LirType::I1),
        (mir::BinOp::IntLe, lir::BinOp::Le, lir::LirType::I1),
        (mir::BinOp::IntGt, lir::BinOp::Gt, lir::LirType::I1),
        (mir::BinOp::IntGe, lir::BinOp::Ge, lir::LirType::I1),
        (mir::BinOp::IntEq, lir::BinOp::Eq, lir::LirType::I1),
        (mir::BinOp::IntNe, lir::BinOp::Ne, lir::LirType::I1),
    ];
    let mut statements: Vec<mir::Statement> = int_cases
        .iter()
        .map(|(mir_op, _, _)| {
            expr_stmt(binary(
                *mir_op,
                mir::Expr::int(1),
                mir::Expr::int(2),
                if matches!(
                    mir_op,
                    mir::BinOp::IntAdd
                        | mir::BinOp::IntSub
                        | mir::BinOp::IntMul
                        | mir::BinOp::IntDiv
                ) {
                    mir::Type::Int
                } else {
                    mir::Type::Boolean
                },
            ))
        })
        .collect();
    let bool_cases = [
        (mir::BinOp::BoolEq, lir::BinOp::Eq, lir::LirType::I1),
        (mir::BinOp::BoolNe, lir::BinOp::Ne, lir::LirType::I1),
    ];
    for (mir_op, _, _) in &bool_cases {
        statements.push(expr_stmt(binary(
            *mir_op,
            mir::Expr::bool(true),
            mir::Expr::bool(false),
            mir::Type::Boolean,
        )));
    }
    let main = b.main(Arena::new(), statements);
    let module = lower(&b.finish(main));

    let expected: Vec<(lir::BinOp, lir::LirType)> = int_cases
        .iter()
        .chain(bool_cases.iter())
        .map(|(_, lir_op, ty)| (*lir_op, ty.clone()))
        .collect();
    let function = &module.functions[0];
    let ops: Vec<(lir::BinOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .map(|instruction| {
            let lir::Instruction::BinOp { out, op, .. } = instruction else {
                panic!("expected a binary instruction")
            };
            (*op, function.temps[*out].ty.clone())
        })
        .collect();
    assert_eq!(ops, expected);
}

#[test]
fn unary_ops_map_to_lir_unops() {
    let mut b = Builder::new();
    let main = b.main(
        Arena::new(),
        vec![
            expr_stmt(expr(
                mir::Type::Int,
                mir::ExprKind::Unary {
                    op: mir::UnOp::IntNeg,
                    operand: Box::new(mir::Expr::int(1)),
                },
            )),
            expr_stmt(expr(
                mir::Type::Boolean,
                mir::ExprKind::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(mir::Expr::bool(true)),
                },
            )),
        ],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let ops: Vec<(lir::UnOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .map(|instruction| {
            let lir::Instruction::UnaryOp { out, op, .. } = instruction else {
                panic!("expected a unary instruction")
            };
            (*op, function.temps[*out].ty.clone())
        })
        .collect();
    assert_eq!(
        ops,
        [
            (lir::UnOp::Neg, lir::LirType::I64),
            (lir::UnOp::Not, lir::LirType::I1),
        ]
    );
}

#[test]
fn struct_values_keep_named_lir_identity() {
    let mut b = Builder::new();
    let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Struct(point)));
    let x = locals.alloc(local("x", mir::Type::Int));
    let main = b.main(
        locals,
        vec![
            val_decl(
                p,
                expr(
                    mir::Type::Struct(point),
                    mir::ExprKind::StructInit {
                        struct_id: point,
                        args: vec![mir::Expr::int(1), mir::Expr::int(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(p, mir::Type::Struct(point))),
                        index: 0,
                    },
                ),
            ),
        ],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let local_types: Vec<lir::LirType> =
        function.locals.iter().map(|(_, l)| l.ty.clone()).collect();
    assert_eq!(
        local_types,
        [
            lir::LirType::Struct(struct_def_id(point)),
            lir::LirType::I64,
        ]
    );

    let instructions = &function.blocks[function.entry].instructions;
    let lir::Instruction::MakeAggregate { out, elements } = &instructions[0] else {
        panic!("struct construction must build an aggregate")
    };
    assert_eq!(elements.len(), 2);
    assert_eq!(
        function.temps[*out].ty,
        lir::LirType::Struct(struct_def_id(point))
    );
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));
    let lir::Instruction::ExtractValue { out, index, .. } = &instructions[2] else {
        panic!("field access must extract from the aggregate")
    };
    assert_eq!(*index, 0);
    assert_eq!(function.temps[*out].ty, lir::LirType::I64);
    assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

    // The struct layout is in the meta.
    let layout = layout_values(&module)
        .find(|l| l.name == "Point")
        .expect("a layout per struct");
    assert_eq!((layout.size, layout.align), (16, 8));
    assert!(plain_refs(layout).is_empty());
}

#[test]
fn unit_is_the_empty_aggregate() {
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::Unit));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::unit())]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::Aggregate(Vec::new()));
    let lir::Instruction::MakeAggregate { out, elements } =
        &function.blocks[function.entry].instructions[0]
    else {
        panic!("Unit must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));

    // Unit itself gets no layout entry (it is just `{}`).
    assert!(!layout_values(&module).any(|l| l.name == "Unit"));
}

#[test]
fn uint_shares_ints_machine_word() {
    // UInt (spec 11.2, M9) maps onto `i64` at LIR — the same
    // machine word as Int, so codegen needs no UInt-specific
    // handling.
    let mut b = Builder::new();
    // A UInt field in a class: an 8-byte scalar slot behind the
    // 16-byte header, exactly like an Int field.
    let _c = b.class("C", None, &[("u", mir::Type::UInt)], empty_vtable(), vec![]);
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::UInt));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::int(1))]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::I64);

    let c_layout = layout_values(&module)
        .find(|l| l.name == "C")
        .expect("a layout per class");
    assert_eq!((c_layout.size, c_layout.align), (24, 8));
    assert!(plain_refs(c_layout).is_empty());
    let c_td = descriptor_values(&module)
        .find(|td| td.name == "C")
        .expect("a TypeDescriptor per class");
    assert_eq!(c_td.size, 24);
    assert_eq!(*fixed_scan(c_td), lir::RefScan::None);
}

#[test]
fn layouts_mark_reference_fields_for_the_gc() {
    let mut b = Builder::new();
    // String field behind one Int: the reference sits at offset 8.
    let s = b.strukt("S", &[("a", mir::Type::Int), ("s", mir::Type::String)]);
    // A String nested inside a tuple field, after a Boolean: the
    // tuple is 8-aligned, so it starts at offset 8.
    let pair = mir::Type::Tuple(vec![mir::Type::String, mir::Type::Int]);
    let _outer = b.strukt("Outer", &[("flag", mir::Type::Boolean), ("pair", pair)]);
    let _ = s;
    // A tuple type that only appears in code (padding: Boolean
    // then Int at offset 8).
    let mut locals = Arena::new();
    let _t = locals.alloc(local(
        "t",
        mir::Type::Tuple(vec![mir::Type::Boolean, mir::Type::Int]),
    ));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };

    // The String singleton is structurally separate; the remaining typed
    // intrinsic layouts stay in declaration order with ordinary layouts.
    let names: Vec<&str> = layout_values(&module).map(|l| l.name.as_str()).collect();
    assert_eq!(
        module.meta.layouts[module.meta.well_known_layouts.string].kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(
        names,
        [
            "S",
            "Outer",
            "Int",
            "UInt",
            "Boolean",
            "String",
            "(String, Int)",
            "(Boolean, Int)"
        ]
    );

    let string = &module.meta.layouts[module.meta.well_known_layouts.string];
    assert_eq!((string.size, string.align), (24, 8));
    assert!(string.fields.is_empty());

    // S { a: Int @0, s: String @8 }: size 16, align 8, refs [8].
    let s_layout = by_name("S");
    assert_eq!((s_layout.size, s_layout.align), (16, 8));
    assert_eq!(plain_refs(s_layout), [8]);

    // Outer { flag: Boolean @0, pair: (String, Int) @8 } with the
    // String at pair+0: size 24, align 8, refs [8].
    let outer = by_name("Outer");
    assert_eq!((outer.size, outer.align), (24, 8));
    assert_eq!(plain_refs(outer), [8]);

    // The tuple field type gets its own layout too.
    let pair_layout = by_name("(String, Int)");
    assert_eq!((pair_layout.size, pair_layout.align), (16, 8));
    assert_eq!(plain_refs(pair_layout), [0]);

    // (Boolean, Int): Int is 8-aligned, so it sits at offset 8 and
    // the size rounds up to 16.
    let padded = by_name("(Boolean, Int)");
    assert_eq!((padded.size, padded.align), (16, 8));
    assert!(plain_refs(padded).is_empty());
}

fn param(name: &str, ty: mir::Type, local: mir::LocalId) -> mir::Param {
    mir::Param {
        name: name.to_string(),
        ty,
        local,
    }
}

fn body_with_terminator(
    locals: Arena<mir::Local>,
    statements: Vec<mir::Statement>,
    terminator: mir::Terminator,
) -> mir::Body {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements,
        terminator,
        unwind: None,
    });
    mir::Body {
        locals,
        blocks,
        entry,
    }
}

fn returning_body(locals: Arena<mir::Local>, value: mir::Expr) -> mir::Body {
    body_with_terminator(
        locals,
        Vec::new(),
        mir::Terminator::Return { value: Some(value) },
    )
}

fn cfg_block(blocks: &mut Arena<mir::BasicBlock>, name: &str) -> mir::BlockId {
    blocks.alloc(mir::BasicBlock {
        name: name.to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    })
}

fn set_cfg_block(
    blocks: &mut Arena<mir::BasicBlock>,
    block: mir::BlockId,
    statements: Vec<mir::Statement>,
    terminator: mir::Terminator,
    unwind: Option<mir::BlockId>,
) {
    blocks[block].statements = statements;
    blocks[block].terminator = terminator;
    blocks[block].unwind = unwind;
}

fn cfg_block_named(body: &mir::Body, name: &str) -> mir::BlockId {
    body.blocks
        .iter()
        .find_map(|(id, block)| (block.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing MIR block `{name}`"))
}

fn single_catch_body(
    locals: Arena<mir::Local>,
    catch_local: mir::LocalId,
    catch_ty: mir::Type,
    body_statements: Vec<mir::Statement>,
    body_terminator: Option<mir::Terminator>,
    catch_statements: Vec<mir::Statement>,
) -> mir::Body {
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let unwind = cfg_block(&mut blocks, "try.unwind.1");
    let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
    let handler_pad = cfg_block(&mut blocks, "try.handler_pad.3");
    let handler_cleanup = cfg_block(&mut blocks, "try.handler_cleanup.4");
    let exit_pad = cfg_block(&mut blocks, "try.exit_pad.5");
    let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.6");
    let end = cfg_block(&mut blocks, "try.end.7");
    let try_body = cfg_block(&mut blocks, "try.body.8");
    let catch = cfg_block(&mut blocks, "try.catch.9");
    let next = cfg_block(&mut blocks, "try.next.10");

    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(try_body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Branch {
            cond: expr(
                mir::Type::Boolean,
                mir::ExprKind::IsInstance {
                    operand: Box::new(mir::Expr::caught_exception()),
                    check_ty: Box::new(catch_ty.clone()),
                },
            ),
            then_block: catch,
            else_block: next,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        handler_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(handler_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        handler_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        end,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    set_cfg_block(
        &mut blocks,
        try_body,
        body_statements,
        body_terminator.unwrap_or(mir::Terminator::Goto(end)),
        Some(unwind),
    );
    let mut catch_body = vec![val_decl(
        catch_local,
        expr(
            catch_ty.clone(),
            mir::ExprKind::Retype {
                operand: Box::new(mir::Expr::caught_exception()),
                ty: Box::new(catch_ty),
            },
        ),
    )];
    catch_body.extend(catch_statements);
    catch_body.push(stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)));
    set_cfg_block(
        &mut blocks,
        catch,
        catch_body,
        mir::Terminator::Goto(end),
        Some(handler_pad),
    );
    set_cfg_block(
        &mut blocks,
        next,
        Vec::new(),
        mir::Terminator::Rethrow {
            unwind: Some(exit_pad),
        },
        None,
    );

    mir::Body {
        locals,
        blocks,
        entry,
    }
}

#[test]
fn function_signatures_params_and_calls() {
    let mut b = Builder::new();
    // fun add(x: Int, y: Int): Int { return x + y }
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Int));
    let y = locals.alloc(local("y", mir::Type::Int));
    let add = b.user_fn_body(
        "add",
        "scoop.add",
        vec![param("x", mir::Type::Int, x), param("y", mir::Type::Int, y)],
        mir::Type::Int,
        returning_body(
            locals,
            binary(
                mir::BinOp::IntAdd,
                local_expr(x, mir::Type::Int),
                local_expr(y, mir::Type::Int),
                mir::Type::Int,
            ),
        ),
    );
    // main: val r = add(40, 2)
    let mut main_locals = Arena::new();
    let r = main_locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        main_locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(add),
                },
                args: vec![mir::Expr::int(40), mir::Expr::int(2)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // Parameters are SSA values (`Value::Param`), not stack slots;
    // the add body has no locals at all.
    let add_fn = &module.functions[0];
    assert_eq!(add_fn.params, [lir::LirType::I64, lir::LirType::I64]);
    assert_eq!(add_fn.return_ty, lir::LirType::I64);
    assert_eq!(add_fn.locals.len(), 0);

    let expected = "\
Module
  fun @scoop.add(i64, i64) -> i64
  block entry
    t0 = Add param0, param1 : i64
    ret t0
  fun @scoop_main() -> void
    local %0 r: i64
  block entry
    call t0 = direct-target0 sig=direct0 (i64, i64) -> i64 effect=managed-safepoint local-fn0(40, 2)
    store t0 -> local0
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn return_inside_a_branch_seals_its_block() {
    // fun f(x: Int): Int { if (true) { return x }; return 0 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let merge = cfg_block(&mut blocks, "if.merge.2");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
            then_block,
            else_block: merge,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then_block,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(local_expr(x, mir::Type::Int)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(mir::Expr::int(0)),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        vec![param("x", mir::Type::Int, x)],
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // The `return` seals the then block: no branch to the merge
    // block follows it.
    let expected = "\
Module
  fun @scoop.f(i64) -> i64
  block entry
    cbr true then @if.then.1 else @if.merge.2
  block if.then.1
    ret param0
  block if.merge.2
    ret 0
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

/// The LIR enum definition transposed from a MIR enum (the arenas
/// align 1:1).
fn edef(module: &lir::Module, id: mir::EnumId) -> &lir::EnumDef {
    &module.enums[lir::EnumDefId::from_raw(id.into_raw())]
}

/// main holding `o: Option<T>` through a None / tag / field /
/// wrap round-trip; shared shell of the two representation tests.
fn option_round_trip(name: &str, payload: mir::Type) -> mir::Module {
    let mut b = Builder::new();
    let option = b.option_enum(name, payload.clone());
    let option_ty = mir::Type::Enum(option, vec![payload.clone()]);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let t = locals.alloc(local("t", mir::Type::Int));
    let p = locals.alloc(local("p", payload.clone()));
    let o2 = locals.alloc(local("o2", option_ty.clone()));
    let main = b.main(
        locals,
        vec![
            // None
            val_decl(
                o,
                expr(
                    option_ty.clone(),
                    mir::ExprKind::VariantConstruct {
                        variant: 1,
                        fields: Vec::new(),
                    },
                ),
            ),
            val_decl(
                t,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::EnumTag(Box::new(local_expr(o, option_ty.clone()))),
                ),
            ),
            val_decl(
                p,
                expr(
                    payload.clone(),
                    mir::ExprKind::EnumField {
                        operand: Box::new(local_expr(o, option_ty.clone())),
                        variant: 0,
                        index: 0,
                    },
                ),
            ),
            val_decl(
                o2,
                expr(
                    option_ty,
                    mir::ExprKind::VariantConstruct {
                        variant: 0,
                        fields: vec![local_expr(p, payload)],
                    },
                ),
            ),
        ],
    );
    b.finish(main)
}

#[test]
fn option_of_string_uses_the_niche_representation() {
    // Option<String>: the payload maps to `Ptr`, so the value is
    // the pointer itself with None = null (spec 7.4).
    let module = lower(&option_round_trip("Option$S", mir::Type::String));

    let expected = "\
Module
  enum Option$S niche(payload_variant=0)
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: ptr<managed>
    local %3 o2: enum0
  block entry
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr<managed>
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$S size=8 align=8 enum-scan=refs[0]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn option_of_raw_pointer_uses_a_niche_without_gc_scanning() {
    let mut builder = Builder::new();
    let option = builder.option_enum("Option$P", mir::Type::Ptr(Box::new(mir::Type::Int)));
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));

    assert!(matches!(
        edef(&module, option).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    let layout = layout_values(&module)
        .find(|layout| layout.name == "Option$P")
        .expect("raw pointer option layout");
    let lir::LayoutKind::Enum { scan } = &layout.kind else {
        panic!("Option<Ptr<Int>> must retain its enum layout identity")
    };
    assert_eq!(*scan, lir::RefScan::None);
}

#[test]
fn option_of_int_uses_the_tagged_representation() {
    // Option<Int>: the `{ i64 tag, [8 x i8] payload }` tagged
    // form — size 16, align 8.
    let module = lower(&option_round_trip("Option$I", mir::Type::Int));

    let expected = "\
Module
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: i64
    local %3 o2: enum0
  block entry
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : i64
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn niche_detection_requires_option_isomorphic_pointer_shape() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let array_int = b.array("Array<Int>", mir::Type::Int);
    let option_array = b.option_enum("Option$Array$I", array_int);
    let option_i = b.option_enum("Option$I", mir::Type::Int);
    // Reversed declaration order: the payload variant comes second.
    let flip = b.enums.alloc(mir::EnumDef {
        name: "Flip".to_string(),
        gc_free: false,
        variants: vec![
            mir::VariantDef {
                name: "Naught".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: false,
                fields: vec![mir::Field {
                    name: "_1".to_string(),
                    ty: mir::Type::String,
                }],
            },
        ],
    });
    // Two variants, but the payload variant has two fields: tagged.
    let pair_or_none = b.enums.alloc(mir::EnumDef {
        name: "PairOrNone".to_string(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Pair".to_string(),
                gc_free: true,
                fields: vec![
                    mir::Field {
                        name: "_1".to_string(),
                        ty: mir::Type::Int,
                    },
                    mir::Field {
                        name: "_2".to_string(),
                        ty: mir::Type::Int,
                    },
                ],
            },
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(&b.finish(main));

    assert!(matches!(
        edef(&module, option_s).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    assert!(matches!(
        edef(&module, option_array).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    assert!(matches!(
        edef(&module, flip).repr,
        lir::EnumRepr::Niche { payload_variant: 1 }
    ));
    let lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    } = &edef(&module, option_i).repr
    else {
        panic!("Option<Int> must use the tagged representation")
    };
    assert_eq!(
        variants[0].fields,
        [lir::EnumFieldRepr {
            ty: lir::LirType::I64,
            offset: 8,
        }]
    );
    assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
    assert!(variants.iter().all(|variant| variant.gc_free));
    assert_eq!((*size, *align), (16, 8));
    let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, pair_or_none).repr else {
        panic!("PairOrNone must be tagged")
    };
    assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
    assert!(variants.iter().all(|variant| variant.gc_free));
}

#[test]
fn c_layout_keeps_packing_alignment_offsets_and_identity() {
    let mut b = Builder::new();
    let inner = b.c_strukt(
        "Inner",
        8,
        1,
        false,
        &[("flag", mir::Type::Boolean), ("value", mir::Type::Int)],
    );
    let outer = b.c_strukt(
        "Outer",
        16,
        2,
        true,
        &[
            ("tag", mir::Type::Boolean),
            ("inner", mir::Type::Struct(inner)),
            ("tail", mir::Type::Int),
        ],
    );
    let wrapped = b.enums.alloc(mir::EnumDef {
        name: "Wrapped".to_string(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::Struct(outer),
                }],
            },
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Number".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::Int,
                }],
            },
        ],
    });
    let outer_array = b.array("Array<Outer>", mir::Type::Struct(outer));
    let wrapped_array = b.array("Array<Wrapped>", mir::Type::Enum(wrapped, Vec::new()));
    let mut locals = Arena::new();
    locals.alloc(local("values", outer_array));
    locals.alloc(local("wrapped", wrapped_array));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

    let inner_def = &module.structs[struct_def_id(inner)];
    assert_eq!((inner_def.size, inner_def.align), (16, 8));
    assert_eq!(
        inner_def
            .fields
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>(),
        [
            lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
            lir::FieldLayout {
                offset: 1,
                access_align: 1,
            },
        ]
    );

    let outer_def = &module.structs[struct_def_id(outer)];
    assert_eq!(
        outer_def.fields[1].ty,
        lir::LirType::Struct(struct_def_id(inner))
    );
    assert_eq!((outer_def.size, outer_def.align), (32, 16));
    assert_eq!(
        outer_def
            .fields
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>(),
        [
            lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
            lir::FieldLayout {
                offset: 2,
                access_align: 2,
            },
            lir::FieldLayout {
                offset: 18,
                access_align: 2,
            },
        ]
    );
    assert!(outer_def.interior_mutable);

    let outer_layout = layout_values(&module)
        .find(|layout| layout.name == "Outer")
        .expect("Outer layout");
    assert_eq!((outer_layout.size, outer_layout.align), (32, 16));
    assert_eq!(
        outer_layout.fields,
        outer_def
            .fields
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>()
    );
    assert!(outer_layout.interior_mutable);
    let array_layout = array_metadata(&module, "Array<Outer>");
    assert_eq!(
        (array_layout.element_size, array_layout.element_align),
        (32, 16)
    );
    let wrapped_layout = layout_values(&module)
        .find(|layout| layout.name == "Wrapped")
        .expect("enum layout");
    assert_eq!((wrapped_layout.size, wrapped_layout.align), (48, 16));
    let wrapped_array = array_metadata(&module, "Array<Wrapped>");
    assert_eq!(
        (wrapped_array.element_size, wrapped_array.element_align),
        (48, 16)
    );
    assert!(lir::dump(&module).contains(
            "layout-meta Outer c-layout(aligned=16,packed=2) fields=[0@1,2@2,18@2] interior-mutable=true"
        ));
}

#[test]
fn recursive_scans_preserve_tagged_enums_in_aggregates_and_arrays() {
    let mut b = Builder::new();
    // enum Msg { Text(String), Pair(Boolean, String), Empty }
    let msg = b.enums.alloc(mir::EnumDef {
        name: "Msg".to_string(),
        gc_free: false,
        variants: vec![
            mir::VariantDef {
                name: "Text".to_string(),
                gc_free: false,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::String,
                }],
            },
            mir::VariantDef {
                name: "Pair".to_string(),
                gc_free: false,
                fields: vec![
                    mir::Field {
                        name: "flag".to_string(),
                        ty: mir::Type::Boolean,
                    },
                    mir::Field {
                        name: "s".to_string(),
                        ty: mir::Type::String,
                    },
                ],
            },
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });
    // A niche enum inside a struct: the value itself is the
    // reference.
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let _s = b.strukt(
        "S",
        &[("o", mir::Type::Enum(option_s, vec![mir::Type::String]))],
    );
    let msg_ty = mir::Type::Enum(msg, Vec::new());
    // Nested { flag: Boolean @0, msg: Msg @8 }. Msg's fixed ref
    // offsets compose without retaining or reading its tag.
    let nested = b.strukt(
        "Nested",
        &[("flag", mir::Type::Boolean), ("msg", msg_ty.clone())],
    );
    // Holder { head: String @16, nested: Nested @24 } combines an
    // unconditional reference with the nested enum scan.
    b.class(
        "Holder",
        None,
        &[
            ("head", mir::Type::String),
            ("nested", mir::Type::Struct(nested)),
        ],
        empty_vtable(),
        vec![],
    );
    let messages_array = b.array("Array<Msg>", msg_ty.clone());
    let nested_array = b.array("Array<Nested>", mir::Type::Struct(nested));
    let mut locals = Arena::new();
    locals.alloc(local("messages", messages_array));
    locals.alloc(local("nestedValues", nested_array));
    let main = b.main(locals, Vec::new());
    let module = lower(&b.finish(main));

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };

    // Msg has two ref-bearing variants, so Text and Pair receive
    // disjoint slots. Empty is the zero-sized shared pure region.
    let msg_layout = by_name("Msg");
    assert_eq!((msg_layout.size, msg_layout.align), (32, 8));
    let lir::LayoutKind::Enum { scan } = &msg_layout.kind else {
        panic!("an enum layout keeps fixed scan offsets")
    };
    assert_eq!(*scan, lir::RefScan::References(vec![8, 24]));
    let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, msg).repr else {
        panic!("Msg is tagged")
    };
    assert_ne!(variants[0].slot_offset, variants[1].slot_offset);
    assert_eq!(variants[2].slot_offset, 8);
    assert_eq!(
        variants[0].fields,
        [lir::EnumFieldRepr {
            ty: lir::LirType::Ptr(lir::PointerKind::Managed),
            offset: 8,
        }]
    );
    assert_eq!(
        variants[1]
            .fields
            .iter()
            .map(|field| field.offset)
            .collect::<Vec<_>>(),
        [16, 24]
    );

    // The niche layout: the payload variant is the reference
    // itself; the unit variant has none.
    let option_layout = by_name("Option$S");
    assert_eq!((option_layout.size, option_layout.align), (8, 8));
    let lir::LayoutKind::Enum { scan } = &option_layout.kind else {
        panic!("an enum layout keeps fixed scan offsets")
    };
    assert_eq!(*scan, lir::RefScan::References(vec![0]));

    // S { o: Option<String> }: the niche value at offset 0 is the
    // struct's reference field.
    let s_layout = by_name("S");
    assert_eq!((s_layout.size, s_layout.align), (8, 8));
    assert_eq!(plain_refs(s_layout), [0]);

    let nested_layout = by_name("Nested");
    assert_eq!((nested_layout.size, nested_layout.align), (40, 8));
    assert_eq!(
        nested_layout.kind,
        lir::LayoutKind::Plain {
            scan: lir::RefScan::References(vec![16, 32]),
        }
    );

    let holder_td = descriptor_values(&module)
        .find(|td| td.name == "Holder")
        .expect("Holder TypeDescriptor");
    assert_eq!((holder_td.size, holder_td.align), (64, 8));
    assert_eq!(
        *fixed_scan(holder_td),
        lir::RefScan::References(vec![16, 40, 56])
    );

    let array_scan = |name: &str| {
        array_scan(descriptor(
            &module,
            array_metadata(&module, name).type_descriptor,
        ))
        .clone()
    };
    assert_eq!(
        array_scan("Array<Msg>"),
        lir::RefScan::References(vec![8, 24])
    );
    assert_eq!(
        array_scan("Array<Nested>"),
        lir::RefScan::References(vec![16, 32])
    );
}

#[test]
fn trap_calls_branch_to_a_shared_trap_block() {
    // fun f(o: Option<Int>): Int { return o!! + o!! } — in the
    // mir-lower shape: each `o!!` is `if (tag == Some) { val $uw =
    // field0 } else { trap(msg) }`.
    let mut b = Builder::new();
    let option_i = b.option_enum("Option$I", mir::Type::Int);
    let option_ty = mir::Type::Enum(option_i, vec![mir::Type::Int]);
    let message = b.string("unwrap on None (function f)");
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let uw1 = locals.alloc(local("$uw.1", mir::Type::Int));
    let uw2 = locals.alloc(local("$uw.2", mir::Type::Int));
    let cond = || {
        binary(
            mir::BinOp::IntEq,
            expr(
                mir::Type::Int,
                mir::ExprKind::EnumTag(Box::new(local_expr(o, option_ty.clone()))),
            ),
            mir::Expr::int(0),
            mir::Type::Boolean,
        )
    };
    let init = |result| {
        val_decl(
            result,
            expr(
                mir::Type::Int,
                mir::ExprKind::EnumField {
                    operand: Box::new(local_expr(o, option_ty.clone())),
                    variant: 0,
                    index: 0,
                },
            ),
        )
    };
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then1 = cfg_block(&mut blocks, "if.then.1");
    let else1 = cfg_block(&mut blocks, "if.else.2");
    let merge1 = cfg_block(&mut blocks, "if.merge.3");
    let then2 = cfg_block(&mut blocks, "if.then.5");
    let else2 = cfg_block(&mut blocks, "if.else.6");
    let merge2 = cfg_block(&mut blocks, "if.merge.7");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: cond(),
            then_block: then1,
            else_block: else1,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then1,
        vec![init(uw1)],
        mir::Terminator::Goto(merge1),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else1,
        Vec::new(),
        mir::Terminator::Trap { message },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge1,
        Vec::new(),
        mir::Terminator::Branch {
            cond: cond(),
            then_block: then2,
            else_block: else2,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then2,
        vec![init(uw2)],
        mir::Terminator::Goto(merge2),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else2,
        Vec::new(),
        mir::Terminator::Trap { message },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge2,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(binary(
                mir::BinOp::IntAdd,
                local_expr(uw1, mir::Type::Int),
                local_expr(uw2, mir::Type::Int),
                mir::Type::Int,
            )),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        vec![param("o", option_ty, o)],
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(&b.finish(main));

    // Both `!!` share the one trap block of the function.
    let expected = "\
Module
  global @scoop.str.0 = \"unwrap on None (function f)\"
  global @scoop.cstr.0 = c\"unwrap on None (function f)\"
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
  fun @scoop.f(enum0) -> i64
    local %0 $uw.1: i64
    local %1 $uw.2: i64
  block entry
    t0 = enum_tag e0 param0 : i64
    t1 = Eq t0, 0 : i1
    cbr t1 then @if.then.1 else @if.else.2
  block if.then.1
    t2 = enum_field e0 v0 f0 param0 : i64
    store t2 -> local0
    br @if.merge.3
  block if.else.2
    br @unwrap.trap.1
  block if.merge.3
    t3 = enum_tag e0 param0 : i64
    t4 = Eq t3, 0 : i1
    cbr t4 then @if.then.5 else @if.else.6
  block if.then.5
    t5 = enum_field e0 v0 f0 param0 : i64
    store t5 -> local1
    br @if.merge.7
  block if.else.6
    br @unwrap.trap.1
  block if.merge.7
    t6 = Add local0, local1 : i64
    ret t6
  block unwrap.trap.1
    call void-target0 sig=void0 (ptr<raw>) effect=no-gc runtime @scoop_rt_trap(global1)
    unreachable
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn array_nodes_become_array_instructions() {
    // val a = [1, 2]; val x = a[0]; val n = a.size
    // val m = MutableArray(a); m[0] = 40
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", mir::Type::Int);
    let mutable_int = b.mutable_array("MutableArray<Int>", mir::Type::Int);
    let mir::Type::Class(array_class) = array_int else {
        unreachable!()
    };
    let mir::Type::Class(mutable_class) = mutable_int else {
        unreachable!()
    };
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", mir::Type::Class(array_class)));
    let x = locals.alloc(local("x", mir::Type::Int));
    let n = locals.alloc(local("n", mir::Type::Int));
    let m = locals.alloc(local("m", mir::Type::Class(mutable_class)));
    let main = b.main(
        locals,
        vec![
            val_decl(
                a,
                expr(
                    mir::Type::Class(array_class),
                    mir::ExprKind::ArrayLiteral {
                        array_type: array_class,
                        elements: vec![mir::Expr::int(1), mir::Expr::int(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::ArrayGet {
                        array_type: array_class,
                        array: Box::new(local_expr(a, mir::Type::Class(array_class))),
                        index: Box::new(mir::Expr::int(0)),
                    },
                ),
            ),
            val_decl(
                n,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::ArrayLen {
                        array_type: array_class,
                        operand: Box::new(local_expr(a, mir::Type::Class(array_class))),
                    },
                ),
            ),
            val_decl(
                m,
                expr(
                    mir::Type::Class(mutable_class),
                    mir::ExprKind::ArrayClone {
                        source_type: array_class,
                        target_type: mutable_class,
                        operand: Box::new(local_expr(a, mir::Type::Class(array_class))),
                    },
                ),
            ),
            stmt(mir::StatementKind::ArraySet {
                array_type: mutable_class,
                array: local_expr(m, mir::Type::Class(mutable_class)),
                index: mir::Expr::int(0),
                value: mir::Expr::int(40),
            }),
        ],
    );
    let module = lower(&b.finish(main));

    // Both nominal applications have managed-pointer storage, while every
    // instruction references its complete typed metadata record.
    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 a: ptr<managed>
    local %1 x: i64
    local %2 n: i64
    local %3 m: ptr<managed>
  block entry
    t0 = array_alloc array0 (1, 2) : ptr<managed>
    store t0 -> local0
    t1 = array_get array0 local0 0 : i64
    store t1 -> local1
    t2 = array_len array0 local0 : i64
    store t2 -> local2
    t3 = array_clone array1 local0 : ptr<managed>
    store t3 -> local3
    array_set array1 local3 0 40
    ret
  array-type array0 Array<Int> kind=immutable element=i64 size=8 align=8 scan=none td=td0
  array-type array1 MutableArray<Int> kind=mutable element=i64 size=8 align=8 scan=none td=td1
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn array_layouts_mark_reference_elements() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
    let option_string = mir::Type::Enum(option_s, vec![mir::Type::String]);
    let array_int = b.array("Array<Int>", mir::Type::Int);
    let array_string = b.array("Array<String>", mir::Type::String);
    let array_option = b.array("Array<Option$S<String>>", option_string);
    let array_point = b.array("Array<Point>", mir::Type::Struct(point));
    let array_nested = b.array("Array<Array<Int>>", array_int.clone());
    let mut locals = Arena::new();
    let _ints = locals.alloc(local("ints", array_int.clone()));
    let _strings = locals.alloc(local("strings", array_string));
    let _options = locals.alloc(local("options", array_option));
    let _points = locals.alloc(local("points", array_point));
    let _nested = locals.alloc(local("nested", array_nested));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

    let array_layout = |name: &str| {
        let array = array_metadata(&module, name);
        (
            array.element_size,
            array.element_align,
            array_scan(descriptor(&module, array.type_descriptor)).clone(),
        )
    };
    // size / align are element-level: the element stride and
    // alignment of the region after header + size.
    assert_eq!(array_layout("Array<Int>"), (8, 8, lir::RefScan::None));
    // String elements are references.
    assert_eq!(
        array_layout("Array<String>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
    // Option<String> uses the niche representation — a bare
    // pointer, hence a reference element.
    assert_eq!(
        array_layout("Array<Option$S<String>>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
    // A value-type element is inline: the Point stride.
    assert_eq!(array_layout("Array<Point>"), (16, 8, lir::RefScan::None));
    // An array element is itself a reference; the nested element
    // type gets its own layout too.
    assert_eq!(
        array_layout("Array<Array<Int>>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
}

#[test]
fn array_fields_are_reference_fields() {
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", mir::Type::Int);
    let _holder = b.strukt("Holder", &[("flag", mir::Type::Boolean), ("xs", array_int)]);
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // flag @0 (1 byte), xs @8: an array value is a pointer-sized
    // reference.
    let holder = layout_values(&module)
        .find(|l| l.name == "Holder")
        .expect("a layout per struct");
    assert_eq!((holder.size, holder.align), (16, 8));
    assert_eq!(plain_refs(holder), [8]);
    // The complete intrinsic class application exists independently of
    // whether a function contains an array instruction.
    assert_eq!(
        array_metadata(&module, "Array<Int>").element,
        lir::LirType::I64
    );
}

// ---- M6: reference types ----

fn empty_vtable() -> Vec<mir::TableSlot> {
    Vec::new()
}

#[test]
fn virtual_calls_load_the_vtable_and_call_indirect() {
    let mut b = Builder::new();
    let c = b.class("C", None, &[], empty_vtable(), vec![]);
    // `C.m(this: C): Int { return 1 }`.
    let mut method_locals = Arena::new();
    let this = method_locals.alloc(local("this", mir::Type::Class(c)));
    let m = b.user_fn_body(
        "C.m",
        "scoop.C.m",
        vec![param("this", mir::Type::Class(c), this)],
        mir::Type::Int,
        returning_body(method_locals, mir::Expr::int(1)),
    );
    b.classes[c].vtable.push(mir::TableSlot::Function(m));
    // main: `val p: C; val r = p.m()` (the first ordinary virtual slot).
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let r = locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Virtual { slot: 0 },
                    callee: mir::Callee::User(m),
                },
                args: vec![local_expr(p, mir::Type::Class(c))],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // The receiver's object header (index 0) holds the TD; its
    // vtable pointer is ScoopTypeDescriptor field 5; the callee is
    // vtable[0].
    let expected = "\
Module
  fun @scoop.C.m(ptr<managed>) -> i64
  block entry
    ret 1
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
    local %1 r: i64
  block entry
    t0 = heap_load local0 +0 : ptr<metadata>
    t1 = heap_load t0 +40 : ptr<metadata>
    call t2 = direct-target0 sig=direct0 (ptr<managed>) -> i64 effect=managed-safepoint dispatch[Virtual:0] t1(local0)
    store t2 -> local1
    ret
  td td0 C @scoop_td_C type-id=2 size=16 parent=none vtable=[local-fn0] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout C size=16 align=8 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn interface_calls_look_up_the_itable() {
    let mut b = Builder::new();
    let iface = b.interface("Describable", &["describe", "label"]);
    // The interface method shell (signature only, never emitted).
    let mut shell_locals = Arena::new();
    let this = shell_locals.alloc(local("this", mir::Type::Interface(iface)));
    let label = b.decl_fn(
        "Describable.label",
        "scoop.Describable.label",
        vec![param("this", mir::Type::Interface(iface), this)],
        mir::Type::Int,
    );
    // main: `val i: Describable; val r = i.label()` (itable slot 1).
    let mut locals = Arena::new();
    let i = locals.alloc(local("i", mir::Type::Interface(iface)));
    let r = locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: iface,
                        slot: 1,
                    },
                    callee: mir::Callee::User(label),
                },
                args: vec![local_expr(i, mir::Type::Interface(iface))],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // `scoop_rt_itable_lookup(td, iface_td)` finds the table; the
    // interface TD is a typed metadata reference, not an ordinary
    // globals-arena entry.
    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 i: ptr<managed>
    local %1 r: i64
  block entry
    t0 = heap_load local0 +0 : ptr<metadata>
    call t1 = direct-target0 sig=direct0 (ptr<metadata>, ptr<metadata>) -> ptr<metadata> effect=no-gc runtime @scoop_rt_itable_lookup(t0, td0)
    call t2 = direct-target1 sig=direct1 (ptr<managed>) -> i64 effect=managed-safepoint dispatch[Interface:1] t1(local0)
    store t2 -> local1
    ret
  td td0 Describable @scoop_td_Describable type-id=2 size=0 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn type_descriptors_carry_tables_parents_and_itables() {
    let mut b = Builder::new();
    let iface = b.interface("I", &["m"]);
    // Methods (bodies don't matter for the meta).
    let mut m_locals = Arena::new();
    let base_m_this = m_locals.alloc(local("this", mir::Type::Int));
    let base_m = b.user_fn_full(
        "Base.m",
        "scoop.Base.m",
        vec![param("this", mir::Type::Int, base_m_this)],
        mir::Type::Unit,
        m_locals,
        vec![],
    );
    let mut dm_locals = Arena::new();
    let derived_m_this = dm_locals.alloc(local("this", mir::Type::Int));
    let derived_m = b.user_fn_full(
        "Derived.m",
        "scoop.Derived.m",
        vec![param("this", mir::Type::Int, derived_m_this)],
        mir::Type::Unit,
        dm_locals,
        vec![],
    );
    let mut dm2_locals = Arena::new();
    let derived_m2_this = dm2_locals.alloc(local("this", mir::Type::Int));
    let derived_m2 = b.user_fn_full(
        "Derived.m2",
        "scoop.Derived.m2",
        vec![param("this", mir::Type::Int, derived_m2_this)],
        mir::Type::Unit,
        dm2_locals,
        vec![],
    );
    // Base implements I; Derived overrides `m` and adds `m2`.
    let mut base_vtable = empty_vtable();
    base_vtable.push(mir::TableSlot::Function(base_m));
    let base = b.class(
        "Base",
        None,
        &[("a", mir::Type::Int)],
        base_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(base_m)],
        }],
    );
    let mut derived_vtable = empty_vtable();
    derived_vtable.push(mir::TableSlot::Function(derived_m));
    derived_vtable.push(mir::TableSlot::Function(derived_m2));
    let _derived = b.class(
        "Derived",
        Some(base),
        // mir-lower flattens the base prefix into the field list.
        &[("a", mir::Type::Int), ("b", mir::Type::String)],
        derived_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(derived_m)],
        }],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // Interfaces first (itable keys), then classes
    // base-before-derived — references always name
    // already-emitted entries.
    assert_eq!(module.meta.type_descriptors.len(), 4);
    let descriptor_by_name = |name: &str| {
        module
            .meta
            .type_descriptors
            .iter()
            .find_map(|(id, descriptor)| {
                (descriptor.name == name).then_some((lir::TypeDescriptorRef::Local(id), descriptor))
            })
            .unwrap_or_else(|| panic!("missing descriptor {name}"))
    };
    let (i_ref, i_td) = descriptor_by_name("I");
    let (base_ref, base_td) = descriptor_by_name("Base");
    let (_, derived_td) = descriptor_by_name("Derived");
    let (_, string_td) = descriptor_by_name("String");
    assert_eq!(i_td.symbol, "scoop_td_I");
    assert_eq!((i_td.size, i_td.align), (0, 0));
    assert!(i_td.parent.is_none());

    assert_eq!(base_td.name, "Base");
    assert_eq!(base_td.symbol, "scoop_td_Base");
    // 16-byte header + Int @16 → size 24.
    assert_eq!((base_td.size, base_td.align), (24, 8));
    assert_eq!(*fixed_scan(base_td), lir::RefScan::None);
    assert!(base_td.parent.is_none());
    assert_eq!(
        base_td.vtable,
        [lir::DispatchEntry {
            callable: lir::CallableRef::Local(lir::LocalFunctionId::from_u32(0)),
        }]
    );
    assert_eq!(base_td.itables.len(), 1);
    assert_eq!(base_td.itables[0].interface, i_ref);
    assert_eq!(base_td.itables[0].slots, base_td.vtable);

    assert_eq!(derived_td.symbol, "scoop_td_Derived");
    assert_eq!(derived_td.parent, Some(base_ref));
    // header 16 + Int @16 + String @24 → size 32; the String is
    // the one reference.
    assert_eq!((derived_td.size, derived_td.align), (32, 8));
    assert_eq!(*fixed_scan(derived_td), lir::RefScan::References(vec![24]));
    assert_eq!(derived_td.vtable.len(), 2);
    assert_eq!(derived_td.itables[0].slots, [derived_td.vtable[0]]);
    assert_eq!(string_td.symbol, lir::STRING_TD_SYMBOL);
}

#[test]
fn class_layouts_shift_ref_offsets_by_the_header() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[
            ("a", mir::Type::Int),
            ("s", mir::Type::String),
            ("flag", mir::Type::Boolean),
            ("r", mir::Type::Any),
        ],
        empty_vtable(),
        vec![],
    );
    let _ = c;
    // A boxed value type: header + the inline payload; references
    // inside the payload shift by the header too.
    let s = b.strukt("S", &[("x", mir::Type::Int), ("s", mir::Type::String)]);
    let boxed = b.class(
        "box$S",
        None,
        &[("value", mir::Type::Struct(s))],
        empty_vtable(),
        vec![],
    );
    let main = b.main(Arena::new(), vec![]);
    let mut mir_module = b.finish(main);
    mir_module.meta.boxed_types.push(mir::BoxedType {
        payload: mir::Type::Struct(s),
        class: boxed,
    });
    let module = lower(&mir_module);

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };
    // C: header 16; a @16, s @24, flag @32, r @40 → size 48.
    let c_layout = by_name("C");
    assert_eq!((c_layout.size, c_layout.align), (48, 8));
    assert_eq!(plain_refs(c_layout), [24, 40]);
    // box$S: header 16 + payload { Int @0, String @8 } @16 → the
    // String lands at 24.
    let boxed_layout = by_name("box$S");
    assert_eq!((boxed_layout.size, boxed_layout.align), (32, 8));
    assert_eq!(plain_refs(boxed_layout), [24]);
    // The TypeDescriptors carry the same reference offsets.
    let td = |name: &str| {
        descriptor_values(&module)
            .find(|td| td.name == name)
            .unwrap_or_else(|| panic!("missing TypeDescriptor for {name}"))
    };
    assert_eq!(*fixed_scan(td("C")), lir::RefScan::References(vec![24, 40]));
    assert_eq!(*fixed_scan(td("box$S")), lir::RefScan::References(vec![24]));
    assert!(td("C").parent.is_none());
    assert!(td("box$S").parent.is_none());
}

#[test]
fn box_unbox_and_is_instance_lower_to_runtime_calls() {
    let mut b = Builder::new();
    let s = b.strukt("S", &[("x", mir::Type::Int)]);
    // mir-lower registers the boxed class of every checked / boxed
    // value type.
    let boxed = b.class(
        "box$D1_SX",
        None,
        &[("value", mir::Type::Struct(s))],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", mir::Type::Any));
    let v = locals.alloc(local("v", mir::Type::Struct(s)));
    let chk = locals.alloc(local("chk", mir::Type::Boolean));
    let main = b.main(
        locals,
        vec![
            val_decl(
                a,
                expr(
                    mir::Type::Any,
                    mir::ExprKind::Box(Box::new(expr(
                        mir::Type::Struct(s),
                        mir::ExprKind::StructInit {
                            struct_id: s,
                            args: vec![mir::Expr::int(1)],
                        },
                    ))),
                ),
            ),
            val_decl(
                v,
                expr(
                    mir::Type::Struct(s),
                    mir::ExprKind::Unbox(Box::new(local_expr(a, mir::Type::Any))),
                ),
            ),
            val_decl(
                chk,
                expr(
                    mir::Type::Boolean,
                    mir::ExprKind::IsInstance {
                        operand: Box::new(local_expr(a, mir::Type::Any)),
                        check_ty: Box::new(mir::Type::Struct(s)),
                    },
                ),
            ),
        ],
    );
    let mut mir_module = b.finish(main);
    mir_module.meta.boxed_types.push(mir::BoxedType {
        payload: mir::Type::Struct(s),
        class: boxed,
    });
    let module = lower(&mir_module);

    assert!(
        module
            .globals
            .iter()
            .all(|(_, global)| !global.symbol.starts_with("scoop_td_")),
        "TypeDescriptors must never be represented by ordinary globals"
    );

    // Box → `scoop_rt_box(td, payload, size)`; Unbox → the payload
    // field behind the header; `is` → `scoop_rt_is_instance(obj,
    // td)`. Both checks share one typed descriptor reference.
    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 a: ptr<managed>
    local %1 v: struct0
    local %2 chk: i1
    local %3 $sc.1: struct0
  block entry
    t0 = aggregate (1) : struct0
    store t0 -> local3
    t1 = local_address local3 : ptr
    call t2 = direct-target0 sig=direct0 (ptr<metadata>, ptr<raw>, i64) -> ptr<managed> effect=managed-safepoint runtime @scoop_rt_box(td0, t1, 8)
    store t2 -> local0
    t3 = heap_load local0 +16 : struct0
    store t3 -> local1
    call t4 = direct-target1 sig=direct1 (ptr<managed>, ptr<metadata>) -> i1 effect=no-gc runtime @scoop_rt_is_instance(local0, td0)
    store t4 -> local2
    ret
  td td0 box$D1_SX @scoop_td_box$D1_SX type-id=2 size=24 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout S size=8 align=8 refs=[]
  layout box$D1_SX size=24 align=8 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn gc_intrinsics_exchange_words_with_the_runtime() {
    // The MIR shapes produced by M12's ordinary GC wrappers:
    // `_pin` / `_getGcHandle` return
    // the runtime's raw word into the handle struct, `unpin` /
    // `releaseGcHandle` unwrap field 0 for the reverse call, and
    // the hooks are a void call / an i64 result.
    let mut b = Builder::new();
    let pinned_ptr = b.strukt("PinnedPtr$S", &[("raw", mir::Type::UInt)]);
    let gc_handle = b.strukt("GcHandle$S", &[("raw", mir::Type::UInt)]);
    let mut locals = Arena::new();
    let v = locals.alloc(local("v", mir::Type::String));
    let raw_pin = locals.alloc(local("$call.1", mir::Type::UInt));
    let h = locals.alloc(local("h", mir::Type::Struct(pinned_ptr)));
    let gc1 = locals.alloc(local("$gc.1", mir::Type::String));
    let p = locals.alloc(local("p", mir::Type::String));
    let raw_handle = locals.alloc(local("$call.2", mir::Type::UInt));
    let gh = locals.alloc(local("gh", mir::Type::Struct(gc_handle)));
    let gc2 = locals.alloc(local("$gc.2", mir::Type::String));
    let p2 = locals.alloc(local("p2", mir::Type::String));
    let n = locals.alloc(local("n", mir::Type::UInt));
    let main = b.main(
        locals,
        vec![
            call_value(
                raw_pin,
                runtime_call(mir::RuntimeFn::Pin, vec![local_expr(v, mir::Type::String)]),
            ),
            val_decl(
                h,
                expr(
                    mir::Type::Struct(pinned_ptr),
                    mir::ExprKind::StructInit {
                        struct_id: pinned_ptr,
                        args: vec![local_expr(raw_pin, mir::Type::UInt)],
                    },
                ),
            ),
            call_value(
                gc1,
                runtime_call(
                    mir::RuntimeFn::Unpin,
                    vec![expr(
                        mir::Type::UInt,
                        mir::ExprKind::FieldAccess {
                            receiver: Box::new(local_expr(h, mir::Type::Struct(pinned_ptr))),
                            index: 0,
                        },
                    )],
                ),
            ),
            val_decl(p, local_expr(gc1, mir::Type::String)),
            call_value(
                raw_handle,
                runtime_call(
                    mir::RuntimeFn::GetHandle,
                    vec![local_expr(v, mir::Type::String)],
                ),
            ),
            val_decl(
                gh,
                expr(
                    mir::Type::Struct(gc_handle),
                    mir::ExprKind::StructInit {
                        struct_id: gc_handle,
                        args: vec![local_expr(raw_handle, mir::Type::UInt)],
                    },
                ),
            ),
            call_value(
                gc2,
                runtime_call(
                    mir::RuntimeFn::ReleaseHandle,
                    vec![expr(
                        mir::Type::UInt,
                        mir::ExprKind::FieldAccess {
                            receiver: Box::new(local_expr(gh, mir::Type::Struct(gc_handle))),
                            index: 0,
                        },
                    )],
                ),
            ),
            val_decl(p2, local_expr(gc2, mir::Type::String)),
            call_stmt(runtime_call(mir::RuntimeFn::GcCollect, vec![])),
            call_value(n, runtime_call(mir::RuntimeFn::GcStats, vec![])),
        ],
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  fun @scoop_main() -> void
    local %0 v: ptr<managed>
    local %1 $call.1: i64
    local %2 h: struct0
    local %3 $gc.1: ptr<managed>
    local %4 p: ptr<managed>
    local %5 $call.2: i64
    local %6 gh: struct1
    local %7 $gc.2: ptr<managed>
    local %8 p2: ptr<managed>
    local %9 n: i64
  block entry
    call t0 = direct-target0 sig=direct0 (ptr<managed>) -> i64 effect=no-gc runtime @scoop_rt_pin(local0)
    store t0 -> local1
    t1 = aggregate (local1) : struct0
    store t1 -> local2
    t2 = extract local2, 0 : i64
    call t3 = direct-target1 sig=direct1 (i64) -> ptr<managed> effect=no-gc runtime @scoop_rt_unpin(t2)
    store t3 -> local3
    store local3 -> local4
    call t4 = direct-target2 sig=direct2 (ptr<managed>) -> i64 effect=no-gc runtime @scoop_rt_get_handle(local0)
    store t4 -> local5
    t5 = aggregate (local5) : struct1
    store t5 -> local6
    t6 = extract local6, 0 : i64
    call t7 = direct-target3 sig=direct3 (i64) -> ptr<managed> effect=no-gc runtime @scoop_rt_release_handle(t6)
    store t7 -> local7
    store local7 -> local8
    call void-target0 sig=void0 () effect=managed-safepoint runtime @scoop_rt_gc_collect()
    t8 = aggregate () : {}
    call t9 = direct-target4 sig=direct4 () -> i64 effect=no-gc runtime @scoop_rt_gc_stats()
    store t9 -> local9
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout PinnedPtr$S size=8 align=8 refs=[]
  layout GcHandle$S size=8 align=8 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn class_field_reads_are_heap_loads() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("a", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![val_decl(
            s,
            expr(
                mir::Type::String,
                mir::ExprKind::FieldAccess {
                    receiver: Box::new(local_expr(p, mir::Type::Class(c))),
                    index: 1,
                },
            ),
        )],
    );
    let module = lower(&b.finish(main));

    // The String field follows the 16-byte header and Int field,
    // so its natural byte offset is 24.
    let function = &module.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    let lir::Instruction::HeapLoad { out, offset, .. } = &instructions[0] else {
        panic!("a class field read must be a heap object load")
    };
    assert_eq!(*offset, 24);
    assert_eq!(function.temps[*out].ty, lir::MANAGED_PTR);
}

#[test]
fn class_init_allocates_and_stores_fields() {
    // The ctor body mir-lower generates for
    // `class Point(val x: Int, val s: String)`:
    // `return ClassInit Point [x, s]` — allocation plus one heap
    // store per flattened field.
    let mut b = Builder::new();
    let str_x = b.string("x");
    let point = b.class(
        "Point",
        None,
        &[("x", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut ctor_locals = Arena::new();
    let x = ctor_locals.alloc(local("x", mir::Type::Int));
    let s = ctor_locals.alloc(local("s", mir::Type::String));
    let ctor = b.user_fn_body(
        "ctor.Point",
        "scoop.ctor.Point",
        vec![
            param("x", mir::Type::Int, x),
            param("s", mir::Type::String, s),
        ],
        mir::Type::Class(point),
        returning_body(
            ctor_locals,
            expr(
                mir::Type::Class(point),
                mir::ExprKind::ClassInit {
                    class_id: point,
                    args: vec![
                        local_expr(x, mir::Type::Int),
                        local_expr(s, mir::Type::String),
                    ],
                },
            ),
        ),
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(point)));
    let main = b.main(
        locals,
        vec![call_value(
            p,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(ctor),
                },
                args: vec![mir::Expr::int(1), string_expr(str_x)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // `scoop_rt_alloc(td, size)` with the class layout size (16
    // header + Int @16 + String @24 = 32), then the fields at
    // those byte offsets.
    let expected = "\
Module
  global @scoop.str.0 = \"x\"
  fun @scoop.ctor.Point(i64, ptr<managed>) -> ptr<managed>
  block entry
    call t0 = direct-target0 sig=direct0 (ptr<metadata>, i64) -> ptr<managed> effect=managed-safepoint runtime @scoop_rt_alloc(td0, 32)
    heap_store t0 +16 param0
    heap_store t0 +24 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
  block entry
    call t0 = direct-target0 sig=direct0 (i64, ptr<managed>) -> ptr<managed> effect=managed-safepoint local-fn0(1, global0)
    store t0 -> local0
    ret
  td td0 Point @scoop_td_Point type-id=2 size=32 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Point size=32 align=8 refs=[24]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn field_set_lowers_to_a_heap_store() {
    // `p.y = 3`: MIR FieldSet index 1 → byte offset 24 after the
    // 16-byte header and the first Int field.
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("x", mir::Type::Int), ("y", mir::Type::Int)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let main = b.main(
        locals,
        vec![stmt(mir::StatementKind::FieldSet {
            object: local_expr(p, mir::Type::Class(c)),
            index: 1,
            value: mir::Expr::int(3),
        })],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    let lir::Instruction::HeapStore {
        object,
        offset: 24,
        value,
    } = &instructions[0]
    else {
        panic!("a FieldSet must lower to a HeapStore")
    };
    assert!(matches!(object, lir::Value::Local(_)));
    assert!(matches!(value, lir::Value::IntConst(3)));
}

#[test]
fn a_trap_only_body_seals_the_function() {
    // mir-lower's abstract-method stub is a single trap call: the
    // block is sealed by the trap branch, so the "non-Unit
    // functions must end with `return`" check must not fire (it
    // applies to hir-lower-produced bodies that fall off the end,
    // not to noreturn bodies like this one).
    let mut b = Builder::new();
    let message = b.string("call to abstract method `Base.id`");
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Any));
    let _stub = b.user_fn_full(
        "Base.id",
        "scoop.Base.id",
        vec![param("this", mir::Type::Any, this)],
        mir::Type::Int,
        locals,
        vec![call_stmt(runtime_call(
            mir::RuntimeFn::Trap,
            vec![string_expr(message)],
        ))],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    assert!(matches!(
        function.blocks[function.entry].terminator,
        lir::Terminator::Br(_)
    ));
}

/// `try { throw e } catch (e: MyError) { handled() }` minus the
/// throw — the shared shell of the M8 tests: `helper()` in the
/// body, `handled()` in the catch, `cleanup()` in the finally.
fn try_shell(finally: bool) -> (Builder, mir::FunctionId, mir::FunctionId, mir::FunctionId) {
    let mut b = Builder::new();
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let handled = b.user_fn("handled", "scoop.handled", Arena::new(), vec![]);
    let cleanup = if finally {
        b.user_fn("cleanup", "scoop.cleanup", Arena::new(), vec![])
    } else {
        helper
    };
    (b, helper, handled, cleanup)
}

fn my_error(b: &mut Builder) -> mir::ClassId {
    b.class("MyError", None, &[], vec![], vec![])
}

#[test]
fn try_catch_lowers_to_invoke_landingpad_and_rethrow() {
    let (mut b, helper, handled, _) = try_shell(false);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        single_catch_body(
            locals,
            e,
            mir::Type::Class(my_error),
            vec![call_stmt(user_call(helper))],
            None,
            vec![call_stmt(user_call(handled))],
        ),
    );
    let module = lower(&b.finish(main));

    let expected = "\
Module
  fun @scoop.helper() -> void
  block entry
    ret
  fun @scoop.handled() -> void
  block entry
    ret
  fun @scoop_main() -> void
    local %0 e: ptr<managed>
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    br @try.body.8
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    call t3 = direct-target0 sig=direct0 (ptr<managed>, ptr<metadata>) -> i1 effect=no-gc runtime @scoop_rt_is_instance(local3, td0)
    cbr t3 then @try.catch.9 else @try.next.10
  block try.handler_pad.3
    (t4, t5) = cleanup_pad : (exception_record, ptr<raw>)
    store t4 -> local1
    store t5 -> local2
    br @try.handler_cleanup.4
  block try.handler_cleanup.4
    end_catch
    resume local1
  block try.exit_pad.5
    (t6, t7) = cleanup_pad : (exception_record, ptr<raw>)
    store t6 -> local1
    store t7 -> local2
    br @try.exit_cleanup.6
  block try.exit_cleanup.6
    end_catch
    resume local1
  block try.end.7
    ret
  block try.body.8
    invoke void-target0 sig=void0 () effect=managed-safepoint local-fn0() normal @invoke.normal.1 unwind @try.unwind.1
    br @invoke.normal.1
  block try.catch.9
    store local3 -> local0
    invoke void-target1 sig=void1 () effect=managed-safepoint local-fn1() normal @invoke.normal.2 unwind @try.handler_pad.3
    br @invoke.normal.2
  block try.next.10
    invoke void-target2 sig=void2 () effect=no-gc runtime @scoop_rt_rethrow() normal @rethrow.normal.3 unwind @try.exit_pad.5
    br @rethrow.normal.3
  block invoke.normal.1
    t8 = aggregate () : {}
    br @try.end.7
  block invoke.normal.2
    t9 = aggregate () : {}
    end_catch
    br @try.end.7
  block rethrow.normal.3
    unreachable
  td td0 MyError @scoop_td_MyError type-id=2 size=16 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn finally_runs_on_the_normal_catch_and_rethrow_paths() {
    let (mut b, helper, handled, cleanup) = try_shell(true);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let mut body = single_catch_body(
        locals,
        e,
        mir::Type::Class(my_error),
        vec![call_stmt(user_call(helper))],
        None,
        vec![call_stmt(user_call(handled))],
    );
    let try_body = cfg_block_named(&body, "try.body.8");
    let catch = cfg_block_named(&body, "try.catch.9");
    let next = cfg_block_named(&body, "try.next.10");
    let handler_cleanup = cfg_block_named(&body, "try.handler_cleanup.4");
    let exit_pad = cfg_block_named(&body, "try.exit_pad.5");
    let end = cfg_block_named(&body, "try.end.7");
    let normal_finally = cfg_block(&mut body.blocks, "scope.normal_finally");
    let catch_finally = cfg_block(&mut body.blocks, "scope.catch_finally");
    body.blocks[try_body].terminator = mir::Terminator::Goto(normal_finally);
    set_cfg_block(
        &mut body.blocks,
        normal_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Goto(end),
        None,
    );
    assert!(matches!(
        body.blocks[catch]
            .statements
            .pop()
            .map(|statement| statement.kind),
        Some(mir::StatementKind::Eh(mir::EhStatement::EndCatch))
    ));
    body.blocks[catch].terminator = mir::Terminator::Goto(catch_finally);
    set_cfg_block(
        &mut body.blocks,
        catch_finally,
        vec![
            stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            call_stmt(user_call(cleanup)),
        ],
        mir::Terminator::Goto(end),
        None,
    );
    body.blocks[next].statements = vec![call_stmt(user_call(cleanup))];
    body.blocks[next].unwind = Some(exit_pad);
    body.blocks[handler_cleanup]
        .statements
        .push(call_stmt(user_call(cleanup)));
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));
    let dump = lir::dump(&module);

    // The finally body is inlined on normal completion, after the
    // catch body, on a catch-body exceptional exit, and before the
    // no-match rethrow.
    assert_eq!(dump.matches("local-fn2()").count(), 4);
    // The last copy is on the rethrow path, before the rethrow.
    let rethrow = dump
        .find("runtime @scoop_rt_rethrow()")
        .expect("a rethrow path");
    let last_cleanup = dump
        .rfind("local-fn2()")
        .expect("the rethrow path runs the finally");
    assert!(last_cleanup < rethrow);
    assert!(dump.contains("landingpad"));
    // A caught normal exit ends directly; catch-body exceptions
    // and no-match/rethrow exits have distinct cleanup pads.
    // Exactly one executes on each path.
    assert_eq!(dump.matches("end_catch").count(), 3);
}

#[test]
fn return_inside_try_runs_finally_before_returning() {
    // fun f(): Int { try { return 1 } finally { cleanup() } }
    let (mut b, _, _, cleanup) = try_shell(true);
    let mut locals = Arena::new();
    let result = locals.alloc(local("$return.1", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let unwind = cfg_block(&mut blocks, "try.unwind.1");
    let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
    let exit_pad = cfg_block(&mut blocks, "try.exit_pad.3");
    let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.4");
    let end = cfg_block(&mut blocks, "try.end.5");
    let try_body = cfg_block(&mut blocks, "try.body.6");
    let return_finally = cfg_block(&mut blocks, "scope.7");
    let rethrow_finally = cfg_block(&mut blocks, "scope.8");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(try_body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Goto(rethrow_finally),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        end,
        Vec::new(),
        mir::Terminator::Unreachable,
        None,
    );
    set_cfg_block(
        &mut blocks,
        try_body,
        vec![val_decl(result, mir::Expr::int(1))],
        mir::Terminator::Goto(return_finally),
        Some(unwind),
    );
    set_cfg_block(
        &mut blocks,
        return_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Return {
            value: Some(local_expr(result, mir::Type::Int)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rethrow_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Rethrow {
            unwind: Some(exit_pad),
        },
        Some(exit_pad),
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        Vec::new(),
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // The finally copy runs before the return on the `return`
    // path and before the rethrow on the unwind path; the merge
    // block is dead (both paths leave the function).
    let expected = "\
Module
  fun @scoop.helper() -> void
  block entry
    ret
  fun @scoop.handled() -> void
  block entry
    ret
  fun @scoop.cleanup() -> void
  block entry
    ret
  fun @scoop.f() -> i64
    local %0 $return.1: i64
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    br @try.body.6
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    br @scope.8
  block try.exit_pad.3
    (t3, t4) = cleanup_pad : (exception_record, ptr<raw>)
    store t3 -> local1
    store t4 -> local2
    br @try.exit_cleanup.4
  block try.exit_cleanup.4
    end_catch
    resume local1
  block try.end.5
    unreachable
  block try.body.6
    store 1 -> local0
    br @scope.7
  block scope.7
    call void-target0 sig=void0 () effect=managed-safepoint local-fn2()
    t5 = aggregate () : {}
    ret local0
  block scope.8
    invoke void-target1 sig=void1 () effect=managed-safepoint local-fn2() normal @invoke.normal.1 unwind @try.exit_pad.3
    br @invoke.normal.1
  block invoke.normal.1
    t6 = aggregate () : {}
    invoke void-target2 sig=void2 () effect=no-gc runtime @scoop_rt_rethrow() normal @rethrow.normal.2 unwind @try.exit_pad.3
    br @rethrow.normal.2
  block rethrow.normal.2
    unreachable
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn throw_outside_try_is_a_throw_instruction() {
    // fun fail(): Unit { throw makeError() } — no try, so the
    // `Throw` instruction ends the block.
    let mut b = Builder::new();
    let my_error = my_error(&mut b);
    let mut ctor_locals = Arena::new();
    let make = b.user_fn_body(
        "makeError",
        "scoop.makeError",
        Vec::new(),
        mir::Type::Class(my_error),
        returning_body(
            std::mem::take(&mut ctor_locals),
            expr(
                mir::Type::Class(my_error),
                mir::ExprKind::ClassInit {
                    class_id: my_error,
                    args: Vec::new(),
                },
            ),
        ),
    );
    let mut main_locals = Arena::new();
    let exception = main_locals.alloc(local("$call.1", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        body_with_terminator(
            main_locals,
            vec![call_value(
                exception,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(make),
                    },
                    args: Vec::new(),
                },
            )],
            mir::Terminator::Throw {
                exception: local_expr(exception, mir::Type::Class(my_error)),
                unwind: None,
            },
        ),
    );
    let module = lower(&b.finish(main));

    // Outside a try the throw is the `Throw` instruction ending
    // the block; the callee stays a plain call.
    let expected = "\
Module
  fun @scoop.makeError() -> ptr<managed>
  block entry
    call t0 = direct-target0 sig=direct0 (ptr<metadata>, i64) -> ptr<managed> effect=managed-safepoint runtime @scoop_rt_alloc(td0, 16)
    ret t0
  fun @scoop_main() -> void
    local %0 $call.1: ptr<managed>
  block entry
    call t0 = direct-target0 sig=direct0 () -> ptr<managed> effect=managed-safepoint local-fn0()
    store t0 -> local0
    throw local0
    unreachable
  td td0 MyError @scoop_td_MyError type-id=2 size=16 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
";
    assert_eq!(lir::dump(&module), expected);
}

#[test]
fn throw_inside_try_invokes_to_the_own_landingpad() {
    // try { throw e } catch (e: MyError) {} — the throw must
    // reach this function's own pad, so it is an invoke of the
    // runtime throw entry, not a plain `Throw`.
    let mut b = Builder::new();
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let body = single_catch_body(
        locals,
        e,
        mir::Type::Class(my_error),
        Vec::new(),
        None,
        Vec::new(),
    );
    let try_body = body
        .blocks
        .iter()
        .find_map(|(id, block)| (block.name == "try.body.8").then_some(id))
        .expect("try body block");
    let unwind = body.blocks[try_body].unwind;
    let mut body = body;
    body.blocks[try_body].terminator = mir::Terminator::Throw {
        exception: local_expr(e, mir::Type::Class(my_error)),
        unwind,
    };
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = module
        .functions
        .iter()
        .find(|f| f.symbol == mir::ENTRY_SYMBOL)
        .expect("the entry function");
    // The explicit MIR entry jumps into the try body. That block
    // ends with the invoke; its unwind target starts with the
    // landingpad.
    let entry = function
        .blocks
        .iter()
        .map(|(_, block)| block)
        .find(|block| block.name == "try.body.8")
        .expect("the try body");
    let lir::Instruction::Invoke {
        site,
        normal,
        unwind,
        ..
    } = entry.instructions.last().expect("the throw invoke")
    else {
        panic!("a throw inside a try must be invoked")
    };
    assert_eq!(call_symbol(&module, function, site), "scoop_rt_throw");
    assert!(matches!(
        function.blocks[*unwind].instructions.first(),
        Some(lir::Instruction::LandingPad { .. })
    ));
    assert!(matches!(
        function.blocks[*normal].terminator,
        lir::Terminator::Unreachable
    ));
    // The invoke block's terminator is the redundant `Br` to the
    // normal target (the codegen convention).
    assert!(matches!(
        entry.terminator,
        lir::Terminator::Br(target) if target == *normal
    ));
}

#[test]
fn nested_trys_unwind_to_their_own_pads() {
    // try { try { a() } catch (e1: E1) { b() } } catch (e2: E2) { c() }
    // — `a` unwinds to the inner pad; `b` (in the inner catch)
    // unwinds through the inner cleanup before the outer pad.
    let mut b = Builder::new();
    let e1 = b.class("E1", None, &[], vec![], vec![]);
    let e2 = b.class("E2", None, &[], vec![], vec![]);
    let a = b.user_fn("a", "scoop.a", Arena::new(), vec![]);
    let bb = b.user_fn("b", "scoop.b", Arena::new(), vec![]);
    let c = b.user_fn("c", "scoop.c", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let e1_local = locals.alloc(local("e1", mir::Type::Class(e1)));
    let e2_local = locals.alloc(local("e2", mir::Type::Class(e2)));
    let mut body = single_catch_body(
        locals,
        e2_local,
        mir::Type::Class(e2),
        Vec::new(),
        None,
        vec![call_stmt(user_call(c))],
    );
    let outer_body = cfg_block_named(&body, "try.body.8");
    let outer_unwind = cfg_block_named(&body, "try.unwind.1");
    let outer_dispatch = cfg_block_named(&body, "try.dispatch.2");
    let outer_end = cfg_block_named(&body, "try.end.7");
    let inner_unwind = cfg_block(&mut body.blocks, "try.unwind.inner");
    let inner_dispatch = cfg_block(&mut body.blocks, "try.dispatch.inner");
    let inner_handler_pad = cfg_block(&mut body.blocks, "try.handler_pad.inner");
    let inner_handler_cleanup = cfg_block(&mut body.blocks, "try.handler_cleanup.inner");
    let inner_exit_pad = cfg_block(&mut body.blocks, "try.exit_pad.inner");
    let inner_exit_cleanup = cfg_block(&mut body.blocks, "try.exit_cleanup.inner");
    let inner_end = cfg_block(&mut body.blocks, "try.end.inner");
    let inner_body = cfg_block(&mut body.blocks, "try.body.inner");
    let inner_catch = cfg_block(&mut body.blocks, "try.catch.inner");
    let inner_next = cfg_block(&mut body.blocks, "try.next.inner");
    body.blocks[outer_body].terminator = mir::Terminator::Goto(inner_body);
    set_cfg_block(
        &mut body.blocks,
        inner_unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_dispatch),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Branch {
            cond: expr(
                mir::Type::Boolean,
                mir::ExprKind::IsInstance {
                    operand: Box::new(mir::Expr::caught_exception()),
                    check_ty: Box::new(mir::Type::Class(e1)),
                },
            ),
            then_block: inner_catch,
            else_block: inner_next,
        },
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_handler_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_handler_cleanup),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_handler_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Goto(outer_dispatch),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Goto(outer_dispatch),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_end,
        Vec::new(),
        mir::Terminator::Goto(outer_end),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_body,
        vec![call_stmt(user_call(a))],
        mir::Terminator::Goto(inner_end),
        Some(inner_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_catch,
        vec![
            val_decl(
                e1_local,
                expr(
                    mir::Type::Class(e1),
                    mir::ExprKind::Retype {
                        operand: Box::new(mir::Expr::caught_exception()),
                        ty: Box::new(mir::Type::Class(e1)),
                    },
                ),
            ),
            call_stmt(user_call(bb)),
            stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        mir::Terminator::Goto(inner_end),
        Some(inner_handler_pad),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_next,
        Vec::new(),
        mir::Terminator::Rethrow {
            unwind: Some(inner_exit_pad),
        },
        None,
    );
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = module
        .functions
        .iter()
        .find(|f| f.symbol == mir::ENTRY_SYMBOL)
        .expect("the entry function");
    // Two primary catch pads, one per try. Inner handler cleanup
    // pads also carry catch-all clauses so they can forward to the
    // outer dispatch; identify primaries by their block role.
    let pads: Vec<&str> = function
        .blocks
        .iter()
        .filter(|(_, block)| block.name.contains("try.unwind"))
        .map(|(_, block)| block.name.as_str())
        .collect();
    assert_eq!(pads.len(), 2);
    let cleanup_pad_count = function
        .blocks
        .iter()
        .filter(|(_, block)| {
            matches!(
                block.instructions.first(),
                Some(lir::Instruction::CleanupPad { .. })
            )
        })
        .count();
    assert_eq!(cleanup_pad_count, 2);
    let handler_pads: Vec<&str> = function
        .blocks
        .iter()
        .filter(|(_, block)| block.name.contains("handler_pad"))
        .map(|(_, block)| block.name.as_str())
        .collect();
    assert_eq!(handler_pads.len(), 2);
    // `a` unwinds to the inner catch pad. `b` runs inside that
    // handler and therefore unwinds through the inner cleanup;
    // `c` does the same through the outer cleanup.
    let mut invokes = Vec::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            if let lir::Instruction::Invoke { site, unwind, .. } = instruction {
                invokes.push((
                    call_symbol(&module, function, site),
                    function.blocks[*unwind].name.as_str(),
                ));
            }
        }
    }
    let unwind_of = |symbol: &str| {
        invokes
            .iter()
            .find(|(s, _)| *s == symbol)
            .map(|(_, u)| *u)
            .unwrap_or_else(|| panic!("{symbol} must be invoked"))
    };
    assert_eq!(unwind_of("scoop.a"), pads[1]);
    assert_eq!(unwind_of("scoop.b"), handler_pads[1]);
    assert_eq!(unwind_of("scoop.c"), handler_pads[0]);
}
