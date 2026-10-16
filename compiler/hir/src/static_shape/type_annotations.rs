use super::*;

impl<'a> StaticFieldShape<'a> {
    /// An unannotated declaration has an empty tree; actual argument annotations
    /// can still be supplied to `value_type().annotated_signature(...)`.
    pub fn type_annotations(
        self,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> &'a [TypeAnnotationNode] {
        self.annotation
            .type_annotations(self.owner, dependencies, AnnotationTypeSlot::Value)
    }
}

impl StaticAnnotationTarget {
    pub(super) fn type_annotations<'a>(
        self,
        owner: StaticNominalShape<'a>,
        dependencies: &ImportedSemanticWorld<'a>,
        slot: AnnotationTypeSlot,
    ) -> &'a [TypeAnnotationNode] {
        match self {
            StaticAnnotationTarget::Current(target) => owner
                .module
                .annotations
                .annotated_type(target, slot)
                .map_or(&[], |usage| usage.nodes.as_slice()),
            StaticAnnotationTarget::Dependency(target) => dependencies
                .nominal_source_provider(owner.declaration)
                .expect("a loaded nominal retains its provider")
                .source_interface()
                .annotations()
                .annotated_type(target, slot)
                .map_or(&[], |usage| usage.nodes.as_slice()),
        }
    }
}

impl SourceAnnotations {
    pub fn type_use(&self, identity: &AnnotationTypeUseId) -> Option<&SourceAnnotatedTypeUse> {
        self.type_uses
            .iter()
            .find(|usage| &usage.identity == identity)
    }

    pub fn annotated_type(
        &self,
        target: SourceAnnotationTarget,
        slot: AnnotationTypeSlot,
    ) -> Option<&SourceAnnotatedTypeUse> {
        let site = self
            .type_sites
            .iter()
            .find(|site| site.target == target && site.slot == slot)?;
        self.type_use(&site.type_use)
    }
}

impl SourceAnnotatedTypeUse {
    pub fn published(
        &self,
        module: &Module,
    ) -> Result<AnnotatedTypeUseV1, HirSignatureTypeMappingError> {
        let binders = self
            .parameters
            .iter()
            .enumerate()
            .map(|(index, &parameter)| HirSignatureBinder {
                parameter,
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let ty = match self.ty {
            AnnotationParameterType::Value(ty) => AnnotationParameterType::Value(
                HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(module))
                    .map(ty, &binders)?,
            ),
            AnnotationParameterType::Annotation(id) => AnnotationParameterType::Annotation(id),
            AnnotationParameterType::AnnotationArray { array, element } => {
                AnnotationParameterType::AnnotationArray { array, element }
            }
        };
        Ok(AnnotatedTypeUseV1 {
            identity: self.identity.clone(),
            ty,
            binder_count: self.parameters.len() as u32,
            nodes: self.nodes.clone(),
        })
    }
}

/// The annotation tree of an actual application. Type identity still comes
/// from the ordinary declaration-to-argument substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticAnnotatedType {
    pub ty: scoop_identity::SignatureTypeKey,
    pub nodes: Vec<TypeAnnotationNode>,
}

impl StaticShapeTypeUse<'_> {
    pub fn annotated_signature(
        self,
        module: &Module,
        binders: &[HirSignatureBinder],
        declaration_annotations: &[TypeAnnotationNode],
        argument_annotations: &[Vec<TypeAnnotationNode>],
    ) -> Result<StaticAnnotatedType, HirSignatureTypeMappingError> {
        let ty = self.signature(module, binders)?;
        let nodes = if self.arguments.is_empty() {
            declaration_annotations.to_vec()
        } else {
            let template = match self.source {
                StaticShapeTypeSource::Published(signature) => signature.clone(),
                StaticShapeTypeSource::Hir(ty) => {
                    let parameters = self
                        .parameters
                        .iter()
                        .enumerate()
                        .map(|(index, parameter)| HirSignatureBinder {
                            parameter: parameter.id,
                            depth: 0,
                            index: index as u32,
                        })
                        .collect::<Vec<_>>();
                    HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(module))
                        .map(ty, &parameters)?
                }
            };
            substitute_type_annotations(&template, declaration_annotations, argument_annotations)
        };
        Ok(StaticAnnotatedType { ty, nodes })
    }
}
