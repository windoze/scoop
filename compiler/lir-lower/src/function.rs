use super::*;

mod arrays;
mod boxing;
mod call;
mod expression;
mod expression_support;
mod objects;
mod places;
mod pointers;
mod scalars;
mod statements;

use call::LoweredCallDestination;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MappedLoopHeaderPollTarget {
    block: lir::BlockId,
}

impl MappedLoopHeaderPollTarget {
    pub(super) fn new(block: lir::BlockId) -> Self {
        Self { block }
    }

    pub(super) fn block(self) -> lir::BlockId {
        self.block
    }
}

pub(super) struct LoweredFunction {
    pub(super) function: lir::Function,
    pub(super) loop_header_polls: Vec<MappedLoopHeaderPollTarget>,
    pub(super) pending_safepoints: safepoints::PendingSafepointSites,
}

/// Map a primitive MIR binary operator onto its LIR opcode. Operand and result
/// types come exclusively from the typed MIR expressions.
fn binary_op(op: mir::BinOp) -> lir::BinOp {
    match op {
        mir::BinOp::BoolEq | mir::BinOp::RefEq => lir::BinOp::Eq,
        mir::BinOp::BoolNe | mir::BinOp::RefNe => lir::BinOp::Ne,
        mir::BinOp::MachineEq(kind) => lir::BinOp::MachineEq(machine_scalar_kind(kind)),
    }
}

fn integer_binary_op(op: mir::IntegerBinaryOperator) -> lir::IntegerBinaryOperation {
    match op {
        mir::IntegerBinaryOperator::Add => lir::IntegerBinaryOperation::Add,
        mir::IntegerBinaryOperator::Subtract => lir::IntegerBinaryOperation::Subtract,
        mir::IntegerBinaryOperator::Multiply => lir::IntegerBinaryOperation::Multiply,
        mir::IntegerBinaryOperator::BitAnd => lir::IntegerBinaryOperation::BitwiseAnd,
        mir::IntegerBinaryOperator::BitOr => lir::IntegerBinaryOperation::BitwiseOr,
        mir::IntegerBinaryOperator::BitXor => lir::IntegerBinaryOperation::BitwiseXor,
    }
}

fn integer_div_rem_op(op: mir::SafeIntegerDivRemOperator) -> lir::IntegerDivRemOperation {
    match op {
        mir::SafeIntegerDivRemOperator::Divide => lir::IntegerDivRemOperation::Divide,
        mir::SafeIntegerDivRemOperator::Remainder => lir::IntegerDivRemOperation::Remainder,
    }
}

fn integer_comparison(op: mir::IntegerComparisonOperator) -> lir::IntegerComparison {
    match op {
        mir::IntegerComparisonOperator::LessThan => lir::IntegerComparison::Less,
        mir::IntegerComparisonOperator::LessThanOrEqual => lir::IntegerComparison::LessOrEqual,
        mir::IntegerComparisonOperator::GreaterThan => lir::IntegerComparison::Greater,
        mir::IntegerComparisonOperator::GreaterThanOrEqual => {
            lir::IntegerComparison::GreaterOrEqual
        }
        mir::IntegerComparisonOperator::Equal => lir::IntegerComparison::Equal,
        mir::IntegerComparisonOperator::NotEqual => lir::IntegerComparison::NotEqual,
    }
}

