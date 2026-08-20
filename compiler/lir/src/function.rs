use super::*;

mod place;
pub use place::*;

mod identity;
pub use identity::*;

/// A temporary SSA-ish value produced by an instruction.
#[derive(Debug)]
pub struct Temp {
    pub ty: LirType,
}

#[derive(Debug)]
pub struct Function {
    /// Persistent identity and canonical runtime key of this emitted Scoop
    /// machine body. Native/runtime declarations use their own typed targets
    /// and never fabricate a callable-body identity.
    pub callable_body: CallableBodyIdentity,
    /// Whether codegen must attach the GC strategy. Polls are explicit LIR
    /// instructions with their own typed root plans.
    pub gc_effect: GcEffect,
    /// The sole logical and physical Scoop ABI signature for this definition.
    /// Arguments remain addressable by logical source index (`Value::Param`).
    pub signature: ScoopAbiSignature,
    /// Function-local call entities. Targets may contain local SSA operands
    /// (for dispatch tables), so their ids are scoped to this function.
    pub call_targets: CallTargets,
    /// Complete identity relation for every safepoint reference in `blocks`.
    pub safepoints: SafepointIdentities,
    pub locals: Arena<Local>,
    pub temps: Arena<Temp>,
    pub blocks: Arena<BasicBlock>,
    /// Entry block; every function has exactly one.
    pub entry: BlockId,
}

impl Function {
    pub fn symbol(&self) -> &str {
        self.callable_body.symbol()
    }

    /// The type of a value in this function.
    pub fn value_ty(&self, globals: &Arena<Global>, value: Value) -> LirType {
        match value {
            Value::ContextKeyCell(_) => RAW_PTR,
            Value::Local(id) => self.locals[id].ty().clone(),
            Value::Temp(id) => self.temps[id].ty.clone(),
            Value::Param(index) => self.signature.arguments()[index as usize]
                .logical_storage_type()
                .clone(),
            Value::IntegerConst(value) => value.scalar_type(),
            Value::FloatConst(value) => LirType::floating(value.kind()),
            Value::MachineScalar(value) => LirType::MachineScalar(value.kind()),
            Value::BoolConst(_) => LirType::I1,
            Value::NullPointer(kind) => LirType::Ptr(kind),
            Value::TypeDescriptor(_) => METADATA_PTR,
            Value::RootScan(_) => METADATA_PTR,
            Value::Global(id) => LirType::Ptr(globals[id].address_kind),
            Value::InitializationUnit(_) => METADATA_PTR,
            Value::CArgumentStorage(_) => RAW_PTR,
        }
    }
}
