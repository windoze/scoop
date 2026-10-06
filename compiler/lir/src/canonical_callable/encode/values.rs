use super::*;

impl Writer<'_, '_> {
    pub(super) fn value(&mut self, value: Value) -> Result {
        match value {
            Value::ContextKeyCell(key) => record!(self, 13; self.id(&key.0)),
            Value::Local(id) => record!(self, 1; self.local(id)),
            Value::Param(index) => record!(self, 2; self.u(u64::from(index))),
            Value::Temp(id) => record!(self, 3; self.temp(id)),
            Value::IntegerConst(value) => {
                record!(self, 4; self.integer_kind(value.kind()), self.u(value.raw_bits()))
            }
            Value::FloatConst(value) => {
                record!(self, 14; self.u(u64::from(value.kind().bits())), self.u(value.raw_bits()))
            }
            Value::MachineScalar(value) => {
                record!(self, 5; self.machine_kind(value.kind()), self.u(value.raw_bits()))
            }
            Value::BoolConst(value) => record!(self, 6; self.boolean(value)),
            Value::NullPointer(kind) => record!(self, 7; self.pointer_kind(kind)),
            Value::TypeDescriptor(id) => record!(self, 8; self.descriptor(id)),
            Value::RootScan(id) => {
                record!(self, 9; self.scan(&self.function.call_targets.root_scans[id]))
            }
            Value::Global(id) => record!(self, 10; self.global(id)),
            Value::InitializationUnit(id) => {
                record!(self, 11; self.id(&self.module.initialization_units[id].identity.id()))
            }
            Value::CArgumentStorage(storage) => record!(self, 12; self.local(storage.local())),
        }
    }

    pub(super) fn values(&mut self, values: &[Value]) -> Result {
        self.e.array(values.len() as u64)?;
        for value in values {
            self.value(*value)?;
        }
        Ok(())
    }

    pub(super) fn local(&mut self, local: LocalId) -> Result {
        let next = self.ids.locals.len() as u64;
        let rank = *self.ids.locals.entry(local).or_insert(next);
        let storage = self.function.locals[local].storage();
        record!(self, 1; self.u(rank), self.local_storage(storage))
    }

    fn local_storage(&mut self, storage: &LocalStorage) -> Result {
        match storage {
            LocalStorage::LogicalZst(value) => record!(self, 1; self.logical_zst(value)),
            LocalStorage::AddressableZst(place) => {
                let lifetime = match place.lifetime() {
                    LocalPlaceLifetime::FunctionActivation => 1,
                };
                record!(self, 2; self.logical_zst(place.value()), self.u(lifetime))
            }
            LocalStorage::NonZero(value) => record!(self, 3; self.abi_value(value)),
        }
    }

    pub(super) fn temp(&mut self, temp: TempId) -> Result {
        let next = self.ids.temps.len() as u64;
        let rank = *self.ids.temps.entry(temp).or_insert(next);
        record!(self, 1; self.u(rank), self.ty(&self.function.temps[temp].ty))
    }

    pub(super) fn call_arguments(&mut self, arguments: &[AbiCallArgument]) -> Result {
        self.e.array(arguments.len() as u64)?;
        for argument in arguments {
            match argument {
                AbiCallArgument::ElidedZst(value) => record!(self, 1; self.value(*value))?,
                AbiCallArgument::Direct(value) => record!(self, 2; self.value(*value))?,
                AbiCallArgument::Indirect(storage) => {
                    record!(self, 3; self.local(storage.local()))?
                }
            }
        }
        Ok(())
    }

    pub(super) fn variant(&mut self, variant: LirVariantRef) -> Result {
        record!(self, 1; self.id(&self.module.enums[variant.definition()].exact_type), self.u(u64::from(variant.index())))
    }

    pub(super) fn field(&mut self, field: LirVariantFieldRef) -> Result {
        record!(self, 1; self.variant(field.variant()), self.u(u64::from(field.index())))
    }
}
