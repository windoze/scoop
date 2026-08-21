//! Codegen stage: mechanically translate LIR to LLVM IR, emit
//! TypeDescriptors, expand codegen-stage intrinsics, produce `.o`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.5 and
//! `docs/milestone2/DESIGN.md` section 2.5.
//!
//! All locals become `alloca`s at the top of the entry block; SSA
//! construction is left to LLVM's mem2reg. Temps are SSA values kept in a
//! map. User functions are `void()`; runtime functions are declared at
//! their call sites with the signature implied by the operands.

use std::collections::HashMap;
use std::path::Path;

use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, GlobalValue, IntValue, PointerValue, ValueKind};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};
use la_arena::{Arena, Idx};
use scoop_lir::{
    BinOp, Function, Global, GlobalInit, Instruction, LirType, Module, TempId, Terminator, UnOp,
    Value,
};

/// Error produced while translating LIR or emitting the object file.
#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

/// Translate `module` to LLVM IR and emit an object file at `output`
/// using the host target.
pub fn emit_object(module: &Module, output: &Path) -> Result<(), CodegenError> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError(format!("failed to initialize native target: {e}")))?;

    let string_layout = module
        .meta
        .layouts
        .iter()
        .find(|layout| layout.name == "String")
        .ok_or_else(|| CodegenError("LIR meta lacks a layout for String".to_string()))?;

    let context = Context::create();
    let llvm = context.create_module("scoop");
    let builder = context.create_builder();

    let ptr_ty = context.ptr_type(AddressSpace::default());
    let i8_ty = context.i8_type();
    let i64_ty = context.i64_type();

    // @scoop_td_String: { i64 type_id, i64 size, i64 align, ptr ref_offsets }
    // (runtime spec 2.2; type_id 1 is the M1-only type, ref_offsets is null
    // until the GC lands).
    let td_ty = context.struct_type(
        &[i64_ty.into(), i64_ty.into(), i64_ty.into(), ptr_ty.into()],
        false,
    );
    let string_td = llvm.add_global(td_ty, None, scoop_lir::STRING_TD_SYMBOL);
    string_td.set_constant(true);
    string_td.set_initializer(&context.const_struct(
        &[
            i64_ty.const_int(1, false).into(),
            i64_ty.const_int(string_layout.size, false).into(),
            i64_ty.const_int(string_layout.align, false).into(),
            ptr_ty.const_null().into(),
        ],
        false,
    ));

    // Globals. Indexed by GlobalId (arena iteration is in index order).
    let mut globals: Vec<GlobalValue> = Vec::with_capacity(module.globals.len());
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                // { ptr td, i64 len, [N x i8] data } (runtime spec 2.4).
                let bytes = value.as_bytes();
                let ty = context.struct_type(
                    &[
                        ptr_ty.into(),
                        i64_ty.into(),
                        i8_ty.array_type(bytes.len() as u32).into(),
                    ],
                    false,
                );
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_constant(true);
                llvm_global.set_initializer(&context.const_struct(
                    &[
                        string_td.as_pointer_value().into(),
                        i64_ty.const_int(bytes.len() as u64, false).into(),
                        context.const_string(bytes, false).into(),
                    ],
                    false,
                ));
                globals.push(llvm_global);
            }
        }
    }

    for function in &module.functions {
        emit_function(
            &context,
            &llvm,
            &builder,
            &module.globals,
            &globals,
            function,
        )?;
    }

    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple)
        .map_err(|e| CodegenError(format!("no target for host triple: {e}")))?;
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| CodegenError("failed to create host target machine".to_string()))?;
    machine
        .write_to_file(&llvm, FileType::Object, output)
        .map_err(|e| CodegenError(format!("failed to write {}: {e}", output.display())))?;
    Ok(())
}

/// The LLVM type of a (non-void) LIR type: aggregates are literal
/// structs per the layout in LIR meta; Unit is the empty struct `{}`.
fn basic_ty<'ctx>(
    context: &'ctx Context,
    ty: &LirType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        LirType::Void => {
            return Err(CodegenError(
                "void is not a value type (locals, temps, call args)".to_string(),
            ));
        }
        LirType::I1 => context.bool_type().into(),
        LirType::I64 => context.i64_type().into(),
        LirType::Ptr => context.ptr_type(AddressSpace::default()).into(),
        LirType::Aggregate(elements) => {
            let fields: Vec<BasicTypeEnum> = elements
                .iter()
                .map(|element| basic_ty(context, element))
                .collect::<Result<_, _>>()?;
            context.struct_type(&fields, false).into()
        }
    })
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

