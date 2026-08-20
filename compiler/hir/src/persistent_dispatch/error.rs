use std::fmt;

use scoop_identity::PersistentDispatchSlotId;

#[derive(Debug)]
pub enum HirDispatchSlotIdentityError {
    InterfaceLength { expected: usize, actual: usize },
    DuplicateVirtualFamily { family: u32 },
    UnknownVirtualFamily { function: u32, family: u32 },
    UnknownVirtualRoot { family: u32, function: u32 },
    VirtualRootDispatch { family: u32, function: u32 },
    VirtualMemberRole { function: u32, family: u32 },
    VirtualIdentity { family: u32 },
    UnreferencedVirtualFamily { family: u32 },
    UnknownInterfaceFunction { member: u32, function: u32 },
    InterfaceDispatch { member: u32, function: u32 },
    InterfaceRole { member: u32, function: u32 },
    InterfaceIdentity { member: u32 },
    InvalidDeclarationIdentity { function: u32 },
    DuplicatePersistentIdentity { identity: PersistentDispatchSlotId },
}

impl fmt::Display for HirDispatchSlotIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InterfaceLength { expected, actual } => write!(
                formatter,
                "interface dispatch identity table has {actual} entries, expected {expected}"
            ),
            Self::DuplicateVirtualFamily { family } => {
                write!(formatter, "virtual dispatch family {family} is duplicated")
            }
            Self::UnknownVirtualFamily { function, family } => write!(
                formatter,
                "function {function} references unknown virtual dispatch family {family}"
            ),
            Self::UnknownVirtualRoot { family, function } => write!(
                formatter,
                "virtual dispatch family {family} references unknown root function {function}"
            ),
            Self::VirtualRootDispatch { family, function } => write!(
                formatter,
                "function {function} is not the root of virtual dispatch family {family}"
            ),
            Self::VirtualMemberRole { function, family } => write!(
                formatter,
                "function {function} has the wrong declaration role for virtual dispatch family {family}"
            ),
            Self::VirtualIdentity { family } => write!(
                formatter,
                "virtual dispatch family {family} has the wrong persistent identity"
            ),
            Self::UnreferencedVirtualFamily { family } => write!(
                formatter,
                "virtual dispatch family {family} has no function member"
            ),
            Self::UnknownInterfaceFunction { member, function } => write!(
                formatter,
                "interface member {member} references unknown function {function}"
            ),
            Self::InterfaceDispatch { member, function } => write!(
                formatter,
                "function {function} does not dispatch through interface member {member}"
            ),
            Self::InterfaceRole { member, function } => write!(
                formatter,
                "interface member {member} role disagrees with function {function} identity"
            ),
            Self::InterfaceIdentity { member } => write!(
                formatter,
                "interface member {member} has the wrong persistent dispatch identity"
            ),
            Self::InvalidDeclarationIdentity { function } => write!(
                formatter,
                "function {function} cannot own a persistent dispatch slot"
            ),
            Self::DuplicatePersistentIdentity { identity } => write!(
                formatter,
                "persistent dispatch slot identity {identity} is duplicated"
            ),
        }
    }
}

impl std::error::Error for HirDispatchSlotIdentityError {}
