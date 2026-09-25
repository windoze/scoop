use super::*;

impl Collector<'_> {
    pub(super) fn children(&mut self, ty: TypeId) -> Result<(), MaterializedTypeClosureError> {
        let module = self.module;
        match &module.types[ty].kind {
            TypeKind::Tuple(elements) => self.types(elements.iter().copied()),
            TypeKind::Ptr(pointee) => self.add(*pointee),
            TypeKind::Function(signature) | TypeKind::FunPtr(signature) => {
                self.function_type(*signature)
            }
            TypeKind::Struct(id) => {
                let declaration = &module.structs[*id];
                self.types(declaration.type_arguments.iter().copied())?;
                self.types(declaration.interfaces.iter().copied())?;
                match &declaration.representation {
                    StructRepresentation::Declared { fields, .. } => {
                        self.types(fields.iter().map(|f| f.ty))
                    }
                    StructRepresentation::Intrinsic { application, .. } => {
                        self.intrinsic(application)
                    }
                }
            }
            TypeKind::Class(id) => {
                let declaration = &module.classes[*id];
                self.types(declaration.type_arguments.iter().copied())?;
                self.types(declaration.interfaces.iter().copied())?;
                match &declaration.representation {
                    ClassRepresentation::Declared { fields, base_class } => {
                        self.types(fields.iter().map(|f| f.ty))?;
                        self.types(base_class.map(|id| module.classes[id].canonical_type))
                    }
                    ClassRepresentation::Intrinsic { application, .. } => {
                        self.intrinsic(application)
                    }
                }
            }
            TypeKind::Enum(id) => {
                let declaration = &module.enums[*id];
                self.types(declaration.type_arguments.iter().copied())?;
                self.types(declaration.interfaces.iter().copied())?;
                for variant in &declaration.variants {
                    self.types(variant.fields.iter().map(|f| f.ty))?;
                }
                Ok(())
            }
            TypeKind::Interface(id) => {
                let declaration = &module.interfaces[*id];
                self.types(declaration.type_arguments.iter().copied())?;
                for method in &declaration.methods {
                    self.types(method.params.iter().map(|p| p.ty))?;
                    self.add(method.return_ty)?;
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
    ) -> Result<(), MaterializedTypeClosureError> {
        let signature = &self.module.function_types[signature];
        self.types(signature.parameter_types.iter().copied())?;
        self.add(signature.return_type)
    }

    fn intrinsic(
        &mut self,
        representation: &IntrinsicTypeRepresentation,
    ) -> Result<(), MaterializedTypeClosureError> {
        match representation {
            IntrinsicTypeRepresentation::Array { element }
            | IntrinsicTypeRepresentation::MutableArray { element }
            | IntrinsicTypeRepresentation::Ptr { pointee: element } => self.add(*element),
            IntrinsicTypeRepresentation::FunPtr { signature } => self.function_type(*signature),
            IntrinsicTypeRepresentation::Integer(_)
            | IntrinsicTypeRepresentation::Boolean
            | IntrinsicTypeRepresentation::String => Ok(()),
        }
    }
}
