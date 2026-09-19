use super::*;

mod place;
pub use place::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableBodyIdentity {
    record: scoop_identity::RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId>,
    symbol: MaterializedSymbol,
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
        Self::from_key(
            scoop_identity::CallableBodyKey::odr(member),
            LinkageClass::OdrWeak,
        )
    }

    pub fn for_initialization_startup_gateway(
        unit: scoop_identity::PersistentInitializationUnitId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::from_key(
            scoop_identity::CallableBodyKey::initialization_startup_gateway(unit),
            LinkageClass::ConeStrong,
        )
    }

    pub fn for_root_gateway(
        root_cone: scoop_identity::ConeIdentity,
        main: scoop_identity::MainCallableBodyId,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::from_key(
            scoop_identity::CallableBodyKey::root_gateway(root_cone, main),
            LinkageClass::ConeStrong,
        )
    }

    pub const fn id(&self) -> scoop_identity::PersistentCallableBodyId {
        self.record.id()
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId> {
        &self.record
    }

    pub const fn symbol_request(&self) -> PersistentSymbolRequest {
        self.symbol.request()
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }

    fn strong(
        owner: scoop_identity::StrongCallableDefinitionOwner,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        Self::from_key(
            scoop_identity::CallableBodyKey::strong(owner),
            LinkageClass::ConeStrong,
        )
    }

    fn from_key(
        key: scoop_identity::CallableBodyKey,
        linkage: LinkageClass,
    ) -> Result<Self, CallableBodyIdentityBuildError> {
        let record = scoop_identity::RuntimeIdentityRecord::from_key(&key)?;
        Ok(Self {
            symbol: MaterializedSymbol::new(
                scoop_identity::PersistentSymbolKey::CallableBody(record.id()),
                linkage,
            )
            .expect("the closed callable-body key/linkage pair is valid"),
            record,
        })
    }
}

pub type CallableBodyIdentityBuildError =
    scoop_identity::RuntimeIdentityRecordBuildError<scoop_wire::HashError>;

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
            Value::Local(id) => self.locals[id].ty().clone(),
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
