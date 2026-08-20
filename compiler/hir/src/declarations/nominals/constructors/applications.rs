use super::*;

/// Storage locations preserve the selected constructor's semantic role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassConstructorDefinition {
    Local(ClassConstructorId),
    Template(ImportedConstructorTemplateId),
}

impl From<ClassConstructorId> for ClassConstructorDefinition {
    fn from(value: ClassConstructorId) -> Self {
        Self::Local(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructConstructorDefinition {
    Local(StructConstructorId),
    Template(ImportedConstructorTemplateId),
}

impl From<StructConstructorId> for StructConstructorDefinition {
    fn from(value: StructConstructorId) -> Self {
        Self::Local(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructorApplication {
    pub constructor: ClassConstructorDefinition,
    pub owner: ClassApplicationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructConstructorApplication {
    pub constructor: StructConstructorDefinition,
    pub owner: StructApplicationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorApplicationRef {
    Class(ClassConstructorApplicationId),
    Struct(StructConstructorApplicationId),
}
