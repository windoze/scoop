use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationRelation {
    Property,
    GlobalStorage,
    DelegateStorage,
    SingletonValue,
    SingletonPublishedRoot,
    Object,
    Companion,
    FailureRoot,
    Initializer,
    Ensure,
}

impl fmt::Display for InitializationRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Property => "property",
            Self::GlobalStorage => "global storage",
            Self::DelegateStorage => "delegate storage",
            Self::SingletonValue => "singleton value",
            Self::SingletonPublishedRoot => "singleton published root",
            Self::Object => "object",
            Self::Companion => "companion relation",
            Self::FailureRoot => "failure root",
            Self::Initializer => "initializer function",
            Self::Ensure => "ensure function",
        })
    }
}

#[derive(Debug)]
pub enum HirInitializationUnitIdentityError {
    UnknownReference {
        unit: u32,
        relation: InitializationRelation,
        target: u32,
    },
    DuplicateReference {
        unit: u32,
        relation: InitializationRelation,
        target: u32,
    },
    UnownedReference {
        relation: InitializationRelation,
        target: u32,
    },
    BackReference {
        unit: u32,
        relation: InitializationRelation,
        target: u32,
    },
    Schedule {
        unit: u32,
    },
    StorageRelation {
        unit: u32,
    },
    PropertyKind {
        unit: u32,
    },
    SingletonRelation {
        unit: u32,
    },
    CompanionRelation {
        unit: u32,
    },
    ObjectIdentity {
        unit: u32,
    },
    InvalidIdentity {
        unit: u32,
        error: scoop_wire::HashError,
    },
    DuplicatePersistentIdentity {
        unit: u32,
    },
}

impl HirInitializationUnitIdentityError {
    pub const fn unit(&self) -> Option<u32> {
        match self {
            Self::UnknownReference { unit, .. }
            | Self::DuplicateReference { unit, .. }
            | Self::BackReference { unit, .. }
            | Self::Schedule { unit }
            | Self::StorageRelation { unit }
            | Self::PropertyKind { unit }
            | Self::SingletonRelation { unit }
            | Self::CompanionRelation { unit }
            | Self::ObjectIdentity { unit }
            | Self::InvalidIdentity { unit, .. }
            | Self::DuplicatePersistentIdentity { unit } => Some(*unit),
            Self::UnownedReference { .. } => None,
        }
    }
}

impl fmt::Display for HirInitializationUnitIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReference {
                unit,
                relation,
                target,
            } => write!(
                formatter,
                "initialization unit {unit} refers to unknown {relation} {target}"
            ),
            Self::DuplicateReference {
                unit,
                relation,
                target,
            } => write!(
                formatter,
                "initialization unit {unit} reuses {relation} {target} owned by another unit or role"
            ),
            Self::UnownedReference { relation, target } => {
                write!(formatter, "{relation} {target} has no initialization unit")
            }
            Self::BackReference {
                unit,
                relation,
                target,
            } => write!(
                formatter,
                "{relation} {target} does not point back to initialization unit {unit}"
            ),
            Self::Schedule { unit } => write!(
                formatter,
                "initialization unit {unit} has a schedule incompatible with its typed kind"
            ),
            Self::StorageRelation { unit } => write!(
                formatter,
                "initialization unit {unit} has an inconsistent property storage relation"
            ),
            Self::PropertyKind { unit } => write!(
                formatter,
                "initialization unit {unit} is not owned by a top-level ordinary or extension property"
            ),
            Self::SingletonRelation { unit } => write!(
                formatter,
                "initialization unit {unit} has an inconsistent singleton relation"
            ),
            Self::CompanionRelation { unit } => write!(
                formatter,
                "initialization unit {unit} has an inconsistent companion relation"
            ),
            Self::ObjectIdentity { unit } => write!(
                formatter,
                "initialization unit {unit} does not have a concrete source object identity"
            ),
            Self::InvalidIdentity { unit, error } => write!(
                formatter,
                "initialization unit {unit} has an invalid persistent identity: {error}"
            ),
            Self::DuplicatePersistentIdentity { unit } => write!(
                formatter,
                "initialization unit {unit} duplicates another persistent identity"
            ),
        }
    }
}

impl std::error::Error for HirInitializationUnitIdentityError {}
