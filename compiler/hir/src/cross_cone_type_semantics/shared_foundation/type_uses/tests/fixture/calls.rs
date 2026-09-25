use super::*;
use callables::{origin, with_tables};
use scoop_identity::{ConcreteExpressionOrigin, DefinitionOrigin, EvaluationOrigin};

mod bindings;
mod changes;

impl Loaded {
    /// Produces actual, positioned call records independently of selected uses.
    pub fn calls(&mut self, provider: &Loaded, members: &[InheritanceCallableDeclarationV1]) {
        self.declaration_calls(
            provider,
            &members.iter().copied().map(origin).collect::<Vec<_>>(),
        );
    }

    pub fn declaration_calls(&mut self, provider: &Loaded, targets: &[CallableTemplateOrigin]) {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&self.identities)
            .unwrap();
        pending
            .register_external_graph_authorities(&provider.identities)
            .unwrap();
        let caller = CborIdentityRecord::<PersistentFunctionId, _>::from_key(
            SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    self.provider,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("execute").unwrap(),
                0,
                None,
                Vec::new(),
            ),
        )
        .unwrap();
        let source = self.foundation.source_records()[0].identity().clone();
        let context = CborIdentityRecord::<PersistentSourceContextId, _>::from_key(
            SourceContextKey::Callable {
                source: source.clone(),
                owner: CallableOwner::Function(caller.id()),
            },
        )
        .unwrap();
        let definition =
            DefinitionOrigin::new(source, SourceSpan::new(0, 7).unwrap(), context.key()).unwrap();
        let root = CallableMaterialization::new(
            CallableTemplateOwner::Function(caller.id()),
            CallableMaterializationContext::NoSubstitution,
        );
        pending
            .register_external_canonical_authority(caller)
            .unwrap();
        pending
            .register_external_canonical_authority(context)
            .unwrap();
        let mut by_target: BTreeMap<CallableTemplateOrigin, Vec<HirDependencyCallSiteV1>> =
            BTreeMap::new();
        let mut type_sites: BTreeMap<
            (ConeIdentity, SourceNominalId),
            Vec<HirDependencyTypeSiteV1>,
        > = BTreeMap::new();
        for (index, target) in targets.iter().copied().enumerate() {
            let declaration = provider
                .public
                .callable_interfaces()
                .declaration(target)
                .unwrap();
            let mut arguments = Vec::new();
            if let Some(SourceNominalId::Concrete(owner)) = declaration.owner().nominal_owner()
                && !matches!(
                    target,
                    CallableTemplateOrigin::Constructor(_)
                        | CallableTemplateOrigin::VariantConstructor(_)
                )
            {
                arguments.push(exact(owner));
            } else if let Some(receiver) = declaration.receiver() {
                arguments.push(
                    provider
                        .metadata()
                        .signature_exact_type(receiver, &mut meter())
                        .unwrap(),
                );
            }
            let receiver = match arguments.first() {
                Some(static_type) => crate::SourceCallReceiver::Receiver {
                    static_type: *static_type,
                },
                None => crate::SourceCallReceiver::NoReceiver,
            };
            for parameter in declaration.parameters().parameters() {
                arguments.push(
                    provider
                        .metadata()
                        .signature_exact_type(parameter.value_type(), &mut meter())
                        .unwrap(),
                );
            }
            let result = provider
                .metadata()
                .signature_exact_type(declaration.result(), &mut meter())
                .unwrap();
            let position = crate::concrete::ExecutableExpressionPosition {
                root,
                expression_index: index as u32,
            };
            let origin = ConcreteExpressionOrigin::new(
                definition.clone(),
                EvaluationOrigin::at_definition(&definition),
            );
            let call = HirDependencyCallSiteV1::try_new(
                position,
                origin.clone(),
                arguments,
                result,
                vec![0],
                receiver,
            )
            .unwrap();
            by_target.entry(target).or_default().push(call);
            let type_site =
                HirDependencyTypeSiteV1::Expression(Box::new(HirExpressionTypeSiteV1::new(
                    position,
                    origin,
                    HirExpressionTypeRoleV1::Value,
                    result,
                )));
            for owner in collect_type_site_nominals(
                result,
                |exact| provider.identities.canonical_key::<_, ExactTypeKey>(exact),
                &mut meter(),
            )
            .unwrap()
            {
                let key = match owner {
                    SourceNominalId::Concrete(owner) => provider
                        .identities
                        .canonical_key::<_, SourceDeclarationKey>(owner),
                    SourceNominalId::GenericTemplate(owner) => {
                        provider
                            .identities
                            .canonical_key::<_, SourceDeclarationKey>(owner)
                    }
                }
                .unwrap();
                type_sites
                    .entry((key.origin(), owner))
                    .or_default()
                    .push(type_site.clone());
            }
        }
        let mut references = Vec::new();
        let mut bindings = BTreeSet::new();
        for (target, calls) in by_target {
            let key = ExportBindingKey::new(
                provider.provider,
                PackagePath::root(),
                CanonicalIdentifier::new("fixtureBinding").unwrap(),
                bindings::target(provider, target),
            );
            let binding =
                CborIdentityRecord::<PersistentExportBindingId, _>::from_key(key).unwrap();
            let witnesses = CanonicalDependencyBindingWitnessesV1::try_new(vec![
                DependencyBindingWitnessV1::new(
                    ReexportRouteV1::try_new(
                        provider.provider,
                        vec![ReexportRouteHopV1::new(provider.provider, binding.id())],
                    )
                    .unwrap(),
                ),
            ])
            .unwrap();
            if bindings.insert(binding.id()) {
                pending
                    .register_external_canonical_authority(binding)
                    .unwrap();
            }
            references.push(
                ExternalHirReferenceV1::try_new(
                    provider.provider,
                    ExternalHirTargetV1::Callable(target),
                    CanonicalExternalHirReferenceRolesV1::try_new(vec![
                        ExternalHirReferenceRoleV1::ConcreteSelectedUse,
                    ])
                    .unwrap(),
                    witnesses,
                    CanonicalHirDependencyCallSitesV1::try_new(calls).unwrap(),
                    Default::default(),
                )
                .unwrap(),
            );
        }
        for ((provider, owner), type_sites) in type_sites {
            references.push(
                ExternalHirReferenceV1::try_new(
                    provider,
                    ExternalHirTargetV1::Nominal(owner),
                    CanonicalExternalHirReferenceRolesV1::try_new(vec![
                        ExternalHirReferenceRoleV1::ExecutableTypeDependency,
                    ])
                    .unwrap(),
                    CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
                    Default::default(),
                    CanonicalHirDependencyTypeSitesV1::try_new(type_sites, &mut meter()).unwrap(),
                )
                .unwrap(),
            );
        }
        self.identities = pending.finish().unwrap();
        self.public = with_tables(
            &self.public,
            self.public.callable_interfaces().clone(),
            CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
        );
    }
}
