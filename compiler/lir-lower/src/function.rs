use super::*;

mod call;
mod expression;

#[derive(Clone, Copy)]
enum LoweredCallDestination {
    Managed(lir::ManagedCallDestination),
    NoGc(lir::NoGcCallDestination),
}

impl LoweredCallDestination {
    fn managed_runtime(function: lir::ManagedRuntimeFunction) -> Self {
        Self::Managed(lir::ManagedCallDestination::runtime(function))
    }

    fn no_gc_runtime(function: lir::NoGcRuntimeFunction) -> Self {
        Self::NoGc(lir::NoGcCallDestination::runtime(function))
    }

    fn local(function: lir::LocalFunctionRef) -> Self {
        match function {
            lir::LocalFunctionRef::Managed(function) => {
                Self::Managed(lir::ManagedCallDestination::local(function))
            }
            lir::LocalFunctionRef::NoGc(function) => {
                Self::NoGc(lir::NoGcCallDestination::local(function))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum NativeCallDestination {
    Safe(lir::NativeSafeCallDestination),
    Borrowed(lir::NativeBorrowedCallDestination),
}

/// A fully typed call shape before its protocol-specific destination is bound.
/// This is lowering-local scratch state and can never enter LIR output.
enum PendingTypedCall {
    Void {
        signature: lir::VoidCallSignatureId,
        args: Vec<lir::Value>,
    },
    Direct {
        signature: lir::DirectCallSignatureId,
        out: lir::TempId,
        args: Vec<lir::Value>,
    },
    IndirectResult {
        signature: lir::IndirectResultCallSignatureId,
        storage: lir::LocalId,
        args: Vec<lir::Value>,
    },
}

impl PendingTypedCall {
    fn result_scan<'a>(&self, targets: &'a lir::CallTargets) -> &'a lir::RefScan {
        match self {
            Self::Void { .. } => &lir::RefScan::None,
            Self::Direct { signature, .. } => &targets.direct_signatures[*signature].result_scan,
            Self::IndirectResult { signature, .. } => {
                &targets.indirect_result_signatures[*signature].result.scan
            }
        }
    }
}

fn bind_typed_call<Destination: Copy>(
    targets: &mut lir::ProtocolCallTargets<Destination>,
    destination: Destination,
    call: PendingTypedCall,
) -> lir::TypedCall<Destination> {
    match call {
        PendingTypedCall::Void { signature, args } => {
            let target = targets.void.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Void { target, args }
        }
        PendingTypedCall::Direct {
            signature,
            out,
            args,
        } => {
            let target = targets.direct.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Direct { target, out, args }
        }
        PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        } => {
            let target = targets.indirect_result.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::IndirectResult {
                target,
                storage,
                args,
            }
        }
    }
}

fn runtime_call_destination(function: lir::RuntimeFunction) -> LoweredCallDestination {
    match function {
        lir::RuntimeFunction::Managed(function) => {
            LoweredCallDestination::managed_runtime(function)
        }
        lir::RuntimeFunction::NoGc(function) => LoweredCallDestination::no_gc_runtime(function),
    }
}

/// Map a primitive MIR binary operator onto its LIR opcode. Operand and result
/// types come exclusively from the typed MIR expressions.
fn binary_op(op: mir::BinOp) -> lir::BinOp {
    match op {
        mir::BinOp::IntAdd => lir::BinOp::Add,
        mir::BinOp::IntSub => lir::BinOp::Sub,
        mir::BinOp::IntMul => lir::BinOp::Mul,
        mir::BinOp::IntDiv => lir::BinOp::SDiv,
        mir::BinOp::IntLt => lir::BinOp::Lt,
        mir::BinOp::IntLe => lir::BinOp::Le,
        mir::BinOp::IntGt => lir::BinOp::Gt,
        mir::BinOp::IntGe => lir::BinOp::Ge,
        mir::BinOp::IntEq | mir::BinOp::BoolEq => lir::BinOp::Eq,
        mir::BinOp::IntNe | mir::BinOp::BoolNe => lir::BinOp::Ne,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_function<'a>(
    module: &'a mir::Module,
    function: &'a mir::Function,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &HashMap<mir::GlobalId, StorageGlobal>,
    globals: &mut Arena<lir::Global>,
    cstr_count: &mut usize,
    layout_types: &mut Vec<mir::Type>,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
    safepoint_ids: &'a mut safepoints::SafepointIds,
) -> lir::Function {
    // Parameters stay SSA values unless `addressOf` requires stable storage.
    // Address-taken parameters are copied once into a method-local slot.
    let address_taken = locals::address_taken(function);
    let mut local_map = HashMap::new();
    let params: Vec<lir::LirType> = function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            record_layout_types(&param.ty, layout_types);
            if !address_taken.contains(&param.local) {
                local_map.insert(param.local, LocalSlot::Param(index as u32));
            }
            lir_type(&param.ty)
        })
        .collect();

