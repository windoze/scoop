use super::*;

impl From<TypeFoundationBindingError> for Error {
    fn from(error: TypeFoundationBindingError) -> Self {
        NominalNestedBindingError::from(error).into()
    }
}
impl From<NominalSourceBindingError> for Error {
    fn from(error: NominalSourceBindingError) -> Self {
        NominalNestedBindingError::from(error).into()
    }
}
impl From<NominalMemberBindingError> for Error {
    fn from(error: NominalMemberBindingError) -> Self {
        NominalNestedBindingError::from(error).into()
    }
}
impl From<NominalConstructorBindingError> for Error {
    fn from(error: NominalConstructorBindingError) -> Self {
        NominalNestedBindingError::from(error).into()
    }
}
impl From<InheritanceSlotSourceBindingError> for Error {
    fn from(error: InheritanceSlotSourceBindingError) -> Self {
        Self::from_slot(error)
    }
}
impl From<ProtectedDeclarationBindingError> for Error {
    fn from(error: ProtectedDeclarationBindingError) -> Self {
        match error {
            ProtectedDeclarationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Protected(Box::new(other)),
        }
    }
}
