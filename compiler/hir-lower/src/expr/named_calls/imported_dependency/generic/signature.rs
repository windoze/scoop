use super::*;

mod intrinsic;
mod variant;

#[derive(Clone)]
pub(in crate::expr) enum ImportedGenericTarget {
    Function(hir::ImportedGenericCallableTemplateId),
    Constructor(hir::ImportedConstructorTemplateId),
    Variant(std::sync::Arc<variant::ImportedVariantSignature>),
    Intrinsic(std::sync::Arc<intrinsic::ImportedIntrinsicSignature>),
}

#[derive(Clone)]
pub(in crate::expr) struct ImportedInferenceSignature {
    pub owner_parameters: Vec<hir::TypeParamDecl>,
    pub type_parameters: Vec<hir::TypeParamDecl>,
    pub parameters: Vec<(String, hir::TypeId)>,
    pub receiver: Option<hir::TypeId>,
    pub return_type: hir::TypeId,
    pub origin: hir::DefinitionOrigin,
    pub span: hir::Span,
}

impl ImportedGenericTarget {
    pub(in crate::expr::named_calls::imported_dependency) fn request(
        state: &mut Lowerer,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<Self, String> {
        if matches!(
            declaration.interface().effects().implementation(),
            hir::CallableImplementationV1::Intrinsic(
                hir::IntrinsicFunctionKind::Pointer(_)
                    | hir::IntrinsicFunctionKind::Array(_)
                    | hir::IntrinsicFunctionKind::ArrayAccess(_)
            )
        ) {
            return intrinsic::ImportedIntrinsicSignature::prepare(state, declaration)
                .map(|signature| Self::Intrinsic(std::sync::Arc::new(signature)));
        }
        match declaration.interface().declaration() {
            scoop_identity::CallableTemplateOrigin::Constructor(_) => state
                .request_imported_constructor_template(declaration)
                .map(Self::Constructor),
            scoop_identity::CallableTemplateOrigin::VariantConstructor(_) => {
                variant::ImportedVariantSignature::prepare(state, declaration)
                    .map(|signature| Self::Variant(std::sync::Arc::new(signature)))
            }
            _ => state
                .request_imported_generic_template(declaration)
                .map(Self::Function),
        }
    }

    pub(in crate::expr::named_calls::imported_dependency) fn declaration(
        &self,
        state: &Lowerer,
    ) -> hir::ImportedCallableDeclaration {
        match self {
            Self::Function(id) => state.imported_generic_templates[*id]
                .source
                .declaration()
                .clone(),
            Self::Constructor(id) => state.imported_constructor_templates[*id].source.clone(),
            Self::Variant(signature) => signature.declaration.clone(),
            Self::Intrinsic(signature) => signature.declaration.clone(),
        }
    }

    pub(in crate::expr) fn signature(
        &self,
        state: &Lowerer,
    ) -> (
        ImportedInferenceSignature,
        crate::imported_core::ImportedTypeBindings,
    ) {
        match self {
            Self::Function(id) => {
                let template = &state.imported_generic_templates[*id];
                let owner_count = match template.declaration {
                    hir::ImportedCallableTemplateOrigin::Nominal {
                        owner_parameter_count,
                        ..
                    } => owner_parameter_count,
                    _ => 0,
                };
                let (owner, callable) = template
                    .type_parameters
                    .declarations()
                    .split_at(owner_count);
                (
                    ImportedInferenceSignature {
                        owner_parameters: owner.to_vec(),
                        type_parameters: callable.to_vec(),
                        parameters: template
                            .params
                            .iter()
                            .map(|p| (p.name.clone(), p.ty))
                            .collect(),
                        receiver: template.receiver,
                        return_type: template.return_ty,
                        origin: template.origin,
                        span: template.span,
                    },
                    template.bindings.clone(),
                )
            }
            Self::Constructor(id) => {
                let template = &state.imported_constructor_templates[*id];
                let signature = &template.signature;
                (
                    ImportedInferenceSignature {
                        owner_parameters: Vec::new(),
                        type_parameters: signature.type_parameters.clone(),
                        parameters: signature
                            .parameters
                            .iter()
                            .map(|p| (p.name.clone(), p.ty))
                            .collect(),
                        receiver: None,
                        return_type: signature.owner,
                        origin: signature.origin,
                        span: signature.origin.span,
                    },
                    template.bindings.clone(),
                )
            }
            Self::Variant(signature) => (signature.signature.clone(), signature.bindings.clone()),
            Self::Intrinsic(signature) => (signature.signature.clone(), signature.bindings.clone()),
        }
    }
}
