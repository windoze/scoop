use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableBodyIdentity {
    record: scoop_identity::RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId>,
}

impl CallableBodyIdentity {
    pub fn for_function(
        function: scoop_identity::PersistentFunctionId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::strong(scoop_identity::StrongCallableDefinitionOwner::Function(
            function,
        ))
    }

    pub fn for_constructor(
        constructor: scoop_identity::PersistentConstructorId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::strong(scoop_identity::StrongCallableDefinitionOwner::Constructor(
            constructor,
        ))
    }

    pub fn for_property_accessor(
        accessor: scoop_identity::PersistentPropertyAccessorId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::strong(scoop_identity::StrongCallableDefinitionOwner::PropertyAccessor(accessor))
    }

    pub fn for_generated_callable(
        callable: scoop_identity::PersistentGeneratedCallableId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::strong(scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(callable))
    }

    pub fn for_odr_member(
        member: scoop_identity::CallableOdrMemberId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::from_key(scoop_identity::CallableBodyKey::odr(member))
    }

    pub const fn id(&self) -> scoop_identity::PersistentCallableBodyId {
        self.record.id()
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId> {
        &self.record
    }

    fn strong(
        owner: scoop_identity::StrongCallableDefinitionOwner,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::from_key(scoop_identity::CallableBodyKey::strong(owner))
    }

    fn from_key(
        key: scoop_identity::CallableBodyKey,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Ok(Self {
            record: scoop_identity::RuntimeIdentityRecord::from_key(&key)?,
        })
    }
}

pub type CallableBodyIdentityBuildError =
    scoop_identity::RuntimeIdentityRecordBuildError<scoop_wire::HashError>;

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
    /// Persistent identity and canonical runtime key of this emitted Scoop
    /// machine body. Native/runtime declarations use their own typed targets
    /// and never fabricate a callable-body identity.
    pub callable_body: CallableBodyIdentity,
    /// Whether codegen must attach the GC strategy. Polls are explicit LIR
    /// instructions with their own typed root plans.
    pub gc_effect: GcEffect,
    pub symbol: String,
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
    /// The type of a value in this function.
    pub fn value_ty(&self, globals: &Arena<Global>, value: Value) -> LirType {
        match value {
            Value::Local(id) => self.locals[id].ty.clone(),
            Value::Temp(id) => self.temps[id].ty.clone(),
            Value::Param(index) => self.signature.arguments()[index as usize]
                .logical_storage_type()
                .clone(),
            Value::IntegerConst(value) => value.scalar_type(),
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
