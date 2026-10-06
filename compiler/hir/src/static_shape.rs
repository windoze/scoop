//! Borrowed source shapes. Queries preserve declaration identities and never
//! materialize types, initialize objects, or recursively expand nominal fields.

use crate::*;

mod annotations;
mod constructors;
mod fields;
mod nominals;
mod properties;
mod type_use;
pub use annotations::*;
pub use constructors::*;
pub use fields::*;
pub use nominals::*;
pub use properties::*;
pub use type_use::*;

#[derive(Clone, Copy)]
pub enum StaticTypeShape<'a> {
    Unit,
    Any,
    Nominal(StaticNominalShape<'a>),
    Tuple(&'a [TypeId]),
    Function(&'a FunctionType),
    Pointer(TypeId),
    NativeFunctionPointer(&'a FunctionType),
    Parameter(&'a TypeParamDecl),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticShapeUnboundParameter(pub TypeParamId);

impl std::fmt::Display for StaticShapeUnboundParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "static shape requires the declaration of type parameter {:?}",
            self.0
        )
    }
}

impl std::error::Error for StaticShapeUnboundParameter {}

impl Module {
    /// Open types retain the actual declaration and its bounds. A parameter
    /// name or substitution position never substitutes for that identity.
    pub fn static_type_shape<'a>(
        &'a self,
        ty: TypeId,
        parameters: &'a [TypeParamDecl],
    ) -> Result<StaticTypeShape<'a>, StaticShapeUnboundParameter> {
        Ok(match &self.types[ty] {
            Type::Unit => StaticTypeShape::Unit,
            Type::Any => StaticTypeShape::Any,
            Type::Tuple(elements) => StaticTypeShape::Tuple(elements),
            Type::Function(function) => StaticTypeShape::Function(&self.function_types[*function]),
            Type::FunPtr(function) => {
                StaticTypeShape::NativeFunctionPointer(&self.function_types[*function])
            }
            Type::Ptr(pointee) => StaticTypeShape::Pointer(*pointee),
            Type::Param(parameter) => StaticTypeShape::Parameter(
                parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .ok_or(StaticShapeUnboundParameter(*parameter))?,
            ),
            Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Struct(_)
            | Type::Class(_)
            | Type::Enum(_)
            | Type::Interface(_) => StaticTypeShape::Nominal(StaticNominalShape::new(self, ty)),
        })
    }
}
