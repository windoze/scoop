//! Direct data dependencies of a concrete type, before or after module assembly.

use super::*;

pub struct ConcreteTypeRelations<'a> {
    pub types: &'a Arena<Type>,
    pub function_types: &'a Arena<FunctionType>,
    pub structs: &'a Arena<StructDef>,
    pub enums: &'a Arena<EnumDef>,
    pub classes: &'a Arena<ClassDef>,
    pub interfaces: &'a Arena<InterfaceDef>,
}

impl<'a> ConcreteTypeRelations<'a> {
    pub fn from_module(module: &'a Module) -> Self {
        Self {
            types: &module.types,
            function_types: &module.function_types,
            structs: &module.structs,
            enums: &module.enums,
            classes: &module.classes,
            interfaces: &module.interfaces,
        }
    }

    pub fn visit_children<E>(
        &self,
        ty: TypeId,
        visit: &mut impl FnMut(TypeId) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.types[ty].kind {
            TypeKind::Tuple(elements) => visit_types(elements.iter().copied(), visit),
            TypeKind::Ptr(pointee) => visit(*pointee),
            TypeKind::Function(signature) | TypeKind::FunPtr(signature) => {
                self.visit_function(*signature, visit)
            }
            TypeKind::Struct(id) => {
                let declaration = &self.structs[*id];
                visit_types(declaration.type_arguments.iter().copied(), visit)?;
                visit_types(declaration.interfaces.iter().copied(), visit)?;
                match &declaration.representation {
                    StructRepresentation::Declared { fields, .. } => {
                        visit_types(fields.iter().map(|field| field.ty), visit)
                    }
                    StructRepresentation::Intrinsic { application, .. } => {
                        self.visit_intrinsic(application, visit)
                    }
                }
            }
            TypeKind::Class(id) => {
                let declaration = &self.classes[*id];
                visit_types(declaration.type_arguments.iter().copied(), visit)?;
                visit_types(declaration.interfaces.iter().copied(), visit)?;
                match &declaration.representation {
                    ClassRepresentation::Declared { fields, base_class } => {
                        visit_types(fields.iter().map(|field| field.ty), visit)?;
                        visit_types(base_class.map(|id| self.classes[id].canonical_type), visit)
                    }
                    ClassRepresentation::Intrinsic { application, .. } => {
                        self.visit_intrinsic(application, visit)
                    }
                }
            }
            TypeKind::Enum(id) => {
                let declaration = &self.enums[*id];
                visit_types(declaration.type_arguments.iter().copied(), visit)?;
                visit_types(declaration.interfaces.iter().copied(), visit)?;
                for variant in &declaration.variants {
                    visit_types(variant.fields.iter().map(|field| field.ty), visit)?;
                }
                Ok(())
            }
            TypeKind::Interface(id) => {
                let declaration = &self.interfaces[*id];
                visit_types(declaration.parents.iter().copied(), visit)?;
                visit_types(declaration.type_arguments.iter().copied(), visit)?;
                for method in &declaration.methods {
                    visit_types(method.params.iter().map(|parameter| parameter.ty), visit)?;
                    visit(method.return_ty)?;
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

    pub fn visit_function<E>(
        &self,
        signature: FunctionTypeId,
        visit: &mut impl FnMut(TypeId) -> Result<(), E>,
    ) -> Result<(), E> {
        let signature = &self.function_types[signature];
        visit_types(signature.parameter_types.iter().copied(), visit)?;
        visit(signature.return_type)
    }

    fn visit_intrinsic<E>(
        &self,
        representation: &IntrinsicTypeRepresentation,
        visit: &mut impl FnMut(TypeId) -> Result<(), E>,
    ) -> Result<(), E> {
        match representation {
            IntrinsicTypeRepresentation::Array { element }
            | IntrinsicTypeRepresentation::MutableArray { element }
            | IntrinsicTypeRepresentation::Ptr { pointee: element } => visit(*element),
            IntrinsicTypeRepresentation::FunPtr { signature } => {
                self.visit_function(*signature, visit)
            }
            IntrinsicTypeRepresentation::Unit
            | IntrinsicTypeRepresentation::Integer(_)
            | IntrinsicTypeRepresentation::Float(_)
            | IntrinsicTypeRepresentation::Char
            | IntrinsicTypeRepresentation::Boolean
            | IntrinsicTypeRepresentation::String => Ok(()),
        }
    }
}

fn visit_types<E>(
    types: impl IntoIterator<Item = TypeId>,
    visit: &mut impl FnMut(TypeId) -> Result<(), E>,
) -> Result<(), E> {
    for ty in types {
        visit(ty)?;
    }
    Ok(())
}
