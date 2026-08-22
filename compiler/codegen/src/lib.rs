//! Codegen stage: mechanically translate LIR to LLVM IR, emit
//! TypeDescriptors, expand codegen-stage intrinsics, produce `.o`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.5 and
//! `docs/milestone2/DESIGN.md` section 2.5.
//!
//! All locals become `alloca`s at the top of the entry block; SSA
//! construction is left to LLVM's mem2reg. Temps are SSA values kept in a
//! map. Function signatures come from LIR (`params` / `return_ty`);
//! runtime functions are declared at their call sites with the signature
//! implied by the operands.

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
            GlobalInit::CString(value) => {
                // [N+1 x i8] c"...\00" (e.g. trap messages); private,
                // only referenced from within the module.
                let bytes = value.as_bytes();
                let ty = i8_ty.array_type(bytes.len() as u32 + 1);
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_constant(true);
                llvm_global.set_linkage(inkwell::module::Linkage::Private);
                llvm_global.set_initializer(&context.const_string(bytes, true));
                globals.push(llvm_global);
            }
        }
    }

    // Two passes: declare every function first so call sites never
    // create shadow extern declarations (a forward call would
    // otherwise declare the symbol as extern, and the later definition
    // would be renamed with a `.N` suffix by LLVM, breaking the link).
    for function in &module.functions {
        declare_function(&context, &llvm, function)?;
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

/// The (opaque) pointer type shared by all `LirType::Ptr` values.
fn ptr_ty(context: &Context) -> inkwell::types::PointerType<'_> {
    context.ptr_type(AddressSpace::default())
}

