use crate::{ClassConstructorId, EnumVariantRef, FunctionId, StructConstructorId};

/// The source declaration owning a parameter, including local functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceParameterOwner {
    Function(FunctionId),
    StructConstructor(StructConstructorId),
    ClassConstructor(ClassConstructorId),
    VariantConstructor(EnumVariantRef),
}

/// A declaration parameter whose default is prepared in its definition scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SourceDefaultKey {
    pub owner: SourceParameterOwner,
    pub position: u32,
}

impl SourceDefaultKey {
    pub fn new(owner: SourceParameterOwner, position: usize) -> Self {
        Self {
            owner,
            position: u32::try_from(position).expect("parameter position fits u32"),
        }
    }

    pub const fn tuple(self) -> (SourceParameterOwner, u32) {
        (self.owner, self.position)
    }
}