/// Per-function emission state: everything instruction translation
/// needs, bundled to keep signatures small.
struct FnEmitter<'a, 'ctx> {
    context: &'ctx Context,
    llvm: &'a LlvmModule<'ctx>,
    builder: &'a inkwell::builder::Builder<'ctx>,
    function: &'a Function,
    globals_arena: &'a Arena<Global>,
    globals: &'a [GlobalValue<'ctx>],
    allocas: Vec<PointerValue<'ctx>>,
    temps: HashMap<TempId, BasicValueEnum<'ctx>>,
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Materialize an operand as an LLVM value: locals are loaded from
    /// their stack slot, temps are SSA values, globals are addressed by
    /// pointer.
    fn value(&self, value: Value) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let context = self.context;
        let function = self.function;
        Ok(match value {
            Value::Local(id) => {
                let ty = basic_ty(context, &function.locals[id].ty)?;
                self.builder
                    .build_load(ty, self.allocas[arena_index(id)], &function.locals[id].name)
                    .map_err(|e| CodegenError(format!("load %{}: {e}", function.locals[id].name)))?
            }
            Value::Temp(id) => *self.temps.get(&id).ok_or_else(|| {
                CodegenError(format!(
                    "temp t{} used before definition",
                    id.into_raw().into_u32()
                ))
            })?,
            Value::IntConst(value) => context.i64_type().const_int(value as u64, true).into(),
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::Global(id) => self.globals[arena_index(id)].as_pointer_value().into(),
        })
    }

    fn instruction(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::BinOp { out, op, lhs, rhs } => {
                let lhs = self.value(*lhs)?.into_int_value();
                let rhs = self.value(*rhs)?.into_int_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let result: IntValue = match op {
                    BinOp::Add => builder.build_int_add(lhs, rhs, &name),
                    BinOp::Sub => builder.build_int_sub(lhs, rhs, &name),
                    BinOp::Mul => builder.build_int_mul(lhs, rhs, &name),
                    BinOp::SDiv => builder.build_int_signed_div(lhs, rhs, &name),
                    // Signed comparisons; icmp works for both i64 and i1 (== / !=).
                    BinOp::Lt => builder.build_int_compare(IntPredicate::SLT, lhs, rhs, &name),
                    BinOp::Le => builder.build_int_compare(IntPredicate::SLE, lhs, rhs, &name),
                    BinOp::Gt => builder.build_int_compare(IntPredicate::SGT, lhs, rhs, &name),
                    BinOp::Ge => builder.build_int_compare(IntPredicate::SGE, lhs, rhs, &name),
                    BinOp::Eq => builder.build_int_compare(IntPredicate::EQ, lhs, rhs, &name),
                    BinOp::Ne => builder.build_int_compare(IntPredicate::NE, lhs, rhs, &name),
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::UnaryOp { out, op, operand } => {
                let operand = self.value(*operand)?.into_int_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let result = match op {
                    // Neg is `0 - x` (no dedicated LLVM neg instruction).
                    UnOp::Neg => {
                        builder.build_int_sub(operand.get_type().const_zero(), operand, &name)
                    }
                    // Not is `xor x, true`: flips the i1 bit directly.
                    UnOp::Not => {
                        builder.build_xor(operand, operand.get_type().const_all_ones(), &name)
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::MakeAggregate { out, elements } => {
                let ty = basic_ty(context, &function.temps[*out].ty)?.into_struct_type();
                let name = format!("t{}", out.into_raw().into_u32());
                let mut aggregate = ty.get_undef();
                for (index, element) in elements.iter().enumerate() {
                    aggregate = builder
                        .build_insert_value(aggregate, self.value(*element)?, index as u32, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "insertvalue @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                        .into_struct_value();
                }
                self.temps.insert(*out, aggregate.into());
            }
            Instruction::ExtractValue {
                out,
                aggregate,
                index,
            } => {
                let aggregate = self.value(*aggregate)?.into_struct_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let element = builder
                    .build_extract_value(aggregate, *index, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "extractvalue @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::Store { local, value: v } => {
                let operand = self.value(*v)?;
                builder
                    .build_store(self.allocas[arena_index(*local)], operand)
                    .map_err(|e| {
                        CodegenError(format!("store %{}: {e}", function.locals[*local].name))
                    })?;
            }
            Instruction::Call { out, symbol, args } => {
                // Signature from the call site: parameter types from the
                // operands, return type from the result temp (void when
                // there is none). Undefined callees are declared extern.
                let param_tys: Vec<BasicMetadataTypeEnum> = args
                    .iter()
                    .map(|arg| {
                        basic_ty(context, &function.value_ty(self.globals_arena, *arg))
                            .map(Into::into)
                    })
                    .collect::<Result<_, _>>()?;
                let fn_ty = match out {
                    Some(temp) => {
                        basic_ty(context, &function.temps[*temp].ty)?.fn_type(&param_tys, false)
                    }
                    None => context.void_type().fn_type(&param_tys, false),
                };
                let callee = self
                    .llvm
                    .get_function(symbol)
                    .unwrap_or_else(|| self.llvm.add_function(symbol, fn_ty, None));
                let call_args: Vec<inkwell::values::BasicMetadataValueEnum> = args
                    .iter()
                    .map(|arg| self.value(*arg).map(Into::into))
                    .collect::<Result<_, _>>()?;
                let call = builder
                    .build_call(callee, &call_args, "call")
                    .map_err(|e| CodegenError(format!("call @{symbol}: {e}")))?;
                if let Some(temp) = out {
                    match call.try_as_basic_value() {
                        ValueKind::Basic(result) => {
                            self.temps.insert(*temp, result);
                        }
                        ValueKind::Instruction(_) => {
                            return Err(CodegenError(format!(
                                "call @{symbol} produced no value for t{}",
                                temp.into_raw().into_u32()
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Translate one LIR function. All M2 user functions are `void()`.
fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    globals_arena: &Arena<Global>,
    globals: &[GlobalValue<'ctx>],
    function: &Function,
) -> Result<(), CodegenError> {
    let void_ty = context.void_type();
    let llvm_function = llvm.add_function(&function.symbol, void_ty.fn_type(&[], false), None);

    // All blocks up front so terminators can reference them in any order.
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for (_, block) in function.blocks.iter() {
        blocks.push(context.append_basic_block(llvm_function, &block.name));
    }

    let mut emitter = FnEmitter {
        context,
        llvm,
        builder,
        function,
        globals_arena,
        globals,
        allocas: Vec::with_capacity(function.locals.len()),
        temps: HashMap::new(),
    };

    // All locals are stack slots allocated at the top of the entry block;
    // LLVM's mem2reg promotes them. Entry is empty at this point, so
    // positioning at its end places the allocas before every instruction.
    builder.position_at_end(blocks[arena_index(function.entry)]);
    for (_, local) in function.locals.iter() {
        let ty = basic_ty(context, &local.ty)?;
        emitter.allocas.push(
            builder
                .build_alloca(ty, &local.name)
                .map_err(|e| CodegenError(format!("alloca %{}: {e}", local.name)))?,
        );
    }

    for (block_id, block) in function.blocks.iter() {
        builder.position_at_end(blocks[arena_index(block_id)]);
        for instruction in &block.instructions {
            emitter.instruction(instruction)?;
        }
        match &block.terminator {
            Terminator::Br(target) => {
                builder
                    .build_unconditional_branch(blocks[arena_index(*target)])
                    .map_err(|e| CodegenError(format!("br @{}: {e}", block.name)))?;
            }
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
                let cond = emitter.value(*cond)?.into_int_value();
                builder
                    .build_conditional_branch(
                        cond,
                        blocks[arena_index(*then_block)],
                        blocks[arena_index(*else_block)],
                    )
                    .map_err(|e| CodegenError(format!("cbr @{}: {e}", block.name)))?;
            }
            Terminator::Return => {
                builder
                    .build_return(None)
                    .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_lir::{BasicBlock, Global, GlobalInit, Layout, LirMeta, Local, Temp};

    use super::*;

    /// An M2-shaped module: string constants, a user function exercising
    /// alloca/store/load, arithmetic, branches, aggregates and
    /// extractvalue, plus calls into the new runtime functions.
    fn values_module() -> Module {
        let mut globals = Arena::default();
        let hello = globals.alloc(Global {
            symbol: "scoop.string.0".to_string(),
            init: GlobalInit::StringConst("hello, ".to_string()),
        });
        let world = globals.alloc(Global {
            symbol: "scoop.string.1".to_string(),
            init: GlobalInit::StringConst("world".to_string()),
        });

        let mut locals = Arena::default();
        let n = locals.alloc(Local {
            name: "n".to_string(),
            ty: LirType::I64,
        });
        let point = locals.alloc(Local {
            name: "p".to_string(),
            ty: LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        });
        let unit = locals.alloc(Local {
            name: "u".to_string(),
            ty: LirType::Aggregate(vec![]),
        });

        let mut temps = Arena::default();
        let temp = |temps: &mut Arena<Temp>, ty: LirType| temps.alloc(Temp { ty });
        // t0 = 1 + 2
        let t0 = temp(&mut temps, LirType::I64);
        // t1 = {40, 2} (Point)
        let t1 = temp(
            &mut temps,
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        );
        // t2 = p.x
        let t2 = temp(&mut temps, LirType::I64);
        // t3 = t2 < 100
        let t3 = temp(&mut temps, LirType::I1);
        // t4 = -t2
        let t4 = temp(&mut temps, LirType::I64);
        // t5 = !true
        let t5 = temp(&mut temps, LirType::I1);
        // t6 = concat(hello, world)
        let t6 = temp(&mut temps, LirType::Ptr);
        // t7 = ()
        let t7 = temp(&mut temps, LirType::Aggregate(vec![]));

        // Allocate the four blocks first so terminators can reference
        // them, then fill in their bodies.
        let mut blocks = Arena::default();
        let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
            blocks.alloc(BasicBlock {
                name: name.to_string(),
                instructions: vec![],
                terminator: Terminator::Return,
            })
        };
        let entry = placeholder(&mut blocks, "entry");
        let then_block = placeholder(&mut blocks, "then");
        let else_block = placeholder(&mut blocks, "else");
        let end = placeholder(&mut blocks, "end");

        blocks[entry] = BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::BinOp {
                    out: t0,
                    op: BinOp::Add,
                    lhs: Value::IntConst(1),
                    rhs: Value::IntConst(2),
                },
                Instruction::Store {
                    local: n,
                    value: Value::Temp(t0),
                },
                Instruction::MakeAggregate {
                    out: t1,
                    elements: vec![Value::IntConst(40), Value::IntConst(2)],
                },
                Instruction::Store {
                    local: point,
                    value: Value::Temp(t1),
                },
                Instruction::ExtractValue {
                    out: t2,
                    aggregate: Value::Local(point),
                    index: 0,
                },
                Instruction::BinOp {
                    out: t3,
                    op: BinOp::Lt,
                    lhs: Value::Temp(t2),
                    rhs: Value::IntConst(100),
                },
                Instruction::Call {
                    out: Some(t6),
                    symbol: "scoop_rt_string_concat".to_string(),
                    args: vec![Value::Global(hello), Value::Global(world)],
                },
            ],
            terminator: Terminator::CondBr {
                cond: Value::Temp(t3),
                then_block,
                else_block,
            },
        };
        blocks[then_block] = BasicBlock {
            name: "then".to_string(),
            instructions: vec![
                Instruction::UnaryOp {
                    out: t4,
                    op: UnOp::Neg,
                    operand: Value::Temp(t2),
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Temp(t4)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println".to_string(),
                    args: vec![Value::Temp(t6)],
                },
            ],
            terminator: Terminator::Br(end),
        };
        blocks[else_block] = BasicBlock {
            name: "else".to_string(),
            instructions: vec![
                Instruction::UnaryOp {
                    out: t5,
                    op: UnOp::Not,
                    operand: Value::BoolConst(true),
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_boolean".to_string(),
                    args: vec![Value::Temp(t5)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Local(n)],
                },
            ],
            terminator: Terminator::Br(end),
        };
        blocks[end] = BasicBlock {
            name: "end".to_string(),
            instructions: vec![
                Instruction::MakeAggregate {
                    out: t7,
                    elements: vec![],
                },
                Instruction::Store {
                    local: unit,
                    value: Value::Temp(t7),
                },
            ],
            terminator: Terminator::Return,
        };

        Module {
            globals,
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                locals,
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 16,
                    align: 8,
                    ref_field_offsets: vec![],
                }],
            },
        }
    }

    #[test]
    fn emits_non_empty_object_file() {
        let module = values_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }
}
