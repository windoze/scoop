use super::*;

impl Collector<'_> {
    pub(super) fn children(
        &mut self,
        ty: TypeId,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        let depth = depth
            .checked_add(1)
            .ok_or(MaterializedTypeClosureError::DepthOverflow)?;
        let module = self.module;
        match &module.types[ty].kind {
            TypeKind::Tuple(elements) => self.types(elements.iter().copied(), depth, meter),
            TypeKind::Ptr(pointee) => self.add(*pointee, depth, meter),
            TypeKind::Function(signature) | TypeKind::FunPtr(signature) => {
                self.function_type(*signature, depth, meter)
            }
            TypeKind::Struct(id) => {
                let declaration = &module.structs[*id];
                self.types(declaration.type_arguments.iter().copied(), depth, meter)?;
                self.types(declaration.interfaces.iter().copied(), depth, meter)?;
                match &declaration.representation {
                    StructRepresentation::Declared { fields, .. } => {
                        self.types(fields.iter().map(|f| f.ty), depth, meter)
                    }
                    StructRepresentation::Intrinsic { application, .. } => {
                        self.intrinsic(application, depth, meter)
                    }
                }
            }
            TypeKind::Class(id) => {
                let declaration = &module.classes[*id];
                self.types(declaration.type_arguments.iter().copied(), depth, meter)?;
                self.types(declaration.interfaces.iter().copied(), depth, meter)?;
                match &declaration.representation {
                    ClassRepresentation::Declared { fields, base_class } => {
                        self.types(fields.iter().map(|f| f.ty), depth, meter)?;
                        self.types(
                            base_class.map(|id| module.classes[id].canonical_type),
                            depth,
                            meter,
                        )
                    }
                    ClassRepresentation::Intrinsic { application, .. } => {
                        self.intrinsic(application, depth, meter)
                    }
                }
            }
            TypeKind::Enum(id) => {
                let declaration = &module.enums[*id];
                self.types(declaration.type_arguments.iter().copied(), depth, meter)?;
                self.types(declaration.interfaces.iter().copied(), depth, meter)?;
                for variant in &declaration.variants {
                    meter.charge_work(1, &WirePath::root())?;
                    self.types(variant.fields.iter().map(|f| f.ty), depth, meter)?;
                }
                Ok(())
            }
            TypeKind::Interface(id) => {
                let declaration = &module.interfaces[*id];
                self.types(declaration.type_arguments.iter().copied(), depth, meter)?;
                for method in &declaration.methods {
                    self.types(method.params.iter().map(|p| p.ty), depth, meter)?;
                    self.add(method.return_ty, depth, meter)?;
                }
                Ok(())
            }
            TypeKind::Unit
            | TypeKind::Integer(_)
            | TypeKind::Boolean
            | TypeKind::String
            | TypeKind::Any => Ok(()),
        }
    }

    pub(super) fn function_type(
        &mut self,
        signature: FunctionTypeId,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        let signature = &self.module.function_types[signature];
        self.types(signature.parameter_types.iter().copied(), depth, meter)?;
        self.add(signature.return_type, depth, meter)
    }

    fn intrinsic(
        &mut self,
        representation: &IntrinsicTypeRepresentation,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        match representation {
            IntrinsicTypeRepresentation::Array { element }
            | IntrinsicTypeRepresentation::MutableArray { element }
            | IntrinsicTypeRepresentation::Ptr { pointee: element } => {
                self.add(*element, depth, meter)
            }
            IntrinsicTypeRepresentation::FunPtr { signature } => {
                self.function_type(*signature, depth, meter)
            }
            IntrinsicTypeRepresentation::Integer(_)
            | IntrinsicTypeRepresentation::Boolean
            | IntrinsicTypeRepresentation::String => Ok(()),
        }
    }
}
