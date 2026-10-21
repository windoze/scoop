use super::*;
use crate::call_resolution::candidates::NominalConstructorSource;

impl Lowerer {
    pub(super) fn query_variant_constructors(
        &mut self,
        application: hir::EnumApplicationId,
        result: hir::SemanticTypeUse,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Vec<hir::SemanticConstructorDescription> {
        let enumeration = self.enum_id(self.enum_applications[application].template);
        (0..self.enums[enumeration].variants.len())
            .map(|index| {
                let variant = self.enum_variant_at(application, index as u32);
                let source = hir::EnumVariantRef::checked(&self.enums, enumeration, index as u32)
                    .expect("the constructor belongs to this variant declaration");
                let identity = CallableTemplateOrigin::VariantConstructor(variant.variant);
                let mut parameters = self.query_current_parameters(
                    hir::ExportParameterOwner::VariantConstructor(source),
                    identity,
                    substitutions,
                );
                for (index, parameter) in parameters.iter_mut().enumerate() {
                    parameter.target =
                        hir::StaticParameterTarget::Field(hir::StaticFieldIdentity::VariantField(
                            self.enum_variant_field_at(variant, index as u32).field,
                        ));
                }
                hir::SemanticConstructorDescription {
                    identity,
                    kind: hir::SemanticConstructorKind::Variant(variant.variant),
                    parameters,
                    result,
                    access: hir::SemanticMemberAccess {
                        visibility: hir::DeclaredVisibilityV1::Public,
                        domain: self.enums[enumeration].access.lookup.0.clone(),
                        accessible: self
                            .constructor_is_accessible(NominalConstructorSource::Variant(source)),
                    },
                    annotations: hir::SemanticAnnotationTarget::Current(
                        hir::SourceAnnotationTarget::Callable(
                            hir::SourceAnnotationCallable::VariantConstructor(source),
                        ),
                    ),
                    origin: self.query_member_location(
                        Owner::Enum(enumeration),
                        self.enum_variant_spans[&source],
                    ),
                }
            })
            .collect()
    }
}
