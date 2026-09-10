use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructorIdentityTable {
    Struct,
    Class,
}

impl fmt::Display for ConstructorIdentityTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Struct => "struct constructor",
            Self::Class => "class constructor",
        })
    }
}

#[derive(Debug)]
pub enum HirConstructorIdentityError {
    SourceIdentity(scoop_identity::SourceDeclarationIdentityError),
    GeneratedIdentity(scoop_identity::GeneratedCallableIdentityError),
    Length {
        table: ConstructorIdentityTable,
        expected: usize,
        actual: usize,
    },
    Ownership {
        table: ConstructorIdentityTable,
        owner: u32,
        constructor: u32,
    },
    Unowned {
        table: ConstructorIdentityTable,
        constructor: u32,
    },
    ObjectBackingClass {
        object: u32,
        class: u32,
    },
    GeneratedOwner,
    SourceDerivation {
        table: ConstructorIdentityTable,
        constructor: u32,
        error: super::HirSourceConstructorIdentityError,
    },
    SourceKey {
        table: ConstructorIdentityTable,
        constructor: u32,
    },
    IdentityKind {
        constructor: u32,
    },
    AdapterSource {
        adapter: u32,
        source: u32,
    },
    AdapterShape {
        adapter: u32,
    },
    AdapterTarget {
        adapter: u32,
    },
    GeneratedKey {
        adapter: u32,
    },
    DuplicateSourceIdentity {
        table: ConstructorIdentityTable,
        constructor: u32,
    },
    DuplicateGeneratedIdentity {
        constructor: u32,
    },
    DuplicateAdapter {
        source: u32,
    },
}

impl fmt::Display for HirConstructorIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceIdentity(error) => error.fmt(formatter),
            Self::GeneratedIdentity(error) => error.fmt(formatter),
            Self::Length {
                table,
                expected,
                actual,
            } => write!(
                formatter,
                "{table} identity table has {actual} entries, expected {expected}"
            ),
            Self::Ownership {
                table,
                owner,
                constructor,
            } => write!(
                formatter,
                "{table} {constructor} has an invalid or duplicate owner relation at nominal {owner}"
            ),
            Self::Unowned { table, constructor } => {
                write!(formatter, "{table} {constructor} has no nominal owner")
            }
            Self::ObjectBackingClass { object, class } => write!(
                formatter,
                "object {object} has invalid or duplicate backing class {class}"
            ),
            Self::GeneratedOwner => {
                formatter.write_str("a source constructor cannot be owned by a generated nominal")
            }
            Self::SourceDerivation {
                table,
                constructor,
                error,
            } => write!(
                formatter,
                "cannot derive {table} {constructor} identity: {error}"
            ),
            Self::SourceKey { table, constructor } => {
                write!(formatter, "{table} {constructor} has an invalid source key")
            }
            Self::IdentityKind { constructor } => write!(
                formatter,
                "class constructor {constructor} identity kind disagrees with its identity record"
            ),
            Self::AdapterSource { adapter, source } => write!(
                formatter,
                "class constructor adapter {adapter} has invalid source constructor {source}"
            ),
            Self::AdapterShape { adapter } => write!(
                formatter,
                "class constructor adapter {adapter} is not a zero-argument delegation with a non-empty source signature"
            ),
            Self::AdapterTarget { adapter } => write!(
                formatter,
                "class constructor adapter {adapter} has an invalid target application"
            ),
            Self::GeneratedKey { adapter } => write!(
                formatter,
                "class constructor adapter {adapter} has an invalid generated callable key"
            ),
            Self::DuplicateSourceIdentity { table, constructor } => write!(
                formatter,
                "{table} {constructor} duplicates another source constructor identity"
            ),
            Self::DuplicateGeneratedIdentity { constructor } => write!(
                formatter,
                "class constructor {constructor} duplicates another generated constructor identity"
            ),
            Self::DuplicateAdapter { source } => write!(
                formatter,
                "source class constructor {source} has more than one zero-argument adapter"
            ),
        }
    }
}

impl std::error::Error for HirConstructorIdentityError {}
