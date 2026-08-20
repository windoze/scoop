use scoop_identity::{CallableTemplateOrigin, PersistentGeneratedCallableId, SignatureTypeKey};

/// The actual typed identity carried by a nested callable descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableIdentityV1 {
    LocalFunction(CallableTemplateOrigin),
    Lambda(PersistentGeneratedCallableId),
    AnonymousFunction(PersistentGeneratedCallableId),
    CallableReference(PersistentGeneratedCallableId),
}

/// Body arguments retained by the complete descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableBodyArgumentsV1<'a> {
    Absent,
    Lexical,
    Explicit(&'a [SignatureTypeKey]),
}

/// A diagnostic position in the complete default body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultNestedCallableSiteV1 {
    Body { ordinal: u64 },
}
