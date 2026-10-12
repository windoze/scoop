use super::*;
use scoop_hir::{
    AnnotationParameterType, AnnotationTargetKind, AnnotationTypeSlot as Slot,
    CanonicalAnnotationsV1, IntrinsicTypeKind, NominalSourceShapeV1, SignatureBinderScopeV1,
    SourceNominalId,
};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn validate_type_annotations(
        &mut self,
        annotations: &CanonicalAnnotationsV1,
        targets: &BTreeSet<AnnotationTargetV1>,
        schemas: &mut values::Schemas,
    ) -> Result<(), Error> {
        for usage in annotations.type_uses() {
            if usage.identity.source.cone() != self.current {
                return Err(Error::Invalid(
                    "annotated type use has a foreign occurrence identity",
                ));
            }
            match &usage.ty {
                AnnotationParameterType::Value(ty) => {
                    SignatureBinderScopeV1::for_declaration(usage.binder_count, None)
                        .validate_signature_semantics(ty, self)
                        .map_err(|_| {
                            Error::Invalid("annotated type has an invalid signature or binder")
                        })?;
                }
                AnnotationParameterType::Annotation(id) => {
                    self.annotation_declaration(*id)?;
                }
                AnnotationParameterType::AnnotationArray { array, element } => {
                    self.annotation_declaration(*element)?;
                    if !matches!(self.annotation_nominal_shape(SourceNominalId::GenericTemplate(*array))?, NominalSourceShapeV1::Intrinsic(shape) if shape.family() == IntrinsicTypeKind::Array)
                    {
                        return Err(Error::Invalid(
                            "annotated annotation array requires core Array",
                        ));
                    }
                }
            }
            for node in &usage.nodes {
                if !scoop_hir::type_annotation_path_exists(&usage.ty, &node.path) {
                    return Err(Error::Invalid(
                        "type annotation path does not exist in its type use",
                    ));
                }
                let mut seen = BTreeSet::new();
                for application in &node.annotations {
                    let rules = self.annotation_declaration(application.annotation)?.rules;
                    if !rules.permits(AnnotationTargetKind::Type) {
                        return Err(Error::Invalid("annotation declaration excludes type uses"));
                    }
                    if !seen.insert(application.annotation) && !rules.repeatable {
                        return Err(Error::Invalid(
                            "repeated non-repeatable annotation on one type position",
                        ));
                    }
                    self.annotation_schema(application.annotation, schemas, &mut BTreeSet::new())?;
                    values::validate_arguments(
                        application.annotation,
                        &application.arguments,
                        schemas,
                    )?;
                }
            }
        }
        for site in annotations.type_sites() {
            if !targets.contains(&site.target)
                && !self
                    .current_foundation
                    .retains_local_annotation_target(site.target)
            {
                return Err(Error::Invalid(
                    "type annotation site has no retained declaration",
                ));
            }
            let usage = annotations
                .type_use(&site.type_use)
                .expect("the canonical table checked its use references");
            use AnnotationTargetV1 as Target;
            let valid = match (site.target, site.slot) {
                (
                    Target::Field(_)
                    | Target::VariantField(_)
                    | Target::Property(_)
                    | Target::ExtensionProperty(_)
                    | Target::Parameter { .. }
                    | Target::CallableReceiver(_)
                    | Target::PropertyReceiver(_)
                    | Target::TypeAlias(_)
                    | Target::PropertyStorage { .. },
                    Slot::Value,
                ) => true,
                (Target::Callable(_), Slot::Result) => true,
                (Target::Nominal(_), Slot::Supertype(_)) => true,
                (
                    Target::Nominal(_)
                    | Target::Callable(_)
                    | Target::TypeAlias(_)
                    | Target::Property(_)
                    | Target::ExtensionProperty(_),
                    Slot::Bound { parameter, .. },
                ) => parameter < usage.binder_count,
                (Target::Annotation(id), Slot::AnnotationParameter(index)) => annotations
                    .declaration(id)
                    .is_some_and(|declaration| (index as usize) < declaration.parameters.len()),
                _ => false,
            };
            if !valid {
                return Err(Error::Invalid(
                    "type annotation slot is not part of its declaration",
                ));
            }
        }
        Ok(())
    }
}
