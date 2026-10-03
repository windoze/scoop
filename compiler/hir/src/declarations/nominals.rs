use super::*;

/// Lexical declaration owner of a static nested nominal. Each target keeps
/// its own nominal id; the owner relation is typed and never reconstructed
/// from a qualified source name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NominalOwner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
    Object(ObjectId),
}

mod classes;
mod constructors;
mod enums;
mod fields;
mod intrinsics;
mod methods;
mod queries;
mod structs;

pub use classes::*;
pub use constructors::*;
pub use enums::*;
pub use fields::*;
pub use intrinsics::*;
pub use methods::*;
pub use structs::*;

#[cfg(test)]
mod tests;
