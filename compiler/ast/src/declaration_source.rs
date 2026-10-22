//! Borrowed source facts passed to declaration analysis after one grammar run.

use scoop_identity::{SourceIdentity, SourceRole};

use crate::{DeclarationInput, SourceHeaderRef};

#[derive(Debug)]
pub struct DeclarationSource<'syntax, 'input> {
    pub identity: &'input SourceIdentity,
    pub role: SourceRole,
    pub text: &'input str,
    pub header: Option<SourceHeaderRef<'syntax>>,
    pub declarations: Vec<DeclarationInput<'syntax>>,
    pub documentation: Option<&'syntax crate::SourceDocumentation>,
}
