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
    foundation: &'foundation ConeLirFoundation,
    nodes: BTreeMap<DigestNodeKey, DigestNodeDraft>,
    registration_nodes: BTreeSet<DigestNodeKey>,
}

impl<'foundation> DigestGraphWriter<'foundation> {
    pub(super) fn new(foundation: &'foundation ConeLirFoundation) -> Self {
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
    ) -> Result<DigestFinalizationPlanV1, DigestProjectionError> {
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
        self.project_odr_definitions()?;
        self.project_image()?;
        self.finish()
    }

    fn registration(
        &mut self,
        definition: ObjectDefinitionPlanId,
        inputs: impl IntoIterator<Item = DigestNodeKey>,
    ) -> Result<(), DigestProjectionError> {
        let record = self
            .foundation
            .definition_plan(definition)
            .ok_or(DigestProjectionError::MissingDefinitionPlan(definition))?;
        let node = self.foundation.registration_digest_key(record);
        self.ensure(node);
        if node.kind() == DigestKind::OdrDefinition {
            let (_, primary) = self
                .foundation
                .resolve_definition_atom(definition, DefinitionAtomRole::Primary)
                .map_err(|source| DigestProjectionError::PrimaryAtom {
                    plan: definition,
                    source,
                })?;
            let object = DigestNodeKey::object_definition(primary);
            self.input(node, object);
            self.input(node, DigestNodeKey::lir_definition(primary));
            for input in inputs {
                if input != object {
                    self.input(object, input);
                    if input.kind() == DigestKind::StackmapRecord {
                        self.input(node, input);
                    }
                }
            }
        } else {
            for input in inputs {
                self.input(node, input);
            }
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
    ) -> Result<StrongDefinition, DigestProjectionError> {
        let plan = self
            .foundation
            .definition_for(entity, role)
            .map(|record| record.id())
            .ok_or(DigestProjectionError::MissingDefinition { entity, role })?;
        let (_, primary) = self
            .foundation
            .resolve_definition_atom(plan, DefinitionAtomRole::Primary)
            .map_err(|source| DigestProjectionError::PrimaryAtom { plan, source })?;
        Ok(StrongDefinition { plan, primary })
    }

    fn project_odr_definitions(&mut self) -> Result<(), DigestProjectionError> {
        for record in self.foundation.definition_plans() {
            let scoop_identity::ObjectDefinitionPlanOwner::Odr { member } = record.key().owner()
            else {
                continue;
            };
            let plan = record.id();
            let (_, primary) = self
                .foundation
                .resolve_definition_atom(plan, DefinitionAtomRole::Primary)
                .map_err(|source| DigestProjectionError::PrimaryAtom { plan, source })?;
            let node = DigestNodeKey::odr_member_definition(member);
            let object = DigestNodeKey::object_definition(primary);
            self.input(node, DigestNodeKey::lir_definition(primary));
            self.input(node, object);
            let stackmaps = self
                .nodes
                .get(&object)
                .into_iter()
                .flat_map(|draft| draft.inputs.iter())
                .filter(|input| input.kind() == DigestKind::StackmapRecord)
                .copied()
                .collect::<Vec<_>>();
            for stackmap in stackmaps {
                self.input(node, stackmap);
            }
        }
        Ok(())
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
    ) -> Result<(), DigestProjectionError> {
        let source = DigestNodeId::from_key(&node).map_err(DigestProjectionError::Identity)?;
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

    fn finish(self) -> Result<DigestFinalizationPlanV1, DigestProjectionError> {
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
                    .map_err(DigestProjectionError::Node)
            })
            .collect::<Result<Vec<_>, _>>()?;
        DigestFinalizationPlanV1::new(nodes, self.foundation).map_err(DigestProjectionError::Plan)
    }
}

#[derive(Clone, Copy)]
struct StrongDefinition {
    plan: ObjectDefinitionPlanId,
    primary: ObjectDefinitionAtomId,
}

fn digest_input(key: DigestNodeKey) -> Result<DigestInputRefV1, DigestProjectionError> {
    let id = DigestNodeId::from_key(&key).map_err(DigestProjectionError::Identity)?;
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
