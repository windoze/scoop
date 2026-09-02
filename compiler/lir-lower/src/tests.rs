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
        let methods = methods
            .iter()
            .map(|method| {
                self.functions.alloc(mir::Function {
                    gc_effect: mir::GcEffect::Managed,
                    name: format!("{name}.{method}"),
                    symbol: format!("test.{name}.{method}"),
                    params: Vec::new(),
                    return_ty: mir::Type::Unit,
                    body: mir::Body::unreachable(Arena::new()),
                })
            })
            .collect();
        self.interfaces.alloc(mir::InterfaceDef {
            name: name.to_string(),
            methods,
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

fn call_symbol(module: &lir::Module, destination: lir::CallDestination) -> &str {
    match destination {
        lir::CallDestination::Local(id) => &module.functions[id.into_u32() as usize].symbol,
        lir::CallDestination::Runtime(runtime) => runtime.symbol(),
        lir::CallDestination::Extern(id) => &module.extern_functions[id].native_symbol,
        lir::CallDestination::Dispatch { .. } => panic!("dispatch calls have no symbol"),
    }
}

fn instructions_without_polls(block: &lir::BasicBlock) -> Vec<&lir::Instruction> {
    block
        .instructions
        .iter()
        .filter(|instruction| !matches!(instruction, lir::Instruction::ManagedPoll { .. }))
        .collect()
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

fn empty_vtable() -> Vec<mir::TableSlot> {
    Vec::new()
}

mod arrays;
mod basics;
mod enums;
mod exceptions;
mod functions;
mod objects;
mod traps;
