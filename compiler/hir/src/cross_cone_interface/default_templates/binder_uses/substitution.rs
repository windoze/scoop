use scoop_identity::SignatureTypeKey;
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::{CanonicalBinderUseListV1, DefaultTemplateTypeSubstitutionError};
use crate::DefaultTemplateProviderShapeV1;

mod build;
#[cfg(test)]
mod tests;
mod walk;

enum Task<'a> {
    Visit {
        value: &'a SignatureTypeKey,
        substitute: bool,
    },
    Finish(&'a SignatureTypeKey),
}
struct Substitution<'a, 'm> {
    transform: Transform<'a>,
    tasks: Vec<Task<'a>>,
    values: Vec<SignatureTypeKey>,

    path: &'m WirePath,
}
#[derive(Clone, Copy)]
enum Transform<'a> {
    Copy,
    Substitute {
        mapping: &'a CanonicalBinderUseListV1,
        provider: DefaultTemplateProviderShapeV1,
    },
}

/// Copies a signature without using the Rust call stack for nested types.
pub fn copy_default_signature_type(
    signature: &SignatureTypeKey,

    path: &WirePath,
) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
    Substitution {
        transform: Transform::Copy,
        tasks: Vec::new(),
        values: Vec::new(),

        path,
    }
    .run(signature)
}
impl CanonicalBinderUseListV1 {
    /// Substitutes provider binders once in an explicit traversal.
    /// Mapping arguments already belong to the publishing owner's scope.
    pub fn substitute_provider_type(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        signature: &SignatureTypeKey,
    ) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
        let path = &WirePath::root();
        if self.len_u32() != provider.binder_arity() {
            return Err(DefaultTemplateTypeSubstitutionError::MappingArity {
                expected: provider.binder_arity(),
                actual: self.len_u32(),
            });
        }
        Substitution {
            transform: Transform::Substitute {
                mapping: self,
                provider,
            },
            tasks: Vec::new(),
            values: Vec::new(),

            path,
        }
        .run(signature)
    }
}
impl<'a> Substitution<'a, '_> {
    fn run(
        mut self,
        signature: &'a SignatureTypeKey,
    ) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
        scoop_wire::allocation::try_reserve(&mut self.tasks, 1, self.path)?;
        scoop_wire::allocation::try_reserve(&mut self.values, 1, self.path)?;
        self.tasks.push(Task::Visit {
            value: signature,
            substitute: true,
        });
        while let Some(task) = self.tasks.pop() {
            match task {
                Task::Visit { value, substitute } => self.visit(value, substitute)?,
                Task::Finish(value) => self.finish(value)?,
            }
        }
        if self.values.len() != 1 {
            return Err(invalid_length(1, self.values.len(), self.path).into());
        }
        self.values
            .pop()
            .ok_or_else(|| invalid_length(1, 0, self.path).into())
    }
}
fn invalid_length(expected: usize, actual: usize, path: &WirePath) -> WireError {
    WireError::new(
        WireErrorKind::InvalidLength {
            expected: expected as u64,
            actual: actual as u64,
        },
        path.clone(),
        None,
    )
}
fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
