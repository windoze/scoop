//! Annotation records are checked once against the already validated source interface.

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError};
use scoop_hir::{AnnotationDeclarationV1, AnnotationTargetV1};
use scoop_identity::{IdentityReferenceError, PersistentAnnotationId, SourceDeclarationKey};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

mod type_uses;
mod types;
mod values;

type Error = CrossConeHirAnnotationError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_annotations(&mut self) -> Result<(), Error> {
        let interface = self.current_interface;
        let annotations = interface.annotations();
        let mut parameter_kinds = BTreeMap::new();
        for declaration in annotations.declarations() {
            let record = self
                .current_foundation
                .annotation(declaration.annotation)
                .ok_or(Error::Invalid(
                    "annotation declaration has no current source key",
                ))?;
            let source = declaration.definition_origin.origin().source();
            if record.key().origin() != self.current
                || source.cone() != self.current
                || record
                    .key()
                    .scope()
                    .source()
                    .is_some_and(|expected| expected != source)
            {
                return Err(Error::Invalid(
                    "annotation definition source differs from its declaration",
                ));
            }
            self.annotation_schema(
                declaration.annotation,
                &mut parameter_kinds,
                &mut Default::default(),
            )?;
        }
        let targets = AnnotationTargetV1::source_targets(
            self.current_interface.nominal_interfaces(),
            self.current_interface.property_interfaces(),
            self.current_interface.callable_interfaces(),
            self.current_interface.type_aliases(),
            annotations.declarations(),
        );
        for target in annotations.targets() {
            if !targets.contains(&target.target)
                && !self
                    .current_foundation
                    .retains_local_annotation_target(target.target)
            {
                return Err(Error::Invalid(
                    "annotation target is not a retained source declaration",
                ));
            }
            let kind = target
                .target
                .kind(self.current_interface.property_interfaces())
                .ok_or(Error::Invalid("annotation accessor target has no property"))?;
            let mut seen = BTreeSet::new();
            for application in &target.annotations {
                let rules = self.annotation_declaration(application.annotation)?.rules;
                if !rules.permits(kind) {
                    return Err(Error::Invalid(
                        "annotation target is excluded by its declaration",
                    ));
                }
                if !seen.insert(application.annotation) && !rules.repeatable {
                    return Err(Error::Invalid(
                        "repeated non-repeatable annotation on one target",
                    ));
                }
                self.annotation_schema(
                    application.annotation,
                    &mut parameter_kinds,
                    &mut Default::default(),
                )?;
                values::validate_arguments(
                    application.annotation,
                    &application.arguments,
                    &parameter_kinds,
                )?;
                if application.definition_origin.origin().source().cone() != self.current {
                    return Err(Error::Invalid(
                        "annotation application has a foreign source origin",
                    ));
                }
            }
        }
        self.validate_type_annotations(annotations, &targets, &mut parameter_kinds)
    }

    fn annotation_declaration(
        &self,
        id: PersistentAnnotationId,
    ) -> Result<&AnnotationDeclarationV1, Error> {
        let key = self
            .identities
            .canonical_key::<PersistentAnnotationId, SourceDeclarationKey>(id)
            .map_err(Error::Identity)?;
        self.provider_interface(key.origin())
            .map_err(Error::Nominal)?
            .annotations()
            .declaration(id)
            .ok_or(Error::Invalid("annotation reference has no declaration"))
    }
}

#[derive(Debug)]
pub enum CrossConeHirAnnotationError {
    Invalid(&'static str),
    Identity(IdentityReferenceError),
    Nominal(CrossConeHirNominalAuthorityError),
}
impl fmt::Display for CrossConeHirAnnotationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Identity(error) => error.fmt(formatter),
            Self::Nominal(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for CrossConeHirAnnotationError {}