/// Per-function emission state: everything instruction translation
/// needs, bundled to keep signatures small.
struct FnEmitter<'a, 'ctx> {
    context: &'ctx Context,
    llvm: &'a LlvmModule<'ctx>,
    builder: &'a inkwell::builder::Builder<'ctx>,
    function: &'a Function,
    llvm_function: inkwell::values::FunctionValue<'ctx>,
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
            Value::Param(index) => self
                .llvm_function
                .get_nth_param(index)
                .ok_or_else(|| CodegenError(format!("param {index} out of range")))?,
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
                let fn_ty = if symbol == scoop_lir::TRAP_SYMBOL {
                    // Fixed runtime contract: void scoop_rt_trap(ptr).
                    context
                        .void_type()
                        .fn_type(&[ptr_ty(context).into()], false)
                } else {
                    match out {
                        Some(temp) => {
                            basic_ty(context, &function.temps[*temp].ty)?.fn_type(&param_tys, false)
                        }
                        None => context.void_type().fn_type(&param_tys, false),
                    }
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
            Instruction::IsSome { out, operand } => {
                // Representation fixed by the operand's LIR type (LIR
                // meta, spec 7.4): niche pointer or `{ i1, T }` tag.
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: IntValue = match operand_ty {
                    LirType::Ptr => builder
                        .build_int_compare(
                            IntPredicate::NE,
                            operand.into_pointer_value(),
                            ptr_ty(context).const_null(),
                            &name,
                        )
                        .map_err(|e| {
                            CodegenError(format!(
                                "is_some @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?,
                    _ => builder
                        .build_extract_value(operand.into_struct_value(), 0, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "is_some @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                        .into_int_value(),
                };
                self.temps.insert(*out, result.into());
            }
            Instruction::Unwrap { out, operand } => {
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                let operand = self.value(*operand)?;
                let result = match operand_ty {
                    // Niche pointer: the payload is the pointer itself.
                    LirType::Ptr => operand,
                    _ => {
                        let name = format!("t{}", out.into_raw().into_u32());
                        builder
                            .build_extract_value(operand.into_struct_value(), 1, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "unwrap @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::SomeWrap { out, value } => {
                let out_ty = &function.temps[*out].ty;
                let value = self.value(*value)?;
                let result = match out_ty {
                    // Niche pointer: `Some(p)` is the pointer itself.
                    LirType::Ptr => value,
                    _ => {
                        let ty = basic_ty(context, out_ty)?.into_struct_type();
                        let name = format!("t{}", out.into_raw().into_u32());
                        let mut wrapped = ty.get_undef();
                        for (index, element) in [
                            (0, context.bool_type().const_int(1, false).into()),
                            (1, value),
                        ] {
                            wrapped = builder
                                .build_insert_value(wrapped, element, index, &name)
                                .map_err(|e| {
                                    CodegenError(format!(
                                        "some_wrap @{symbol}: {e}",
                                        symbol = function.symbol
                                    ))
                                })?
                                .into_struct_value();
                        }
                        wrapped.into()
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::NoneConst { out } => {
                let out_ty = &function.temps[*out].ty;
                let result: BasicValueEnum = match out_ty {
                    // Niche pointer: `None` is the null pointer.
                    LirType::Ptr => ptr_ty(context).const_null().into(),
                    _ => {
                        // `{ false, undef }`: only the tag is meaningful.
                        let ty = basic_ty(context, out_ty)?.into_struct_type();
                        let name = format!("t{}", out.into_raw().into_u32());
                        builder
                            .build_insert_value(
                                ty.get_undef(),
                                context.bool_type().const_int(0, false),
                                0,
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "none @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                            .into_struct_value()
                            .into()
                    }
                };
                self.temps.insert(*out, result);
            }
        }
        Ok(())
    }
}

/// Translate one LIR function. Signature (parameters and return type)
/// comes from LIR; parameters are SSA values (`Value::Param`).
fn fn_type_of<'ctx>(
    context: &'ctx Context,
    function: &Function,
) -> Result<inkwell::types::FunctionType<'ctx>, CodegenError> {
    let param_tys: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|ty| basic_ty(context, ty).map(Into::into))
        .collect::<Result<_, _>>()?;
    Ok(match &function.return_ty {
        LirType::Void => context.void_type().fn_type(&param_tys, false),
        return_ty => basic_ty(context, return_ty)?.fn_type(&param_tys, false),
    })
}

/// Declare a function with its final symbol and signature.
fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    function: &Function,
) -> Result<(), CodegenError> {
    let fn_ty = fn_type_of(context, function)?;
    llvm.add_function(&function.symbol, fn_ty, None);
    Ok(())
}

fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    globals_arena: &Arena<Global>,
    globals: &[GlobalValue<'ctx>],
    function: &Function,
) -> Result<(), CodegenError> {
    // Pre-declared in the first pass (see `emit_object`).
    let llvm_function = llvm
        .get_function(&function.symbol)
        .expect("function declared in the first pass");

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
        llvm_function,
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
            Terminator::Return { value } => {
                let value = value
                    .map(|value| emitter.value(value))
                    .transpose()
                    .map_err(|e: CodegenError| {
                        CodegenError(format!("ret @{}: {}", function.symbol, e.0))
                    })?;
                builder
                    .build_return(
                        value
                            .as_ref()
                            .map(|v| v as &dyn inkwell::values::BasicValue),
                    )
                    .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
            }
            Terminator::Unreachable => {
                builder
                    .build_unreachable()
                    .map_err(|e| CodegenError(format!("unreachable @{}: {e}", function.symbol)))?;
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
                terminator: Terminator::Return { value: None },
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
            terminator: Terminator::Return { value: None },
        };

        Module {
            globals,
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                params: vec![],
                return_ty: LirType::Void,
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

    /// An M3-shaped module: functions with parameters and return values,
    /// both Option representations (niche pointer for `Option<String>`,
    /// `{ i1, T }` tag for `Option<Int>`) exercised through IsSome /
    /// Unwrap / SomeWrap / NoneConst, a CString global, a trap call and
    /// an unreachable terminator.
    fn option_module() -> Module {
        let mut globals = Arena::default();
        let trap_message = globals.alloc(Global {
            symbol: "scoop.trap.0".to_string(),
            init: GlobalInit::CString("unwrap on None".to_string()),
        });

        // fun @scoop.identity$I(x: i64) -> i64 = x
        let mut identity_blocks = Arena::default();
        let identity_entry = identity_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![],
            terminator: Terminator::Return {
                value: Some(Value::Param(0)),
            },
        });
        let identity = Function {
            symbol: "scoop.identity$I".to_string(),
            params: vec![LirType::I64],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: identity_blocks,
            entry: identity_entry,
        };

        // fun @scoop.option_ptr(o: ptr) -> i1: all four Option
        // instructions on the niche-pointer representation.
        let mut ptr_temps = Arena::default();
        let ptr_t0 = ptr_temps.alloc(Temp { ty: LirType::I1 }); // is_some o
        let ptr_t1 = ptr_temps.alloc(Temp { ty: LirType::Ptr }); // unwrap o
        let ptr_t2 = ptr_temps.alloc(Temp { ty: LirType::Ptr }); // some_wrap t1
        let ptr_t3 = ptr_temps.alloc(Temp { ty: LirType::Ptr }); // none
        let ptr_t4 = ptr_temps.alloc(Temp { ty: LirType::I1 }); // is_some t3
        let mut ptr_blocks = Arena::default();
        let ptr_entry = ptr_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::IsSome {
                    out: ptr_t0,
                    operand: Value::Param(0),
                },
                Instruction::Unwrap {
                    out: ptr_t1,
                    operand: Value::Param(0),
                },
                Instruction::SomeWrap {
                    out: ptr_t2,
                    value: Value::Temp(ptr_t1),
                },
                Instruction::NoneConst { out: ptr_t3 },
                Instruction::IsSome {
                    out: ptr_t4,
                    operand: Value::Temp(ptr_t3),
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(ptr_t4)),
            },
        });
        let option_ptr = Function {
            symbol: "scoop.option_ptr".to_string(),
            params: vec![LirType::Ptr],
            return_ty: LirType::I1,
            locals: Arena::default(),
            temps: ptr_temps,
            blocks: ptr_blocks,
            entry: ptr_entry,
        };

        // fun @scoop.option_tag(o: { i1, i64 }) -> { i1, i64 }: all four
        // Option instructions on the tagged representation.
        let tag_ty = LirType::Aggregate(vec![LirType::I1, LirType::I64]);
        let mut tag_temps = Arena::default();
        let tag_t0 = tag_temps.alloc(Temp { ty: LirType::I1 }); // is_some o
        let tag_t1 = tag_temps.alloc(Temp { ty: LirType::I64 }); // unwrap o
        let tag_t2 = tag_temps.alloc(Temp { ty: tag_ty.clone() }); // some_wrap t1
        let tag_t3 = tag_temps.alloc(Temp { ty: tag_ty.clone() }); // none
        let mut tag_blocks = Arena::default();
        let tag_entry = tag_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::IsSome {
                    out: tag_t0,
                    operand: Value::Param(0),
                },
                Instruction::Unwrap {
                    out: tag_t1,
                    operand: Value::Param(0),
                },
                Instruction::SomeWrap {
                    out: tag_t2,
                    value: Value::Temp(tag_t1),
                },
                Instruction::NoneConst { out: tag_t3 },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(tag_t2)),
            },
        });
        let option_tag = Function {
            symbol: "scoop.option_tag".to_string(),
            params: vec![tag_ty.clone()],
            return_ty: tag_ty,
            locals: Arena::default(),
            temps: tag_temps,
            blocks: tag_blocks,
            entry: tag_entry,
        };

        // fun @scoop.trap_on_none(): the `!!`-on-None path — trap call
        // (noreturn) followed by unreachable.
        let mut trap_blocks = Arena::default();
        let trap_entry = trap_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::Call {
                out: None,
                symbol: scoop_lir::TRAP_SYMBOL.to_string(),
                args: vec![Value::Global(trap_message)],
            }],
            terminator: Terminator::Unreachable,
        });
        let trap_on_none = Function {
            symbol: "scoop.trap_on_none".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: trap_blocks,
            entry: trap_entry,
        };

        Module {
            globals,
            functions: vec![identity, option_ptr, option_tag, trap_on_none],
            entry_symbol: "scoop.identity$I".to_string(),
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
    fn emits_m3_features() {
        let module = option_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m3_test_{}.o", std::process::id()));
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }
}