fn integer_shift_op(operation: mir::IntegerShiftOperation) -> lir::IntegerShiftOperation {
    match (operation.value_kind().signedness(), operation.operator()) {
        (_, mir::IntegerShiftOperator::Left) => lir::IntegerShiftOperation::Left,
        (mir::IntegerSignedness::Signed, mir::IntegerShiftOperator::Right) => {
            lir::IntegerShiftOperation::ArithmeticRight
        }
        (mir::IntegerSignedness::Unsigned, mir::IntegerShiftOperator::Right)
        | (mir::IntegerSignedness::Signed, mir::IntegerShiftOperator::UnsignedRight) => {
            lir::IntegerShiftOperation::LogicalRight
        }
        (mir::IntegerSignedness::Unsigned, mir::IntegerShiftOperator::UnsignedRight) => {
            unreachable!("MIR rejects unsigned ushr when constructing its typed operation")
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_function<'a>(
    context: &'a LoweringContext,
    producer: scoop_identity::ConeIdentity,
    module: &'a mir::Module,
    callable_body: lir::CallableBodyIdentity,
    function: &'a mir::Function,
    signature: &'a lir::ScoopAbiSignature,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &HashMap<mir::GlobalId, StorageGlobal>,
    globals: &mut Arena<lir::Global>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    function_signatures: &'a HashMap<mir::FunctionId, lir::ScoopAbiSignature>,
    external_callables: &'a Arena<lir::ExternalCallable>,
    external_callable_map: &'a HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
    extern_functions: &'a lir::ExternFunctions,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
) -> StorageResult<LoweredFunction> {
    // Parameters stay SSA values unless `addressOf` requires stable storage.
    // Address-taken parameters are copied once into a method-local slot.
    let address_taken = locals::address_taken(function);
    let mut local_map = HashMap::new();
    assert_eq!(
        function.params.len(),
        signature.logical_argument_count(),
        "preclassified function signature preserves logical arity"
    );
    for (index, (param, abi_argument)) in function
        .params
        .iter()
        .zip(signature.arguments())
        .enumerate()
    {
        assert_eq!(
            &lir_type(&param.ty),
            abi_argument.logical_storage_type(),
            "preclassified function parameter preserves its LIR storage type"
        );
        if !address_taken.contains(&param.local) {
            local_map.insert(param.local, LocalSlot::Param(index as u32));
        }
    }

    // One LIR stack slot per non-parameter MIR local, in declaration
    // order.
    let mut locals = Arena::new();
    for (mir_id, local) in function.body.locals.iter() {
        if local_map.contains_key(&mir_id) {
            continue; // a parameter
        }
        let lir_id = locals.alloc(places::source_local(
            context,
            module,
            local,
            address_taken.contains(&mir_id),
            structs,
            enums,
        )?);
        local_map.insert(mir_id, LocalSlot::Slot(lir_id));
    }

    // Unit-returning functions are void at the LLVM level (DESIGN 2.4).
    let returns_void = matches!(signature.result(), lir::AbiReturn::UnitVoid);
    assert_eq!(returns_void, function.return_ty == mir::Type::Unit);
    if let Some(storage_type) = signature.result().logical_storage_type() {
        assert_eq!(
            storage_type,
            &lir_type(&function.return_ty),
            "preclassified function result preserves its LIR storage type"
        );
    }

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
    let loop_header_polls = function
        .body
        .loop_header_polls
        .iter()
        .map(|target| MappedLoopHeaderPollTarget::new(block_map[&target.header()]))
        .collect();
    let mut pending_safepoints = safepoints::PendingSafepointSites::default();
    let mut lowerer = FunctionLowerer {
        context,
        producer,
        callable_body: &callable_body,
        module,
        mir_locals: &function.body.locals,
        global_map,
        storage_globals,
        globals,
        cstr_count: 0,
        structs,
        enums,
        array_types,
        type_descriptors,
        local_function_map,
        function_signatures,
        external_callables,
        external_callable_map,
        extern_functions,
        extern_function_refs,
        pending_safepoints: &mut pending_safepoints,
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
        lowerer.lower_statements(&block.statements)?;
        if !lowerer.current_sealed {
            lowerer.lower_terminator(&block.terminator)?;
        }
        assert!(lowerer.current_sealed, "every MIR block has a terminator");
    }
    Ok(LoweredFunction {
        function: lir::Function {
            gc_effect: match function.gc_effect {
                mir::GcEffect::Managed => lir::GcEffect::Managed,
                mir::GcEffect::NoGc => lir::GcEffect::NoGc,
            },
            signature: signature.clone(),
            call_targets: lowerer.call_targets,
            safepoints: lir::SafepointIdentities::default(),
            locals: lowerer.locals,
            temps: lowerer.temps,
            blocks: lowerer.blocks,
            entry,
            callable_body,
        },
        loop_header_polls,
        pending_safepoints,
    })
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
    context: &'a LoweringContext,
    producer: scoop_identity::ConeIdentity,
    callable_body: &'a lir::CallableBodyIdentity,
    module: &'a mir::Module,
    /// Locals of the MIR function being lowered (for local storage and parameters).
    mir_locals: &'a Arena<mir::Local>,
    global_map: &'a HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &'a HashMap<mir::GlobalId, StorageGlobal>,
    /// Sink for ordinary globals such as trap-message C strings.
    globals: &'a mut Arena<lir::Global>,
    cstr_count: u32,
    /// Complete value layouts used to classify return conventions and scans.
    structs: &'a lir::StructDefs,
    /// Enum definitions with complete fixed value representations.
    enums: &'a lir::EnumDefs,
    /// Complete class-application to array-metadata mapping produced before
    /// any function is lowered.
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    /// Complete typed TypeDescriptor graph built before body lowering.
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    function_signatures: &'a HashMap<mir::FunctionId, lir::ScoopAbiSignature>,
    external_callables: &'a Arena<lir::ExternalCallable>,
    external_callable_map: &'a HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
    extern_functions: &'a lir::ExternFunctions,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
    pending_safepoints: &'a mut safepoints::PendingSafepointSites,
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

    fn value_layout(&self, ty: &mir::Type) -> StorageResult<(u64, u64)> {
        let enum_shape =
            |id: mir::EnumId| Ok(repr_shape(self.context, &self.enums[enum_def_id(id)].repr));
        size_align(self.context, self.module, &enum_shape, ty)
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

    fn new_safepoint(&mut self, role: lir::SafepointSiteRole) -> lir::SafepointSiteRef {
        self.pending_safepoints.allocate(role)
    }

    /// A fresh hidden slot carrying a short-circuit result across
    /// basic blocks (LIR has no phi nodes; mem2reg removes it).
    fn new_hidden_local(&mut self, ty: lir::LirType) -> StorageResult<lir::LocalId> {
        self.hidden_count += 1;
        let value = match abi::classify_argument(self.context, ty, self.structs, self.enums)? {
            lir::AbiArgument::Direct(value) | lir::AbiArgument::Indirect(value) => value,
            lir::AbiArgument::ElidedZst(_) => {
                unreachable!("hidden physical storage requires a nonzero ABI value")
            }
        };
        Ok(self.locals.alloc(lir::Local::new(
            format!("$sc.{}", self.hidden_count),
            lir::LocalStorage::NonZero(value),
        )))
    }

    fn exception_slots(&mut self) -> StorageResult<(lir::LocalId, lir::LocalId)> {
        if let Some(slots) = self.exception_slots {
            return Ok(slots);
        }
        let record = self.new_hidden_local(lir::LirType::ExceptionRecord)?;
        let raw = self.new_hidden_local(lir::RAW_PTR)?;
        let slots = (record, raw);
        self.exception_slots = Some(slots);
        Ok(slots)
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

    /// The transient physical LIR shape of a MIR value type.
    fn value_type(&mut self, ty: &mir::Type) -> lir::LirType {
        lir_type(ty)
    }
}