    // One LIR stack slot per non-parameter MIR local, in declaration
    // order.
    let mut locals = Arena::new();
    for (mir_id, local) in function.body.locals.iter() {
        if local_map.contains_key(&mir_id) {
            continue; // a parameter
        }
        record_layout_types(&local.ty, layout_types);
        let lir_id = locals.alloc(lir::Local {
            name: local.name.clone(),
            ty: lir_type(&local.ty),
        });
        local_map.insert(mir_id, LocalSlot::Slot(lir_id));
    }

    // Unit-returning functions are void at the LLVM level (DESIGN 2.4).
    record_layout_types(&function.return_ty, layout_types);
    let returns_void = function.return_ty == mir::Type::Unit;
    let return_ty = if returns_void {
        lir::LirType::Void
    } else {
        lir_type(&function.return_ty)
    };

    let mut blocks = Arena::new();
    let mut block_map = HashMap::new();
    for (mir_id, block) in function.body.blocks.iter() {
        let lir_id = blocks.alloc(lir::BasicBlock {
            name: block.name.clone(),
            instructions: Vec::new(),
            terminator: lir::Terminator::Unreachable,
        });
        block_map.insert(mir_id, lir_id);
    }
    let entry = block_map[&function.body.entry];
    let mut lowerer = FunctionLowerer {
        module,
        mir_locals: &function.body.locals,
        global_map,
        storage_globals,
        globals,
        cstr_count,
        layout_types,
        structs,
        enums,
        array_types,
        type_descriptors,
        local_function_map,
        extern_function_refs,
        safepoint_ids,
        local_map,
        locals,
        temps: Arena::new(),
        blocks,
        current: entry,
        block_map,
        current_unwind: None,
        block_count: 0,
        hidden_count: 0,
        returns_void,
        call_targets: lir::CallTargets::default(),
        trap_blocks: HashMap::new(),
        current_sealed: false,
        exception_slots: None,
        caught_exception: None,
    };
    for (index, param) in function.params.iter().enumerate() {
        if address_taken.contains(&param.local) {
            let local = lowerer.local_slot(param.local);
            lowerer.blocks[entry]
                .instructions
                .push(lir::Instruction::Store {
                    local,
                    value: lir::Value::Param(index as u32),
                });
        }
    }
    for (mir_id, block) in function.body.blocks.iter() {
        let lir_id = lowerer.block_map[&mir_id];
        lowerer.enter(lir_id);
        lowerer.current_unwind = block.unwind.map(|target| lowerer.block_map[&target]);
        lowerer.lower_statements(&block.statements);
        if !lowerer.current_sealed {
            lowerer.lower_terminator(&block.terminator);
        }
        assert!(lowerer.current_sealed, "every MIR block has a terminator");
    }
    lir::Function {
        gc_effect: match function.gc_effect {
            mir::GcEffect::Managed => lir::GcEffect::Managed,
            mir::GcEffect::NoGc => lir::GcEffect::NoGc,
        },
        symbol: function.symbol.clone(),
        params,
        return_ty,
        call_targets: lowerer.call_targets,
        locals: lowerer.locals,
        temps: lowerer.temps,
        blocks: lowerer.blocks,
        entry,
    }
}

