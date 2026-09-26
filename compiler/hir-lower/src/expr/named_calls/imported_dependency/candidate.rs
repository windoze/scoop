use hir::ImportedCallableSource;
use scoop_hir as hir;

#[derive(Clone)]
pub(super) enum ImportedCallableCandidate {
    Binding(Box<hir::ImportedDependencyCallableCandidate>),
    Member(Box<hir::ImportedCallableDeclaration>),
}

#[derive(Clone, Copy)]
pub(super) enum NormalizedImportedIntrinsic {
    Integer(hir::IntegerIntrinsicKind),
    Unary(hir::PrimitiveUnaryKind),
    Binary(hir::PrimitiveBinaryKind),
}

impl ImportedCallableCandidate {
    fn source(&self) -> &dyn ImportedCallableSource {
        match self {
            Self::Binding(source) => source.as_ref(),
            Self::Member(source) => source.as_ref(),
        }
    }

    pub(super) fn capability(&self) -> Option<&hir::ParamFreeNominalCallableV1> {
        match self {
            Self::Binding(source) => source.capability(),
            Self::Member(source) => source.capability(),
        }
    }

    pub(super) fn normalized_intrinsic(&self) -> Option<NormalizedImportedIntrinsic> {
        let Self::Member(source) = self else {
            return None;
        };
        let interface = source.interface();
        if !interface.type_parameters().is_empty()
            || interface.effects().execution() != scoop_identity::Effect::Ordinary
            || interface.modality() != hir::CallableModalityV1::Final
            || interface.access() != hir::PublicLookupAccessV1::DirectOnly
        {
            return None;
        }
        let hir::CallableImplementationV1::Intrinsic(intrinsic) =
            interface.effects().implementation()
        else {
            return None;
        };
        match intrinsic {
            hir::IntrinsicFunctionKind::Integer(
                kind @ (hir::IntegerIntrinsicKind::NoGcOperation { .. }
                | hir::IntegerIntrinsicKind::Conversion { .. }),
            ) => Some(NormalizedImportedIntrinsic::Integer(kind)),
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
        self.capability().is_some() || self.normalized_intrinsic().is_some()
    }
}

impl ImportedCallableSource for ImportedCallableCandidate {
    fn interface(&self) -> &hir::CallableInterfaceRecordV1 {
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
