use super::*;

pub(super) struct MachineValues<'a> {
    module: &'a lir::Module,
    structs: std::collections::HashMap<lir::StructDefId, bool>,
}

impl<'a> MachineValues<'a> {
    pub(super) fn new(module: &'a lir::Module) -> Self {
        Self {
            module,
            structs: Default::default(),
        }
    }

    pub(super) fn gc_free(&mut self, ty: &lir::LirType) -> bool {
        use lir::LirType as T;
        match ty {
            T::Ptr(lir::PointerKind::Managed) | T::Interface | T::ExceptionRecord => false,
            T::Aggregate(elements) => elements.iter().all(|ty| self.gc_free(ty)),
            T::Struct(id) => {
                if let Some(value) = self.structs.get(id) {
                    return *value;
                }
                let value = match &self.module.structs[*id].representation {
                    lir::StructRepresentation::Scoop { fields } => {
                        fields.iter().all(|field| self.gc_free(&field.ty))
                    }
                    lir::StructRepresentation::C { .. } => true,
                    lir::StructRepresentation::Intrinsic(_) => false,
                };
                self.structs.insert(*id, value);
                value
            }
            T::Enum(id) => self.module.enums[*id].scan == lir::RefScan::None,
            T::Void
            | T::I1
            | T::I8
            | T::I16
            | T::I32
            | T::F32
            | T::F64
            | T::I64
            | T::MachineScalar(_)
            | T::Ptr(_) => true,
        }
    }
}
