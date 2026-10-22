use super::*;

/// Source positions and declaration candidates for an unfinished call.
#[derive(Clone, Debug)]
pub struct EditingIncompleteCall {
    pub span: Span,
    pub name: Option<String>,
    pub opening: Span,
    pub commas: Vec<Span>,
    pub arguments_end: u32,
    pub closed: bool,
    pub argument_spans: Vec<Span>,
    pub trailing: Option<Span>,
    pub candidates: EditingValue<Vec<EditingCallCandidate>>,
}

/// A candidate's signature display and actual supplied argument bindings.
/// Type spellings are presentation data, never a substitute for type identity.
#[derive(Clone, Debug)]
pub struct EditingCallCandidate {
    pub parameters: Vec<EditingCandidateParameter>,
    pub result: EditingValue<String>,
    pub source_parameters: Vec<usize>,
    pub missing_parameters: Vec<usize>,
    pub unchecked_arguments: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct EditingCandidateParameter {
    pub name: Option<String>,
    pub type_label: String,
    pub vararg: bool,
    pub has_default: bool,
}
