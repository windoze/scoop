//! Annotation records are checked once against the already validated source interface.

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError};
use scoop_hir::{
    AnnotationDeclarationV1, AnnotationTargetV1, CanonicalConstValueKindV1, IntrinsicTypeKind,
    NominalSourceShapeV1, SourceNominalId,
};
use scoop_identity::{IdentityReferenceError, PersistentAnnotationId, SourceDeclarationKey};
use std::collections::BTreeMap;
use std::fmt;

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
                    .is_some_and(|value| value.kind() != *kind)
                {
                    return Err(Error::Invalid(
                        "annotation default has a different scalar type",
                    ));
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
                    .any(|(kind, value)| *kind != value.kind())
                {
                    return Err(Error::Invalid(
                        "annotation argument has a different scalar type",
                    ));
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

    fn annotation_parameter_kinds(
        &self,
        declaration: &AnnotationDeclarationV1,
    ) -> Result<Vec<CanonicalConstValueKindV1>, Error> {
        declaration
            .parameters
            .iter()
            .map(|parameter| {
                let id = SourceNominalId::Concrete(parameter.value_type);
                let key = self.source_nominal_key(id).map_err(Error::Nominal)?;
                let nominal = self
                    .provider_interface(key.origin())
                    .map_err(Error::Nominal)?
                    .nominal_interfaces()
                    .declaration(id)
                    .ok_or(Error::Invalid(
                        "annotation parameter has no nominal declaration",
                    ))?;
                let NominalSourceShapeV1::Intrinsic(representation) = nominal.source_shape() else {
                    return Err(Error::Invalid(
                        "annotation parameter is not a scalar intrinsic type",
                    ));
                };
                match representation.family() {
                    IntrinsicTypeKind::Boolean => Ok(CanonicalConstValueKindV1::Boolean),
                    IntrinsicTypeKind::Integer(kind) => {
                        Ok(CanonicalConstValueKindV1::Integer(kind))
                    }
                    IntrinsicTypeKind::Char => Ok(CanonicalConstValueKindV1::Char),
                    IntrinsicTypeKind::String => Ok(CanonicalConstValueKindV1::String),
                    _ => Err(Error::Invalid(
                        "annotation parameter is not a scalar intrinsic type",
                    )),
                }
            })
            .collect()
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
