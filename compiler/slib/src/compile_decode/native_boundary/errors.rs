use super::*;

#[derive(Debug)]
pub enum NativeBoundaryCompileError {
    Identity(IdentityValidationError),
    Reference(scoop_identity::IdentityReferenceError),
    TypeDefinition(scoop_hir::NativeBoundaryDefinitionError),
    NominalProvider {
        owner: NativeBoundaryNominalOwner,
        declared: scoop_identity::ConeIdentity,
        provider: scoop_identity::ConeIdentity,
    },
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    ConflictingExactType {
        exact: PersistentExactTypeId,
    },
    ConflictingTypeWitness {
        owner: NativeBoundaryNominalOwner,
    },
    MissingCallableApplication {
        application: PersistentCallableApplicationId,
    },
    MissingInitializationUnit {
        unit: PersistentInitializationUnitId,
    },
    MissingExactType {
        exact: PersistentExactTypeId,
    },
    ClosureRequired {
        owner: NativeBoundaryNominalOwner,
    },
    UnrelatedDefinition {
        owner: NativeBoundaryNominalOwner,
    },
    Target(NativeBoundaryTargetError),
}

impl fmt::Display for NativeBoundaryCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Reference(error) => error.fmt(formatter),
            Self::TypeDefinition(error) => error.fmt(formatter),
            Self::NominalProvider {
                owner,
                declared,
                provider,
            } => write!(
                formatter,
                "ABI nominal {owner:?} belongs to {declared}, not its supplied provider {provider}"
            ),
            Self::Resource(error) => error.fmt(formatter),
            Self::Encoding(error) => error.fmt(formatter),
            Self::ConflictingExactType { exact } => {
                write!(formatter, "conflicting ABI exact type {exact}")
            }
            Self::ConflictingTypeWitness { owner } => {
                write!(formatter, "conflicting ABI source witness for {owner:?}")
            }
            Self::MissingCallableApplication { application } => write!(
                formatter,
                "native boundary references missing callable application {application}"
            ),
            Self::MissingInitializationUnit { unit } => write!(
                formatter,
                "native boundary references missing initialization application {unit}"
            ),
            Self::MissingExactType { exact } => {
                write!(
                    formatter,
                    "native boundary references missing exact type {exact}"
                )
            }
            Self::ClosureRequired { owner } => write!(
                formatter,
                "SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED: no source witness for {owner:?}"
            ),
            Self::UnrelatedDefinition { owner } => write!(
                formatter,
                "native boundary contains unrelated source witness for {owner:?}"
            ),
            Self::Target(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeBoundaryCompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Reference(error) => Some(error),
            Self::TypeDefinition(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::Target(error) => Some(error),
            _ => None,
        }
    }
}