// Safepoint placement and liveness live in safepoints.rs.

/// Where a MIR local lives in LIR: parameters are SSA values, all
/// other locals get stack slots.
#[derive(Clone, Copy)]
enum LocalSlot {
    Slot(lir::LocalId),
    Param(u32),
}

/// Per-function lowering state: locals, temps, and the basic blocks
/// built so far. Invariant: the `current` block is always unsealed
/// (its terminator is a placeholder); a block is sealed exactly when
/// control flow leaves it. `current_sealed` tracks whether the current
/// block was already sealed (by a `return`, or by a trap call), so
/// structured control flow does not seal it again with a branch.
struct FunctionLowerer<'a> {
    module: &'a mir::Module,
    /// Locals of the MIR function being lowered (for local storage and parameters).
    mir_locals: &'a Arena<mir::Local>,
    global_map: &'a HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &'a HashMap<mir::GlobalId, StorageGlobal>,
    /// Sink for ordinary globals such as trap-message C strings.
    globals: &'a mut Arena<lir::Global>,
    cstr_count: &'a mut usize,
    /// Sink for tuple types encountered in value types (meta layouts).
    layout_types: &'a mut Vec<mir::Type>,
    /// Complete value layouts used to classify return conventions and scans.
    structs: &'a Arena<lir::StructDef>,
    /// Enum definitions with fixed representations (enum value
    /// sizing, e.g. for `scoop_rt_box` payload sizes).
    enums: &'a Arena<lir::EnumDef>,
    /// Complete class-application to array-metadata mapping produced before
    /// any function is lowered.
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    /// Complete typed TypeDescriptor graph built before body lowering.
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
    safepoint_ids: &'a mut safepoints::SafepointIds,
    local_map: HashMap<mir::LocalId, LocalSlot>,
    locals: Arena<lir::Local>,
    temps: Arena<lir::Temp>,
    blocks: Arena<lir::BasicBlock>,
    current: lir::BlockId,
    block_map: HashMap<mir::BlockId, lir::BlockId>,
    current_unwind: Option<lir::BlockId>,
    /// Counters for unique block / hidden-local names.
    block_count: usize,
    hidden_count: usize,
    returns_void: bool,
    call_targets: lir::CallTargets,
    /// The shared trap blocks of this function, one per message,
    /// created on first use.
    trap_blocks: HashMap<String, lir::BlockId>,
    /// Whether the current block was already sealed.
    current_sealed: bool,
    /// Function-local spill slots shared by all landing pads. They
    /// avoid phi nodes when an inner catch cleanup forwards the same
    /// exception to an enclosing try's dispatch block.
    exception_slots: Option<(lir::LocalId, lir::LocalId)>,
    caught_exception: Option<lir::LocalId>,
}

impl<'a> FunctionLowerer<'a> {
    fn array_type_id(&self, class: mir::ClassId) -> lir::ArrayTypeId {
        self.array_types[&class]
    }

    fn new_block(&mut self, base: &str) -> lir::BlockId {
        self.block_count += 1;
        self.blocks.alloc(lir::BasicBlock {
            name: format!("{base}.{}", self.block_count),
            instructions: Vec::new(),
            terminator: lir::Terminator::Return { value: None }, // placeholder, see struct docs
        })
    }

    /// Seal the current block with its terminator.
    fn seal(&mut self, terminator: lir::Terminator) {
        self.blocks[self.current].terminator = terminator;
        self.current_sealed = true;
    }

    /// Make `block` the current (unsealed) block.
    fn enter(&mut self, block: lir::BlockId) {
        self.current = block;
        self.current_sealed = false;
    }

    fn push(&mut self, instruction: lir::Instruction) {
        self.blocks[self.current].instructions.push(instruction);
    }

    fn new_temp(&mut self, ty: lir::LirType) -> lir::TempId {
        self.temps.alloc(lir::Temp { ty })
    }

