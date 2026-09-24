//! Shared typed graph assembly for producer and reader projections.

use super::*;

mod callables;
mod registrations;
mod runtime;

#[derive(Default)]
struct DigestNodeDraft {
    inputs: BTreeSet<DigestNodeKey>,
    patches: BTreeSet<DigestPatchIntentKey>,
}

pub(super) struct DigestGraphWriter<'foundation> {
    foundation: &'foundation OdrFreeLirFoundation,
    nodes: BTreeMap<DigestNodeKey, DigestNodeDraft>,
    registration_nodes: BTreeSet<DigestNodeKey>,
}

impl<'foundation> DigestGraphWriter<'foundation> {
    pub(super) fn new(foundation: &'foundation OdrFreeLirFoundation) -> Self {
        Self {
            foundation,
            nodes: BTreeMap::new(),
            registration_nodes: BTreeSet::new(),
        }
    }

    pub(super) fn project<D: Copy, C, I>(
        mut self,
        safepoints: &StrongSafepointSemanticPlanSetV1,
        types: &StrongTypeDescriptorSemanticPlanSet<D, C>,
        immortals: &StrongImmortalObjectSemanticPlanSetV1,
        initialization: &StrongInitializationUnitSemanticPlanSet<I>,
        entry_source: &EntryProductionSourceV1,
    ) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
        self.project_safepoints(
            safepoints
                .sites()
                .iter()
                .map(|value| (value.site(), value.owner())),
        )?;
        self.project_callables()?;
        self.project_types(
            types
                .descriptors()
                .iter()
                .map(|value| (value.exact_type(), value.instance_layout())),
        )?;
        self.project_immortal_objects(immortals.objects().iter().map(|value| value.object()))?;
        self.project_static_storages(
            initialization
                .static_storages()
                .storages()
                .iter()
                .map(|value| (value.storage(), value.layout(), value.scan())),
        )?;
        self.project_initialization_units(
            initialization
                .units()
                .iter()
                .map(|value| (value.unit(), value.schedule())),
        )?;
        self.project_entry(entry_source)?;
        self.project_image()?;
        self.finish()
    }

    fn registration(
        &mut self,
        definition: ObjectDefinitionPlanId,
        inputs: impl IntoIterator<Item = DigestNodeKey>,
    ) -> Result<(), StrongDigestProjectionError> {
        let node = DigestNodeKey::strong_registration(definition);
        self.ensure(node);
        for input in inputs {
            self.input(node, input);
        }
        self.patch(
            node,
            definition,
            DigestSemanticFieldRole::RegistrationDefinition,
        )?;
        self.registration_nodes.insert(node);
        Ok(())
    }

    fn definition(
        &self,
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<StrongDefinition, StrongDigestProjectionError> {
        let key = ObjectDefinitionPlanKey::strong(self.foundation.producer(), entity, role)
            .map_err(StrongDigestProjectionError::DefinitionIdentity)?;
        let plan = self
            .foundation
            .definition_plans()
            .iter()
            .find(|record| record.key() == &key)
            .map(|record| record.id())
            .ok_or(StrongDigestProjectionError::MissingDefinition { entity, role })?;
        let (_, primary) = self
            .foundation
            .resolve_definition_atom(plan, DefinitionAtomRole::Primary)
            .map_err(|source| StrongDigestProjectionError::PrimaryAtom { plan, source })?;
        Ok(StrongDefinition { plan, primary })
    }

    fn ensure(&mut self, key: DigestNodeKey) {
        self.nodes.entry(key).or_default();
    }

    fn input(&mut self, node: DigestNodeKey, input: DigestNodeKey) {
        self.nodes.entry(input).or_default();
        self.nodes.entry(node).or_default().inputs.insert(input);
    }

    fn patch(
        &mut self,
        node: DigestNodeKey,
        target: ObjectDefinitionPlanId,
        role: DigestSemanticFieldRole,
    ) -> Result<(), StrongDigestProjectionError> {
        let source =
            DigestNodeId::from_key(&node).map_err(StrongDigestProjectionError::Identity)?;
        self.nodes
            .entry(node)
            .or_default()
            .patches
            .insert(DigestPatchIntentKey::new(
                source,
                target,
                DefinitionAtomRole::Primary,
                role,
            ));
        Ok(())
    }

    fn finish(self) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
        let nodes = self
            .nodes
            .into_iter()
            .map(|(key, draft)| {
                let inputs = draft
                    .inputs
                    .into_iter()
                    .map(digest_input)
                    .collect::<Result<Vec<_>, _>>()?;
                DigestNodeV1::new(key, inputs, draft.patches.into_iter().collect())
                    .map_err(StrongDigestProjectionError::Node)
            })
            .collect::<Result<Vec<_>, _>>()?;
        StrongDigestFinalizationPlanV1::new(nodes, self.foundation)
            .map_err(StrongDigestProjectionError::Plan)
    }
}

#[derive(Clone, Copy)]
struct StrongDefinition {
    plan: ObjectDefinitionPlanId,
    primary: ObjectDefinitionAtomId,
}

fn digest_input(key: DigestNodeKey) -> Result<DigestInputRefV1, StrongDigestProjectionError> {
    let id = DigestNodeId::from_key(&key).map_err(StrongDigestProjectionError::Identity)?;
    Ok(match key.kind() {
        DigestKind::SourceSignature => DigestInputRefV1::SourceSignature(id),
        DigestKind::Layout => DigestInputRefV1::Layout(id),
        DigestKind::Scan => DigestInputRefV1::Scan(id),
        DigestKind::LirDefinition => DigestInputRefV1::LirDefinition(id),
        DigestKind::ObjectSupport => DigestInputRefV1::ObjectSupport(id),
        DigestKind::ObjectDefinition => DigestInputRefV1::ObjectDefinition(id),
        DigestKind::StackmapRecord => DigestInputRefV1::StackmapRecord(id),
        DigestKind::OdrDefinition => DigestInputRefV1::OdrDefinition(id),
        DigestKind::StrongRegistration => DigestInputRefV1::StrongRegistration(id),
        DigestKind::RuntimeImage => DigestInputRefV1::RuntimeImage(id),
    })
}
