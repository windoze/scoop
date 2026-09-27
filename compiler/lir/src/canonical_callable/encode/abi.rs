use super::*;

impl Writer<'_, '_> {
    pub(super) fn ty(&mut self, ty: &LirType) -> Result {
        match ty {
            LirType::Void => record!(self, 1;),
            LirType::I1 => record!(self, 2;),
            LirType::I8 => record!(self, 3;),
            LirType::I16 => record!(self, 4;),
            LirType::I32 => record!(self, 5;),
            LirType::I64 => record!(self, 6;),
            LirType::MachineScalar(kind) => record!(self, 7; self.machine_kind(*kind)),
            LirType::Ptr(kind) => record!(self, 8; self.pointer_kind(*kind)),
            LirType::ExceptionRecord => record!(self, 9;),
            LirType::Aggregate(elements) => record!(self, 10; self.types(elements)),
            LirType::Struct(id) => record!(self, 11; self.id(&self.module.structs[*id].exact_type)),
            LirType::Enum(id) => record!(self, 12; self.id(&self.module.enums[*id].exact_type)),
        }
    }

    fn types(&mut self, types: &[LirType]) -> Result {
        self.e.array(types.len() as u64)?;
        for ty in types {
            self.ty(ty)?;
        }
        Ok(())
    }

    pub(super) fn abi_value(&mut self, value: &AbiValue) -> Result {
        record!(self, 1; self.ty(value.storage_type()), self.u(value.layout().size().get()), self.u(value.layout().alignment().get()), self.scan(value.scan()))
    }

    pub(super) fn abi_zst(&mut self, value: &AbiZst) -> Result {
        record!(self, 1; self.ty(value.storage_type()), self.u(value.layout().alignment().get()))
    }

    pub(super) fn logical_zst(&mut self, value: &LogicalZstValue) -> Result {
        record!(self, 1; self.id(&value.exact()), self.abi_zst(value.representation()))
    }

    pub(super) fn arguments(&mut self, arguments: &[AbiArgument]) -> Result {
        self.e.array(arguments.len() as u64)?;
        for argument in arguments {
            match argument {
                AbiArgument::ElidedZst(value) => record!(self, 1; self.abi_zst(value))?,
                AbiArgument::Direct(value) => record!(self, 2; self.abi_value(value))?,
                AbiArgument::Indirect(value) => record!(self, 3; self.abi_value(value))?,
            }
        }
        Ok(())
    }

    pub(super) fn signature(&mut self, signature: &ScoopAbiSignature) -> Result {
        record!(self, 1; self.id(&signature.calling_convention()), self.arguments(signature.arguments()), self.abi_return(signature.result()))
    }

    fn abi_return(&mut self, result: &AbiReturn) -> Result {
        match result {
            AbiReturn::UnitVoid => record!(self, 1;),
            AbiReturn::ElidedZst(value) => record!(self, 2; self.abi_zst(value)),
            AbiReturn::Direct(value) => record!(self, 3; self.abi_value(value)),
            AbiReturn::Indirect(value) => record!(self, 4; self.abi_value(value)),
        }
    }

    pub(super) fn scan(&mut self, scan: &RefScan) -> Result {
        scan.encode(self.e)
    }

    pub(super) fn pointer_kind(&mut self, kind: PointerKind) -> Result {
        self.u(match kind {
            PointerKind::Managed => 1,
            PointerKind::Raw => 2,
            PointerKind::Code => 3,
            PointerKind::Metadata => 4,
        })
    }

    pub(super) fn machine_kind(&mut self, kind: MachineScalarKind) -> Result {
        self.u(match kind {
            MachineScalarKind::ByteSize => 1,
            MachineScalarKind::EnumTag => 2,
            MachineScalarKind::InitializationOutcome => 3,
            MachineScalarKind::CoroutineFrameState => 4,
            MachineScalarKind::CoroutineAdapterState => 5,
            MachineScalarKind::ForeignCallbackStatus => 6,
            MachineScalarKind::PointerElementOffset => 7,
        })
    }

    pub(super) fn integer_kind(&mut self, kind: IntegerKind) -> Result {
        let signedness = match kind.signedness() {
            IntegerSignedness::Signed => 1,
            IntegerSignedness::Unsigned => 2,
        };
        record!(self, signedness; self.u(u64::from(kind.width().bits())))
    }
}
