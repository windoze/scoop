use super::*;
use callables::{origin, with_tables};
use scoop_identity::{ConcreteExpressionOrigin, DefinitionOrigin, EvaluationOrigin, PropertyOwner};

impl Loaded {
    pub fn change_last_argument(&mut self, exact: PersistentExactTypeId) {
        let mut references = self.public.external_references().records().to_vec();
        let reference = references
            .iter_mut()
            .find(|reference| reference.call_sites().records().len() > 1)
            .unwrap();
        let mut calls = reference.call_sites().records().to_vec();
        let site = calls.last_mut().unwrap();
        let mut arguments = site.arguments().to_vec();
        arguments[0] = exact;
        *site = HirDependencyCallSiteV1::try_new(
            site.position(),
            site.origin().clone(),
            arguments,
            site.result(),
            site.witness_indices().to_vec(),
        )
        .unwrap();
        *reference = ExternalHirReferenceV1::try_new(
            reference.origin(),
            reference.target(),
            reference.roles().clone(),
            reference.witnesses().clone(),
            CanonicalHirDependencyCallSitesV1::try_new(calls).unwrap(),
            reference.type_sites().clone(),
        )
        .unwrap();
        self.public = with_tables(
            &self.public,
            self.public.callable_interfaces().clone(),
            CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
        );
    }

    /// Produces actual, positioned call records independently of selected uses.
    pub fn calls(&mut self, provider: &Loaded, members: &[InheritanceCallableDeclarationV1]) {
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
        let mut type_sites = Vec::new();
        for (index, member) in members.iter().enumerate() {
            let target = origin(*member);
            let declaration = provider
                .public
                .callable_interfaces()
                .declaration(target)
                .unwrap();
            let mut arguments = Vec::new();
            if let Some(SourceNominalId::Concrete(owner)) = declaration.owner().nominal_owner() {
                arguments.push(exact(owner));
            } else if let Some(receiver) = declaration.receiver() {
                arguments.push(
                    provider
                        .metadata()
                        .signature_exact_type(receiver, &mut meter())
                        .unwrap(),
                );
            }
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
            )
            .unwrap();
            by_target.entry(target).or_default().push(call);
            type_sites.push(HirDependencyTypeSiteV1::Expression(Box::new(
                HirExpressionTypeSiteV1::new(
                    position,
                    origin,
                    HirExpressionTypeRoleV1::Value,
                    result,
                ),
            )));
        }
        let mut references = Vec::new();
        let mut bindings = BTreeSet::new();
        for (target, calls) in by_target {
            let key = ExportBindingKey::new(
                provider.provider,
                PackagePath::root(),
                CanonicalIdentifier::new("fixtureBinding").unwrap(),
                match target {
                    CallableTemplateOrigin::Function(id) => {
                        let key = provider
                            .identities
                            .canonical_key::<_, SourceDeclarationKey>(id)
                            .unwrap();
                        if provider
                            .public
                            .callable_interfaces()
                            .declaration(target)
                            .unwrap()
                            .receiver()
                            .is_some()
                        {
                            BindingTarget::extension_function(&key).unwrap()
                        } else {
                            BindingTarget::function(&key).unwrap()
                        }
                    }
                    CallableTemplateOrigin::Accessor(id) => {
                        let key = provider
                            .identities
                            .canonical_key::<_, PropertyAccessorKey>(id)
                            .unwrap();
                        let PropertyOwner::Property(property) = key.owner() else {
                            panic!("fixture accessors use ordinary properties");
                        };
                        BindingTarget::property(
                            provider
                                .identities
                                .canonical_key::<_, SourceDeclarationKey>(property)
                                .unwrap()
                                .as_ref(),
                        )
                        .unwrap()
                    }
                    _ => panic!("fixture source calls use functions and accessors"),
                },
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
        if !type_sites.is_empty() {
            references.push(
                ExternalHirReferenceV1::try_new(
                    ConeIdentity::CORE,
                    ExternalHirTargetV1::Nominal(SourceNominalId::Concrete(
                        CoreBuiltinNominal::Unit.identity_record().id(),
                    )),
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
