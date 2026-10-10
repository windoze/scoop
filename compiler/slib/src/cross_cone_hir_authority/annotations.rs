//! Annotation records are checked once against the already validated source interface.

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError};
use scoop_hir::{AnnotationDeclarationV1, AnnotationTargetV1};
use scoop_identity::{IdentityReferenceError, PersistentAnnotationId, SourceDeclarationKey};
use std::collections::BTreeMap;
use std::fmt;

mod types;

type Error = CrossConeHirAnnotationError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_annotations(&self) -> Result<(), Error> {
        let annotations = self.current_interface.annotations();
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
            let kinds = self.annotation_parameter_kinds(declaration)?;
            for (parameter, kind) in declaration.parameters.iter().zip(&kinds) {
                if parameter
                    .default
                    .as_ref()
                    .is_some_and(|value| !kind.accepts(value))
                {
                    return Err(Error::Invalid("annotation default has a different type"));
                }
            }
            parameter_kinds.insert(declaration.annotation, kinds);
        }
        let targets = AnnotationTargetV1::source_targets(
            self.current_interface.nominal_interfaces(),
            self.current_interface.property_interfaces(),
        );
        for target in annotations.targets() {
            if !targets.contains(&target.target) {
                return Err(Error::Invalid(
                    "annotation target is not a retained source declaration",
                ));
            }
            for application in &target.annotations {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    parameter_kinds.entry(application.annotation)
                {
                    let declaration = self.annotation_declaration(application.annotation)?;
                    entry.insert(self.annotation_parameter_kinds(declaration)?);
                }
                let kinds = &parameter_kinds[&application.annotation];
                if kinds.len() != application.arguments.len() {
                    return Err(Error::Invalid(
                        "annotation argument count differs from its declaration",
                    ));
                }
                if kinds
                    .iter()
                    .zip(&application.arguments)
                    .any(|(kind, value)| !kind.accepts(value))
                {
                    return Err(Error::Invalid("annotation argument has a different type"));
                }
                if application.definition_origin.origin().source().cone() != self.current {
                    return Err(Error::Invalid(
                        "annotation application has a foreign source origin",
                    ));
                }
            }
        }
        Ok(())
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
