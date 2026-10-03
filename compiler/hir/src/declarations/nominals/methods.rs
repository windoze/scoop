use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// Effective dispatch modality of a member function (spec 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

/// Member-only function metadata. Keeping owner and modality together
/// makes it impossible for a method to reach downstream stages without
/// a dispatch modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
    /// Complete source-level dispatch identity. Overrides share a typed
    /// virtual family; interface declarations name their exact member.
    pub dispatch: MethodDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodDispatch {
    Direct,
    Virtual(VirtualMethodId),
    /// A final override is called directly through its own static type but
    /// still replaces the inherited virtual-family slot for base-typed calls.
    FinalOverride(VirtualMethodId),
    Interface(InterfaceMethodId),
}

/// Closed language-level identity assigned to a successfully validated
/// `operator` declaration. Consumers match this role directly and never
/// recover it from a source name, nominal owner or intrinsic symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    UnaryPlus,
    UnaryMinus,
    Not,
    Inc,
    Dec,
    Plus,
    Minus,
    Times,
    Div,
    Rem,
    RangeTo,
    RangeUntil,
    Contains,
    Get,
    Set,
    Invoke,
    PlusAssign,
    MinusAssign,
    TimesAssign,
    DivAssign,
    RemAssign,
    CompareTo,
    Equals,
    Component { index: std::num::NonZeroU32 },
    Iterator,
}
