/// Position in an existing source declaration; no declaration is synthesized.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnnotationTypeSlot {
    Value,
    Result,
    Supertype(u32),
    Bound { parameter: u32, index: u32 },
    AnnotationParameter(u32),
}
