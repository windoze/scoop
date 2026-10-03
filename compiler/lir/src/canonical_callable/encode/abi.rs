use super::*;

impl Writer<'_, '_> {
    pub(super) fn ty(&mut self, ty: &LirType) -> Result {
        crate::canonical_type::encode_type(self.module, ty, self.e)
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
        self.u(crate::canonical_type::pointer_kind(kind))
    }

    pub(super) fn machine_kind(&mut self, kind: MachineScalarKind) -> Result {
        self.u(crate::canonical_type::machine_kind(kind))
    }

    pub(super) fn integer_kind(&mut self, kind: IntegerKind) -> Result {
        crate::canonical_type::encode_integer(kind, self.e)
    }
}
