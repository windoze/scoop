use crate::{Type, TypeId};

/// Source applications retain the nominal owner independently from the
/// method's own arguments. A substitution vector is only a transient view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedCallableArguments {
    Function(Vec<TypeId>),
    Method {
        owner: TypeId,
        method_arguments: Vec<TypeId>,
    },
}

impl ImportedCallableArguments {
    pub fn substitution(
        &self,
        types: &la_arena::Arena<Type>,
        enums: &la_arena::Arena<crate::EnumApplication>,
        structs: &la_arena::Arena<crate::StructApplication>,
        classes: &la_arena::Arena<crate::ClassApplication>,
    ) -> Vec<TypeId> {
        match self {
            Self::Function(arguments) => arguments.clone(),
            Self::Method {
                owner,
                method_arguments,
            } => {
                let owner_arguments = match &types[*owner] {
                    Type::Ptr(pointee) => std::slice::from_ref(pointee),
                    Type::Enum(application) => &enums[*application].arguments,
                    Type::Struct(application) => &structs[*application].arguments,
                    Type::Class(application) => &classes[*application].arguments,
                    owner => {
                        owner
                            .imported_nominal_application()
                            .expect("an imported method retains its complete nominal owner")
                            .1
                    }
                };
                owner_arguments
                    .iter()
                    .chain(method_arguments)
                    .copied()
                    .collect()
            }
        }
    }

    pub fn map(&self, mut map: impl FnMut(TypeId) -> TypeId) -> Self {
        match self {
            Self::Function(arguments) => {
                Self::Function(arguments.iter().copied().map(map).collect())
            }
            Self::Method {
                owner,
                method_arguments,
            } => Self::Method {
                owner: map(*owner),
                method_arguments: method_arguments.iter().copied().map(map).collect(),
            },
        }
    }
}
