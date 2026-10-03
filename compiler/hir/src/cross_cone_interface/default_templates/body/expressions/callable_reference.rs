use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, PersistentGeneratedCallableId, SignatureTypeKey,
    StructuralDefinitionPath,
};

use super::DefaultExpressionV1;
use crate::{DefaultCallableRefV1, DefaultCaptureV1, DefaultMethodCalleeV1};

mod decoded;
mod indexed;

pub use decoded::{
    DecodedDefaultCallableReferenceTargetV1, DecodedDefaultCallableReferenceV1,
    DefaultCallableReferenceResolutionError,
};
pub use indexed::{DefaultCallableReferenceIndexError, IndexedDefaultCallableReferenceV1};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCallableReferenceV1 {
    invoke: PersistentGeneratedCallableId,
    definition_path: StructuralDefinitionPath,
    target: DefaultCallableReferenceTargetV1,
    function_type: SignatureTypeKey,
    captures: Vec<DefaultCaptureV1>,
    capture_count: u32,
    owner_type_parameter_count: u32,
}

impl DefaultCallableReferenceV1 {
    pub fn try_new(
        invoke: PersistentGeneratedCallableId,
        definition_path: StructuralDefinitionPath,
        target: DefaultCallableReferenceTargetV1,
        function_type: SignatureTypeKey,
        captures: Vec<DefaultCaptureV1>,
        owner_type_parameter_count: u32,
    ) -> Result<Self, DefaultCallableReferenceBuildError> {
        validate_target(&target)?;
        let capture_count = u32::try_from(captures.len())
            .map_err(|_| DefaultCallableReferenceBuildError::TooManyCaptures)?;
        Ok(Self {
            invoke,
            definition_path,
            target,
            function_type,
            captures,
            capture_count,
            owner_type_parameter_count,
        })
    }

    pub const fn invoke(&self) -> PersistentGeneratedCallableId {
        self.invoke
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.definition_path
    }

    pub const fn target(&self) -> &DefaultCallableReferenceTargetV1 {
        &self.target
    }

    pub const fn function_type(&self) -> &SignatureTypeKey {
        &self.function_type
    }

    pub fn captures(&self) -> &[DefaultCaptureV1] {
        &self.captures
    }

    pub const fn capture_count(&self) -> u32 {
        self.capture_count
    }

    pub const fn owner_type_parameter_count(&self) -> u32 {
        self.owner_type_parameter_count
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultCallableReferenceTargetV1 {
    Named(DefaultCallableRefV1),
    Local {
        declaration: CallableTemplateOrigin,
        callee: DefaultCallableRefV1,
    },
    BoundMember {
        receiver: Box<DefaultExpressionV1>,
        callee: DefaultMethodCalleeV1,
    },
    BoundExtension {
        receiver: Box<DefaultExpressionV1>,
        callee: DefaultCallableRefV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultCallableReferenceBuildError {
    TooManyCaptures,
    UnsupportedLocalDeclaration(CallableTemplateOrigin),
}

impl fmt::Display for DefaultCallableReferenceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCaptures => {
                formatter.write_str("default callable-reference capture count exceeds u32")
            }
            Self::UnsupportedLocalDeclaration(declaration) => write!(
                formatter,
                "default callable reference uses unsupported local declaration {declaration:?}"
            ),
        }
    }
}

impl std::error::Error for DefaultCallableReferenceBuildError {}

fn validate_target(
    target: &DefaultCallableReferenceTargetV1,
) -> Result<(), DefaultCallableReferenceBuildError> {
    if let DefaultCallableReferenceTargetV1::Local { declaration, .. } = target
        && !matches!(
            declaration,
            CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
        )
    {
        return Err(DefaultCallableReferenceBuildError::UnsupportedLocalDeclaration(*declaration));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
