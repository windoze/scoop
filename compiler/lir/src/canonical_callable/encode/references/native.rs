use super::*;

impl Writer<'_, '_> {
    pub(in super::super) fn native_global(&mut self, id: NativeGlobalId) -> Result {
        let global = &self.module.native_globals[id];
        record!(self, 1; self.text(&global.library), self.text(&global.native_symbol), self.c_type(&global.c_type), self.boolean(global.thread_local), self.native_access(global.access))
    }

    fn native_access(&mut self, access: NativeGlobalAccess) -> Result {
        let bridges = &self.module.native_global_bridges;
        match access {
            NativeGlobalAccess::ReadOnly { get, address } => {
                record!(self, 1; self.id(&bridges.gets[get].identity.unit()), self.id(&bridges.addresses[address].identity.unit()))
            }
            NativeGlobalAccess::Mutable { get, set, address } => {
                record!(self, 2; self.id(&bridges.gets[get].identity.unit()), self.id(&bridges.sets[set].identity.unit()), self.id(&bridges.addresses[address].identity.unit()))
            }
        }
    }

    fn c_type(&mut self, ty: &CType) -> Result {
        match ty {
            CType::Integer(kind) => record!(self, 1; self.integer_kind(*kind)),
            CType::Boolean => record!(self, 2;),
            CType::Float(kind) => record!(self, 6; self.u(u64::from(kind.bits()))),
            CType::DataPointer { pointee, storage } => {
                record!(self, 3; self.c_pointee(pointee), self.c_data_storage(storage))
            }
            CType::CodePointer { signature, storage } => {
                record!(self, 4; self.c_signature(signature), self.c_code_storage(storage))
            }
            CType::Struct(reference) => {
                record!(self, 5; self.id(&self.module.structs[reference.definition()].exact_type))
            }
        }
    }

    fn c_pointee(&mut self, pointee: &CDataPointee) -> Result {
        match pointee {
            CDataPointee::OpaqueVoid => record!(self, 1;),
            CDataPointee::Object(ty) => record!(self, 2; self.c_type(ty)),
        }
    }

    pub(in super::super) fn c_signature(&mut self, signature: &CFunctionType) -> Result {
        self.e.map(2)?;
        self.e.field(1)?;
        self.e.array(signature.params.len() as u64)?;
        for param in &signature.params {
            self.c_type(param)?;
        }
        self.e.field(2)?;
        match &signature.return_type {
            CReturnType::Void => record!(self, 1;),
            CReturnType::Value(ty) => record!(self, 2; self.c_type(ty)),
        }
    }

    fn c_data_storage(&mut self, storage: &CDataPointerStorage) -> Result {
        match storage {
            CDataPointerStorage::Direct => record!(self, 1;),
            CDataPointerStorage::Nullable(reference) => {
                record!(self, 2; self.id(&self.module.enums[reference.definition()].exact_type), self.c_pointee(reference.pointee()))
            }
        }
    }

    fn c_code_storage(&mut self, storage: &CCodePointerStorage) -> Result {
        match storage {
            CCodePointerStorage::Direct => record!(self, 1;),
            CCodePointerStorage::Nullable(reference) => {
                record!(self, 2; self.id(&self.module.enums[reference.definition()].exact_type), self.c_signature(reference.signature()))
            }
        }
    }
}
