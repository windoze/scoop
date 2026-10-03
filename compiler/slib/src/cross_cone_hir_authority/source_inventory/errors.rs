use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceInventoryDeclaration {
    Nominal(SourceNominalId),
    Callable(CallableTemplateOrigin),
    Property(PropertyDeclarationId),
}

#[derive(Debug)]
pub enum CrossConeHirSourceInventoryError {
    Missing(SourceInventoryDeclaration),
}

impl std::fmt::Display for CrossConeHirSourceInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(id) => write!(f, "required shared source declaration is absent: {id:?}"),
        }
    }
}
impl std::error::Error for CrossConeHirSourceInventoryError {}
