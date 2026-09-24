use scoop_identity::{
    BindingTarget, CallableMaterialization, CallableMaterializationContext, CallableOwner,
    CallableTemplateOrigin, CallableTemplateOwner, CanonicalIdentifier, CborIdentityRecord,
    ConcreteExpressionOrigin, ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerChain, EvaluationOrigin, ExactTypeKey, ExportBindingKey,
    NormalizedSourcePath, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
    PersistentExportBindingId, PersistentFunctionId, PersistentSourceContextId, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
    ValidatedIdentityGraph,
};

use super::super::*;
use crate::{
    CanonicalDependencyBindingWitnessesV1, DependencyBindingWitnessV1, ExternalHirTargetV1,
    ReexportRouteHopV1, ReexportRouteV1,
};

pub(in crate::cross_cone_interface::external_references) struct Fixture {
    pub(in crate::cross_cone_interface::external_references) current: ConeIdentity,
    pub(in crate::cross_cone_interface::external_references) provider: ConeIdentity,
    pub(in crate::cross_cone_interface::external_references) function:
        CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    pub(in crate::cross_cone_interface::external_references) context:
        CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    pub(in crate::cross_cone_interface::external_references) unit: PersistentExactTypeId,
    pub(in crate::cross_cone_interface::external_references) binding: PersistentExportBindingId,
}

impl Fixture {
    pub(in crate::cross_cone_interface::external_references) fn new() -> Self {
        let current = ConeCoordinate::new("tests", "calls", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let provider = ConeCoordinate::new("tests", "dependency", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                current,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("execute").unwrap(),
            0,
            None,
            vec![],
        ))
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::Callable {
            source: SourceIdentity::new(
                current,
                NormalizedSourcePath::new("src/calls.scoop").unwrap(),
            )
            .unwrap(),
            owner: CallableOwner::Function(function.id()),
        })
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
            current,
            PackagePath::root(),
            CanonicalIdentifier::new("execute").unwrap(),
            BindingTarget::function(function.key()).unwrap(),
        ))
        .unwrap();
        Self {
            current,
            provider,
            function,
            context,
            unit,
            binding,
        }
    }

    pub(in crate::cross_cone_interface::external_references) fn site(
        &self,
        index: u32,
        indices: Vec<u32>,
    ) -> Result<HirDependencyCallSiteV1, HirDependencyCallSiteBuildError> {
        let definition = DefinitionOrigin::new(
            self.context.key().source().clone(),
            SourceSpan::new(10, 20).unwrap(),
            self.context.key(),
        )
        .unwrap();
        HirDependencyCallSiteV1::try_new(
            ExecutableExpressionPosition {
                root: CallableMaterialization::new(
                    CallableTemplateOwner::Function(self.function.id()),
                    CallableMaterializationContext::NoSubstitution,
                ),
                expression_index: index,
            },
            ConcreteExpressionOrigin::new(
                definition.clone(),
                EvaluationOrigin::at_definition(&definition),
            ),
            vec![self.unit, self.unit],
            self.unit,
            indices,
        )
    }

    pub(in crate::cross_cone_interface::external_references) fn target(
        &self,
    ) -> ExternalHirTargetV1 {
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(self.function.id()))
    }

    pub(in crate::cross_cone_interface::external_references) fn witnesses(
        &self,
    ) -> CanonicalDependencyBindingWitnessesV1 {
        CanonicalDependencyBindingWitnessesV1::try_new(vec![DependencyBindingWitnessV1::new(
            ReexportRouteV1::try_new(
                self.current,
                vec![ReexportRouteHopV1::new(self.current, self.binding)],
            )
            .unwrap(),
        )])
        .unwrap()
    }

    pub(in crate::cross_cone_interface::external_references) fn graph(
        &self,
    ) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(self.current).unwrap();
        pending.register_authority(self.provider).unwrap();
        pending
            .register_external_canonical_authority(self.function.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.register_authority(self.unit).unwrap();
        pending.register_authority(self.binding).unwrap();
        pending.finish().unwrap()
    }
}