    fn next_safepoint(&mut self) -> lir::SafepointId {
        self.safepoint_ids.allocate()
    }

    /// A fresh hidden slot carrying a short-circuit result across
    /// basic blocks (LIR has no phi nodes; mem2reg removes it).
    fn new_hidden_local(&mut self, ty: lir::LirType) -> lir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(lir::Local {
            name: format!("$sc.{}", self.hidden_count),
            ty,
        })
    }

    fn exception_slots(&mut self) -> (lir::LocalId, lir::LocalId) {
        if let Some(slots) = self.exception_slots {
            return slots;
        }
        let record = self.new_hidden_local(lir::LirType::ExceptionRecord);
        let raw = self.new_hidden_local(lir::RAW_PTR);
        let slots = (record, raw);
        self.exception_slots = Some(slots);
        slots
    }

    /// The value of a MIR local: a stack slot load, or the SSA
    /// parameter itself.
    fn local_value(&self, local: mir::LocalId) -> lir::Value {
        match self.local_map[&local] {
            LocalSlot::Slot(id) => lir::Value::Local(id),
            LocalSlot::Param(index) => lir::Value::Param(index),
        }
    }

    /// The stack slot of a MIR local that is stored to. Parameters are
    /// immutable (M3), so stores never target them.
    fn local_slot(&self, local: mir::LocalId) -> lir::LocalId {
        match self.local_map[&local] {
            LocalSlot::Slot(id) => id,
            LocalSlot::Param(_) => {
                unreachable!("parameters are immutable; stores never target them")
            }
        }
    }

    /// The LIR value type of a MIR type; tuple types are recorded for
    /// the meta layouts on the way.
    fn value_type(&mut self, ty: &mir::Type) -> lir::LirType {
        record_layout_types(ty, self.layout_types);
        lir_type(ty)
    }

    fn lower_statements(&mut self, statements: &'a [mir::Statement]) {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &'a mir::Statement) {
        match &statement.kind {
            mir::StatementKind::Expr(expr) => {
                self.lower_expr(expr);
            }
            mir::StatementKind::Call(effect) => match effect {
                mir::CallEffect::Unit(call) => {
                    self.lower_call(call, &mir::Type::Unit);
                }
                mir::CallEffect::Value { destination, call } => {
                    let ty = self.mir_locals[*destination].ty.clone();
                    let value = self.lower_call(call, &ty);
                    self.push(lir::Instruction::Store {
                        local: self.local_slot(*destination),
                        value,
                    });
                }
            },
            // Initialization and assignment are both stores into the
            // local's stack slot.
            mir::StatementKind::ValDecl { local, init } => {
                let value = self.lower_expr(init);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::Assign { local, value } => {
                let value = self.lower_expr(value);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::GlobalAssign { global, value } => {
                let value = self.lower_expr(value);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalStore { global, value })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalStore {
                            global,
                            value,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
            }
            // `m[i] = v`: bounds check and the element store are
            // codegen's job; the element layout comes from the array
            // operand's type.
            mir::StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                let array = self.lower_expr(array);
                let index = self.lower_expr(index);
                let value = self.lower_expr(value);
                self.push(lir::Instruction::ArraySet {
                    array,
                    index,
                    value,
                    array_type: self.array_type_id(*array_type),
                });
            }
            // `obj.field = value`: MIR carries the flattened field
            // index; LIR fixes it to the class layout's byte offset.
            mir::StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("a field store targets a class object")
                };
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let offset = offsets[*index as usize];
                let object = self.lower_expr(object);
                let value = self.lower_expr(value);
                self.push(lir::Instruction::HeapStore {
                    object,
                    offset,
                    value,
                });
            }
            mir::StatementKind::AtomicFieldStore {
                object,
                index,
                value,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field store targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object);
                let value = self.lower_expr(value);
                self.push(lir::Instruction::AtomicStore {
                    object,
                    offset: offsets[*index as usize],
                    value,
                });
            }
            mir::StatementKind::Eh(eh) => self.lower_eh_statement(eh),
        }
    }

    fn lower_eh_statement(&mut self, statement: &mir::EhStatement) {
        match statement {
            mir::EhStatement::LandingPad { cleanup } => {
                let (record_slot, raw_slot) = self.exception_slots();
                let record = self.new_temp(lir::LirType::ExceptionRecord);
                let raw = self.new_temp(lir::RAW_PTR);
                if *cleanup {
                    self.push(lir::Instruction::CleanupPad { record, raw });
                } else {
                    self.push(lir::Instruction::LandingPad { record, raw });
                }
                self.push(lir::Instruction::Store {
                    local: record_slot,
                    value: lir::Value::Temp(record),
                });
                self.push(lir::Instruction::Store {
                    local: raw_slot,
                    value: lir::Value::Temp(raw),
                });
            }
            mir::EhStatement::BeginCatch => {
                let (_, raw_slot) = self.exception_slots();
                let exception = self.new_temp(lir::MANAGED_PTR);
                self.push(lir::Instruction::BeginCatch {
                    out: exception,
                    raw: lir::Value::Local(raw_slot),
                });
                let slot = match self.caught_exception {
                    Some(slot) => slot,
                    None => {
                        let slot = self.new_hidden_local(lir::MANAGED_PTR);
                        self.caught_exception = Some(slot);
                        slot
                    }
                };
                self.push(lir::Instruction::Store {
                    local: slot,
                    value: lir::Value::Temp(exception),
                });
            }
            mir::EhStatement::EndCatch => self.push(lir::Instruction::EndCatch),
        }
    }

    fn lower_terminator(&mut self, terminator: &mir::Terminator) {
        match terminator {
            mir::Terminator::Goto(target) => {
                self.seal(lir::Terminator::Br(self.block_map[target]));
            }
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.lower_expr(cond);
                self.seal(lir::Terminator::CondBr {
                    cond,
                    then_block: self.block_map[then_block],
                    else_block: self.block_map[else_block],
                });
            }
            mir::Terminator::Return { value } => {
                let value = match (self.returns_void, value) {
                    (true, None) => None,
                    (true, Some(value)) => {
                        self.lower_expr(value);
                        None
                    }
                    (false, Some(value)) => Some(self.lower_expr(value)),
                    (false, None) => unreachable!("non-Unit return without a value"),
                };
                self.seal(lir::Terminator::Return { value });
            }
            mir::Terminator::Throw { exception, unwind } => {
                let value = self.lower_expr(exception);
                if let Some(unwind) = unwind {
                    let normal = self.new_block("throw.normal");
                    let (call, _) =
                        self.typed_call(vec![lir::MANAGED_PTR], lir::LirType::Void, vec![value]);
                    let site = self.invoke_site(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Throw),
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                } else {
                    self.push(lir::Instruction::Throw { exception: value });
                    self.seal(lir::Terminator::Unreachable);
                }
            }
            mir::Terminator::Rethrow { unwind } => match unwind {
                Some(unwind) => {
                    let normal = self.new_block("rethrow.normal");
                    let (call, _) = self.typed_call(Vec::new(), lir::LirType::Void, Vec::new());
                    let site = self.invoke_site(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Rethrow),
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                }
                None => {
                    self.emit_plain_call(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Rethrow),
                        Vec::new(),
                        lir::LirType::Void,
                        Vec::new(),
                    );
                    self.seal(lir::Terminator::Unreachable);
                }
            },
            mir::Terminator::Resume => {
                let (record, _) = self.exception_slots();
                self.seal(lir::Terminator::Resume {
                    exception: lir::Value::Local(record),
                });
            }
            mir::Terminator::Trap { message } => {
                let message = self.module.strings[*message].value.clone();
                let trap = self.trap_block(&message);
                self.seal(lir::Terminator::Br(trap));
            }
            mir::Terminator::Unreachable => self.seal(lir::Terminator::Unreachable),
        }
    }
}
