use super::*;
use scoop_hir::CanonicalAnnotationValueV1;
use std::collections::BTreeSet;
use types::AnnotationParameterKind;

pub(super) type Schemas = BTreeMap<PersistentAnnotationId, Vec<AnnotationParameterKind>>;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn annotation_schema(
        &self,
        id: PersistentAnnotationId,
        schemas: &mut Schemas,
        active: &mut BTreeSet<PersistentAnnotationId>,
    ) -> Result<(), Error> {
        if active.contains(&id) {
            return Err(Error::Invalid("recursive annotation schema"));
        }
        if schemas.contains_key(&id) {
            return Ok(());
        }
        active.insert(id);
        let declaration = self.annotation_declaration(id)?;
        let kinds = self.annotation_parameter_kinds(declaration)?;
        for kind in &kinds {
            if let Some(nested) = kind.nested() {
                self.annotation_schema(nested, schemas, active)?;
            }
        }
        for (parameter, kind) in declaration.parameters.iter().zip(&kinds) {
            if let Some(value) = &parameter.default {
                validate_value(
                    kind,
                    value,
                    schemas,
                    "annotation default has a different type",
                )?;
            }
        }
        active.remove(&id);
        schemas.insert(id, kinds);
        Ok(())
    }
}

pub(super) fn validate_arguments(
    id: PersistentAnnotationId,
    arguments: &[CanonicalAnnotationValueV1],
    schemas: &Schemas,
) -> Result<(), Error> {
    let kinds = schemas
        .get(&id)
        .ok_or(Error::Invalid("annotation application has no schema"))?;
    if kinds.len() != arguments.len() {
        return Err(Error::Invalid(
            "annotation argument count differs from its declaration",
        ));
    }
    for (kind, value) in kinds.iter().zip(arguments) {
        validate_value(
            kind,
            value,
            schemas,
            "annotation argument has a different type",
        )?;
    }
    Ok(())
}

fn validate_value(
    kind: &AnnotationParameterKind,
    value: &CanonicalAnnotationValueV1,
    schemas: &Schemas,
    mismatch: &'static str,
) -> Result<(), Error> {
    if !kind.accepts(value) {
        return Err(Error::Invalid(mismatch));
    }
    match value {
        CanonicalAnnotationValueV1::Annotation {
            annotation,
            arguments,
        } => validate_arguments(*annotation, arguments, schemas),
        CanonicalAnnotationValueV1::Array { elements, .. } => {
            for element in elements {
                if let CanonicalAnnotationValueV1::Annotation {
                    annotation,
                    arguments,
                } = &element.value
                {
                    validate_arguments(*annotation, arguments, schemas)?;
                }
            }
            Ok(())
        }
        CanonicalAnnotationValueV1::Scalar(_) | CanonicalAnnotationValueV1::IntegralEnum { .. } => {
            Ok(())
        }
    }
}
