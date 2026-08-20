use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSignatureTypeMappingError {
    UnknownType(u32),
    UnknownFunctionType(u32),
    UnknownApplication(u32),
    InvalidApplication(u32),
    InvalidFunctionType(u32),
    UnknownNominal(u32),
    RecursiveType(u32),
    UnknownBinder(u32),
    EmptyTuple,
    NominalArity { expected: usize, actual: usize },
    NominalIdentityKind,
    GeneratedNominal,
    DuplicateObjectBackingClass(u32),
    SuspendNativeFunctionPointer,
}

impl fmt::Display for HirSignatureTypeMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty}"),
            Self::UnknownFunctionType(function) => {
                write!(formatter, "unknown function type {function}")
            }
            Self::UnknownApplication(application) => {
                write!(formatter, "unknown nominal application {application}")
            }
            Self::InvalidApplication(ty) => write!(
                formatter,
                "type {ty} has an invalid nominal application canonical-type relation"
            ),
            Self::InvalidFunctionType(ty) => {
                write!(formatter, "type {ty} has an invalid function-type relation")
            }
            Self::UnknownNominal(owner) => write!(formatter, "unknown nominal owner {owner}"),
            Self::RecursiveType(ty) => write!(formatter, "type {ty} recursively contains itself"),
            Self::UnknownBinder(parameter) => write!(
                formatter,
                "type parameter {parameter} is outside the declaration binder"
            ),
            Self::EmptyTuple => formatter.write_str("a signature tuple must be non-empty"),
            Self::NominalArity { expected, actual } => write!(
                formatter,
                "nominal signature type has {actual} arguments, expected {expected}"
            ),
            Self::NominalIdentityKind => formatter
                .write_str("nominal declaration arity disagrees with its source identity kind"),
            Self::GeneratedNominal => formatter
                .write_str("a generated nominal cannot replace a source nominal in a signature"),
            Self::DuplicateObjectBackingClass(class) => {
                write!(formatter, "class {class} backs more than one source object")
            }
            Self::SuspendNativeFunctionPointer => {
                formatter.write_str("a native function pointer cannot be suspend")
            }
        }
    }
}

impl std::error::Error for HirSignatureTypeMappingError {}
