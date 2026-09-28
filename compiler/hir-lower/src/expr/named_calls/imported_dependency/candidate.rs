use hir::ImportedCallableSource;
use scoop_hir as hir;

#[derive(Clone)]
pub(super) enum ImportedCallableCandidate {
    Binding(Box<hir::ImportedDependencyCallableCandidate>),
    Declaration(Box<hir::ImportedCallableDeclaration>),
}

#[derive(Clone, Copy)]
pub(super) enum NormalizedImportedIntrinsic {
    Integer(hir::IntegerIntrinsicKind),
    Unary(hir::PrimitiveUnaryKind),
    Binary(hir::PrimitiveBinaryKind),
}

impl ImportedCallableCandidate {
    pub(super) fn description(&self) -> &'static str {
        if matches!(
            self.interface().declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
                | scoop_identity::CallableTemplateOrigin::VariantConstructor(_)
        ) {
            "constructor"
        } else {
            "function"
        }
    }

    fn source(&self) -> &dyn ImportedCallableSource {
        match self {
            Self::Binding(source) => source.as_ref(),
            Self::Declaration(source) => source.as_ref(),
        }
    }

    pub(super) fn capability(&self) -> Option<&hir::ParamFreeNominalCallableV1> {
        match self {
            Self::Binding(source) => source.capability(),
            Self::Declaration(source) => source.capability(),
        }
    }

    pub(super) fn normalized_intrinsic(&self) -> Option<NormalizedImportedIntrinsic> {
        let Self::Declaration(source) = self else {
            return None;
        };
        let interface = source.interface();
        if !interface.type_parameters().is_empty()
            || interface.effects().execution() != scoop_identity::Effect::Ordinary
            || interface.modality() != hir::CallableModalityV1::Final
            || !interface.slot_relations().is_empty()
        {
            return None;
        }
        let hir::CallableImplementationV1::Intrinsic(intrinsic) =
            interface.effects().implementation()
        else {
            return None;
        };
        match intrinsic {
            hir::IntrinsicFunctionKind::Integer(kind) => {
                Some(NormalizedImportedIntrinsic::Integer(kind))
            }
            hir::IntrinsicFunctionKind::PrimitiveUnary(kind) => {
                Some(NormalizedImportedIntrinsic::Unary(kind))
            }
            hir::IntrinsicFunctionKind::PrimitiveBinary(kind) => {
                Some(NormalizedImportedIntrinsic::Binary(kind))
            }
            _ => None,
        }
    }

    pub(super) fn integer_equality_kind(&self) -> Option<hir::IntegerKind> {
        match self.normalized_intrinsic()? {
            NormalizedImportedIntrinsic::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
                kind,
                operation: hir::NoGcIntegerOperation::Equals,
            }) => Some(kind),
            _ => None,
        }
    }

    pub(super) fn executable(&self) -> bool {
        self.capability().is_some()
            || self.normalized_intrinsic().is_some()
            || matches!(
                self.interface().declaration(),
                scoop_identity::CallableTemplateOrigin::VariantConstructor(_)
            )
    }
}

impl ImportedCallableSource for ImportedCallableCandidate {
    fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        self.source().source_location(source, context)
    }
    fn callable_body(&self) -> Option<&hir::ExportGenericCallableBodyV1> {
        self.source().callable_body()
    }
    fn interface(&self) -> &hir::CallableDeclarationRecordV1 {
        self.source().interface()
    }
    fn source_interface(&self) -> Option<&hir::CallableSourceInterfaceV1> {
        self.source().source_interface()
    }
    fn default_template(
        &self,
        key: hir::ExportDefaultTemplateKeyV1,
    ) -> Option<&hir::ExportDefaultTemplateV1> {
        self.source().default_template(key)
    }
    fn definition_source(
        &self,
        source: &hir::ExportDefinitionSourceV1,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        self.source().definition_source(source)
    }
}
