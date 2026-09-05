use super::*;

/// A local variable's stack slot.
#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: LirType,
}

/// A temporary SSA-ish value produced by an instruction.
#[derive(Debug)]
pub struct Temp {
    pub ty: LirType,
}

#[derive(Debug)]
pub struct Function {
    /// Whether codegen must attach the GC strategy. Polls are explicit LIR
    /// instructions with their own typed root plans.
    pub gc_effect: GcEffect,
    pub symbol: String,
    /// Parameter types; arguments are SSA values (`Value::Param`).
    pub params: Vec<LirType>,
    pub return_ty: LirType,
    /// Function-local call entities. Targets may contain local SSA operands
    /// (for dispatch tables), so their ids are scoped to this function.
    pub call_targets: CallTargets,
    pub locals: Arena<Local>,
    pub temps: Arena<Temp>,
    pub blocks: Arena<BasicBlock>,
    /// Entry block; every function has exactly one.
    pub entry: BlockId,
}

impl Function {
    /// The type of a value in this function.
    pub fn value_ty(&self, globals: &Arena<Global>, value: Value) -> LirType {
        match value {
            Value::Local(id) => self.locals[id].ty.clone(),
            Value::Temp(id) => self.temps[id].ty.clone(),
            Value::Param(index) => self.params[index as usize].clone(),
            Value::IntConst(_) => LirType::I64,
            Value::MachineScalar(value) => LirType::MachineScalar(value.kind()),
            Value::BoolConst(_) => LirType::I1,
            Value::NullPointer(kind) => LirType::Ptr(kind),
            Value::TypeDescriptor(_) => METADATA_PTR,
            Value::RootScan(_) => METADATA_PTR,
            Value::Global(id) => LirType::Ptr(globals[id].address_kind),
            Value::InitializationUnit(_) => METADATA_PTR,
        }
    }
}
