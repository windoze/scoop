use scoop_hir::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    DependencyBindingWitnessV1, ExternalHirReferenceV1, ReexportRouteHopV1, ReexportRouteV1,
};
use scoop_identity::{
    BindingTarget, CallableMaterialization, CanonicalIdentifier, ConcreteExpressionOrigin,
    ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, EvaluationOrigin, ExactTypeKey, ExportBindingKey, NormalizedSourcePath,
    PackagePath, PersistentExactTypeId, PersistentExportBindingId, PersistentFunctionId,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
    StrongCallableDefinitionOwner,
};
use scoop_mir::{
    CanonicalMirFoundation, OdrFreeMirFoundation, SelectedDependencyMirCallableV1,
    StrongCallableBridgeV1,
};

use super::*;

pub(super) struct Fixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    root: PersistentFunctionId,
    target: PersistentFunctionId,
    binding: PersistentExportBindingId,
    pub(super) unit: PersistentExactTypeId,
    pub(super) bridge: CrossConeMirBridgeSectionV1,
    pub(super) strong: StrongCallableBridgeSurfaceV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let cone = |name| {
            ConeCoordinate::new("tests", name, "1.0.0")
                .unwrap()
                .identity()
                .unwrap()
        };
        let current = cone("consumer");
        let provider = cone("provider");
        let declaration = |cone, name| {
            SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    cone,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new(name).unwrap(),
                0,
                None,
                vec![],
            )
        };
        let root =
            PersistentFunctionId::from_source_declaration(&declaration(current, "caller")).unwrap();
        let target_key = declaration(provider, "callee");
        let target = PersistentFunctionId::from_source_declaration(&target_key).unwrap();
        let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
            provider,
            PackagePath::root(),
            CanonicalIdentifier::new("callee").unwrap(),
            BindingTarget::function(&target_key).unwrap(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit);
        let strong = StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
            CallableOwner::Function(root),
            signature.clone(),
        )])
        .unwrap();
        let bridge = bridge(current, provider, target, signature);
        Self {
            current,
            provider,
            root,
            target,
            binding,
            unit,
            bridge,
            strong,
        }
    }

    pub(super) fn set_signature(&mut self, signature: ExactCallableSignature) {
        self.bridge = bridge(self.current, self.provider, self.target, signature);
    }

    pub(super) fn site(
        &self,
        index: u32,
        arguments: Vec<PersistentExactTypeId>,
    ) -> HirDependencyCallSiteV1 {
        let source = SourceIdentity::new(
            self.current,
            NormalizedSourcePath::new("src/caller.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::Callable {
            source: source.clone(),
            owner: CallableOwner::Function(self.root),
        };
        let definition =
            DefinitionOrigin::new(source, SourceSpan::new(10, 20).unwrap(), &context).unwrap();
        HirDependencyCallSiteV1::try_new(
            scoop_hir::concrete::ExecutableExpressionPosition {
                root: CallableMaterialization::new(
                    CallableTemplateOwner::Function(self.root),
                    CallableMaterializationContext::NoSubstitution,
                ),
                expression_index: index,
            },
            ConcreteExpressionOrigin::new(
                definition.clone(),
                EvaluationOrigin::at_definition(&definition),
            ),
            arguments,
            self.unit,
            vec![0],
            scoop_hir::SourceCallReceiver::NoReceiver,
        )
        .unwrap()
    }

    pub(super) fn interface(
        &self,
        sites: Vec<HirDependencyCallSiteV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        let reference = ExternalHirReferenceV1::try_new(
            self.provider,
            dependency_hir_target(DependencyCallableDeclarationId::Function(self.target)),
            CanonicalExternalHirReferenceRolesV1::try_new(vec![
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ])
            .unwrap(),
            CanonicalDependencyBindingWitnessesV1::try_new(vec![DependencyBindingWitnessV1::new(
                ReexportRouteV1::try_new(
                    self.provider,
                    vec![ReexportRouteHopV1::new(self.provider, self.binding)],
                )
                .unwrap(),
            )])
            .unwrap(),
            CanonicalHirDependencyCallSitesV1::try_new(sites).unwrap(),
            Default::default(),
        )
        .unwrap();
        selected::empty_interface(vec![reference])
    }
}

fn bridge(
    current: ConeIdentity,
    provider: ConeIdentity,
    target: PersistentFunctionId,
    signature: ExactCallableSignature,
) -> CrossConeMirBridgeSectionV1 {
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    CrossConeMirBridgeSectionV1::try_new(
        current,
        &foundation,
        vec![],
        vec![
            SelectedDependencyMirCallableV1::try_new(
                provider,
                DependencyCallableDeclarationId::Function(target),
                StrongCallableDefinitionOwner::Function(target),
                signature,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
