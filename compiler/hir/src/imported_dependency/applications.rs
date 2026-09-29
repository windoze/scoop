use crate::{NonEmptyVec, Type, TypeId};

/// Source applications retain the nominal owner independently from the
/// method's own arguments. A substitution vector is only a transient view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedCallableArguments {
    Function(NonEmptyVec<TypeId>),
    Method {
        owner: TypeId,
        method_arguments: Vec<TypeId>,
    },
}

impl ImportedCallableArguments {
    pub fn substitution(&self, types: &la_arena::Arena<Type>) -> Vec<TypeId> {
        match self {
            Self::Function(arguments) => arguments.iter().copied().collect(),
            Self::Method {
                owner,
                method_arguments,
            } => {
                let owner_arguments = match &types[*owner] {
                    Type::Ptr(pointee) => std::slice::from_ref(pointee),
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
            Self::Function(arguments) => Self::Function(
                NonEmptyVec::from_vec(arguments.iter().copied().map(map).collect())
                    .expect("substitution preserves callable argument arity"),
            ),
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
