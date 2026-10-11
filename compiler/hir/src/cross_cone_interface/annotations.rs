use std::collections::BTreeSet;
use std::fmt;

use crate::{DeclaredVisibilityV1, ExportDefinitionSourceV1};
use scoop_identity::{CanonicalIdentifier, PersistentAnnotationId, SignatureTypeKey};

mod parameter;
mod references;
mod source_targets;
pub use parameter::AnnotationParameterType;
mod target;
mod value;
mod wire;
pub use target::{AnnotationCallableV1, AnnotationTargetV1, DecodedAnnotationTargetV1};
pub use value::{
    AnnotationElementTypeV1, AnnotationElementV1, CanonicalAnnotationValueV1,
    DecodedAnnotationValueV1,
};
pub use wire::{
    AnnotationDataResolutionError, AnnotationDataResolver, DecodedCanonicalAnnotationsV1,
};

/// Parameters retain their complete type after source aliases are expanded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationParameterV1 {
    pub name: CanonicalIdentifier,
    pub value_type: AnnotationParameterType<SignatureTypeKey>,
    pub default: Option<CanonicalAnnotationValueV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationDeclarationV1 {
    pub annotation: PersistentAnnotationId,
    pub parameters: Vec<AnnotationParameterV1>,
    pub rules: crate::AnnotationRules,
    pub visibility: DeclaredVisibilityV1,
    pub definition_origin: ExportDefinitionSourceV1,
}

/// Arguments are complete and in declaration order, including defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationApplicationV1 {
    pub annotation: PersistentAnnotationId,
    pub arguments: Vec<CanonicalAnnotationValueV1>,
    pub definition_origin: ExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotatedTargetV1 {
    pub target: AnnotationTargetV1,
    pub annotations: Vec<AnnotationApplicationV1>,
}

/// Static annotation data refers to the original source entities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalAnnotationsV1 {
    declarations: Vec<AnnotationDeclarationV1>,
    targets: Vec<AnnotatedTargetV1>,
}

impl CanonicalAnnotationsV1 {
    pub const fn empty() -> Self {
        Self {
            declarations: Vec::new(),
            targets: Vec::new(),
        }
    }

    pub fn try_new(
        mut declarations: Vec<AnnotationDeclarationV1>,
        mut targets: Vec<AnnotatedTargetV1>,
    ) -> Result<Self, AnnotationDataBuildError> {
        declarations.sort_by_key(|declaration| declaration.annotation);
        targets.sort_by_key(|target| target.target);
        if declarations
            .windows(2)
            .any(|pair| pair[0].annotation == pair[1].annotation)
        {
            return Err(AnnotationDataBuildError("duplicate annotation declaration"));
        }
        if targets
            .windows(2)
            .any(|pair| pair[0].target == pair[1].target)
        {
            return Err(AnnotationDataBuildError("duplicate annotation target"));
        }
        for declaration in &declarations {
            let mut names = BTreeSet::new();
            if declaration
                .parameters
                .iter()
                .any(|parameter| !names.insert(&parameter.name))
            {
                return Err(AnnotationDataBuildError("duplicate annotation parameter"));
            }
        }
        for target in &targets {
            if target.annotations.is_empty() {
                return Err(AnnotationDataBuildError("empty annotation target"));
            }
        }
        Ok(Self {
            declarations,
            targets,
        })
    }

    pub fn declarations(&self) -> &[AnnotationDeclarationV1] {
        &self.declarations
    }

    pub fn targets(&self) -> &[AnnotatedTargetV1] {
        &self.targets
    }

    pub fn declaration(
        &self,
        annotation: PersistentAnnotationId,
    ) -> Option<&AnnotationDeclarationV1> {
        self.declarations
            .binary_search_by_key(&annotation, |entry| entry.annotation)
            .ok()
            .map(|index| &self.declarations[index])
    }

    pub fn annotations(&self, target: AnnotationTargetV1) -> &[AnnotationApplicationV1] {
        self.targets
            .binary_search_by_key(&target, |entry| entry.target)
            .ok()
            .map_or(&[], |index| &self.targets[index].annotations)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationDataBuildError(pub &'static str);

impl fmt::Display for AnnotationDataBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
impl std::error::Error for AnnotationDataBuildError {}
