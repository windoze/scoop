use super::*;

pub(super) fn nominal_kind_tag(
    kind: SourceDeclarationKind,
) -> Result<char, ExactTypeDiagnosticError> {
    match kind {
        SourceDeclarationKind::Class => Ok('C'),
        SourceDeclarationKind::Interface => Ok('I'),
        SourceDeclarationKind::Struct => Ok('S'),
        SourceDeclarationKind::Enum => Ok('E'),
        SourceDeclarationKind::Object => Ok('O'),
        SourceDeclarationKind::AnnotationClass => Ok('A'),
        _ => Err(ExactTypeDiagnosticError::NonNominalDeclaration),
    }
}

pub(super) fn nominal_name(
    declaration: &SourceDeclarationKey,
) -> Result<&crate::CanonicalIdentifier, ExactTypeDiagnosticError> {
    match declaration.name() {
        DeclarationName::Named(name) => Ok(name),
        DeclarationName::Constructor => Err(ExactTypeDiagnosticError::ConstructorUsedAsNominalName),
    }
}
