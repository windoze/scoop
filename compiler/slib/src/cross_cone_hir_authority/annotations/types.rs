use super::{CanonicalCrossConeHirSurfaceAuthority, Error};
use scoop_hir::{
    AnnotationDeclarationV1, AnnotationParameterType, CanonicalConstValueKindV1,
    EnumRepresentation, IntrinsicTypeKind, NominalSourceShapeV1, SourceNominalId,
};
use scoop_identity::SignatureTypeKey;

mod atoms;
pub(super) use atoms::{AnnotationAtomKind, AnnotationParameterKind};
#[cfg(test)]
mod tests;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn annotation_parameter_kinds(
        &self,
        declaration: &AnnotationDeclarationV1,
    ) -> Result<Vec<AnnotationParameterKind>, Error> {
        declaration.parameters.iter().map(|parameter| match &parameter.value_type {
            AnnotationParameterType::Annotation(id) => Ok(AnnotationParameterKind::Atom(AnnotationAtomKind::Annotation(*id))),
            AnnotationParameterType::AnnotationArray { array, element } => {
                if !matches!(self.annotation_nominal_shape(SourceNominalId::GenericTemplate(*array))?, NominalSourceShapeV1::Intrinsic(r) if r.family() == IntrinsicTypeKind::Array) {
                    return Err(Error::Invalid("annotation array parameter must use core Array"));
                }
                Ok(AnnotationParameterKind::Array(AnnotationAtomKind::Annotation(*element)))
            }
            AnnotationParameterType::Value(ty) => {
                if let SignatureTypeKey::NominalApplication { origin, arguments } = ty
                    && matches!(self.annotation_nominal_shape(SourceNominalId::GenericTemplate(*origin))?, NominalSourceShapeV1::Intrinsic(r) if r.family() == IntrinsicTypeKind::Array)
                {
                    let [element] = arguments.as_slice() else { return Err(Error::Invalid("annotation array requires one element type")) };
                    return self.annotation_atom_kind(element).map(AnnotationParameterKind::Array);
                }
                self.annotation_atom_kind(ty).map(AnnotationParameterKind::Atom)
            }
        }).collect()
    }

    fn annotation_atom_kind(&self, ty: &SignatureTypeKey) -> Result<AnnotationAtomKind, Error> {
        let id = match ty {
            SignatureTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
            SignatureTypeKey::NominalApplication { origin, .. } => {
                SourceNominalId::GenericTemplate(*origin)
            }
            _ => {
                return Err(Error::Invalid(
                    "annotation value requires a scalar or integral enum type",
                ));
            }
        };
        match self.annotation_nominal_shape(id)? {
            NominalSourceShapeV1::Enum(shape) => {
                let EnumRepresentation::Integral { kind, values } = shape.representation() else {
                    return Err(Error::Invalid(
                        "algebraic enum is not an annotation parameter type",
                    ));
                };
                Ok(AnnotationAtomKind::IntegralEnum {
                    ty: ty.clone(),
                    kind: *kind,
                    variants: shape
                        .variants()
                        .iter()
                        .zip(values)
                        .map(|(v, raw)| (v.variant(), *raw))
                        .collect(),
                })
            }
            NominalSourceShapeV1::Intrinsic(representation) => {
                let kind = match representation.family() {
                    IntrinsicTypeKind::Boolean => CanonicalConstValueKindV1::Boolean,
                    IntrinsicTypeKind::Integer(kind) => CanonicalConstValueKindV1::Integer(kind),
                    IntrinsicTypeKind::Float(kind) => CanonicalConstValueKindV1::Float(kind),
                    IntrinsicTypeKind::Char => CanonicalConstValueKindV1::Char,
                    IntrinsicTypeKind::String => CanonicalConstValueKindV1::String,
                    _ => {
                        return Err(Error::Invalid(
                            "annotation element is not a scalar intrinsic type",
                        ));
                    }
                };
                Ok(AnnotationAtomKind::Scalar(kind))
            }
            _ => Err(Error::Invalid(
                "annotation element is not a scalar or integral enum type",
            )),
        }
    }

    fn annotation_nominal_shape(
        &self,
        id: SourceNominalId,
    ) -> Result<&NominalSourceShapeV1, Error> {
        let key = self.source_nominal_key(id).map_err(Error::Nominal)?;
        self.provider_interface(key.origin())
            .map_err(Error::Nominal)?
            .nominal_interfaces()
            .declaration(id)
            .map(|d| d.source_shape())
            .ok_or(Error::Invalid(
                "annotation parameter has no nominal declaration",
            ))
    }
}
