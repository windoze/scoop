use super::{CanonicalCrossConeHirSurfaceAuthority, Error};
use scoop_hir::{
    AnnotationDeclarationV1, CanonicalAnnotationValueV1, CanonicalConstValueKindV1,
    IntrinsicTypeKind, NominalSourceShapeV1, SourceNominalId,
};
use scoop_identity::{PersistentTypeId, SignatureTypeKey};

#[cfg(test)]
mod tests;

pub(super) enum AnnotationParameterKind {
    Scalar(CanonicalConstValueKindV1),
    Array(CanonicalConstValueKindV1),
}

impl AnnotationParameterKind {
    pub(super) fn accepts(&self, value: &CanonicalAnnotationValueV1) -> bool {
        match (self, value) {
            (Self::Scalar(kind), CanonicalAnnotationValueV1::Scalar(value)) => {
                *kind == value.kind()
            }
            (
                Self::Array(kind),
                CanonicalAnnotationValueV1::Array {
                    element_type,
                    elements,
                },
            ) => kind == element_type && elements.iter().all(|value| value.kind() == *kind),
            _ => false,
        }
    }
}

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn annotation_parameter_kinds(
        &self,
        declaration: &AnnotationDeclarationV1,
    ) -> Result<Vec<AnnotationParameterKind>, Error> {
        declaration
            .parameters
            .iter()
            .map(|parameter| match &parameter.value_type {
                SignatureTypeKey::Nominal(id) => self
                    .annotation_scalar_kind(*id)
                    .map(AnnotationParameterKind::Scalar),
                SignatureTypeKey::NominalApplication { origin, arguments } => {
                    if self
                        .annotation_intrinsic_family(SourceNominalId::GenericTemplate(*origin))?
                        != IntrinsicTypeKind::Array
                    {
                        return Err(Error::Invalid(
                            "annotation array parameter must use core Array",
                        ));
                    }
                    let [SignatureTypeKey::Nominal(element)] = arguments.as_slice() else {
                        return Err(Error::Invalid(
                            "annotation array requires one scalar element type",
                        ));
                    };
                    self.annotation_scalar_kind(*element)
                        .map(AnnotationParameterKind::Array)
                }
                _ => Err(Error::Invalid(
                    "annotation parameter must have a scalar or array type",
                )),
            })
            .collect()
    }

    fn annotation_scalar_kind(
        &self,
        id: PersistentTypeId,
    ) -> Result<CanonicalConstValueKindV1, Error> {
        match self.annotation_intrinsic_family(SourceNominalId::Concrete(id))? {
            IntrinsicTypeKind::Boolean => Ok(CanonicalConstValueKindV1::Boolean),
            IntrinsicTypeKind::Integer(kind) => Ok(CanonicalConstValueKindV1::Integer(kind)),
            IntrinsicTypeKind::Float(kind) => Ok(CanonicalConstValueKindV1::Float(kind)),
            IntrinsicTypeKind::Char => Ok(CanonicalConstValueKindV1::Char),
            IntrinsicTypeKind::String => Ok(CanonicalConstValueKindV1::String),
            _ => Err(Error::Invalid(
                "annotation element is not a scalar intrinsic type",
            )),
        }
    }

    fn annotation_intrinsic_family(&self, id: SourceNominalId) -> Result<IntrinsicTypeKind, Error> {
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
                "annotation parameter is not an intrinsic type",
            ));
        };
        Ok(representation.family())
    }
}
