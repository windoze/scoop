//! Deterministic digest-DAG projection for the strong production profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableBodyKey, DefinitionAtomRole, DigestKind, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{BudgetMeter, HashError};

use crate::{
    DefinitionAtomResolutionError, DigestInputRefV1, DigestNodeBuildError, DigestNodeV1,
    EntryProductionSourceV1, Module, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongDigestPlanBuildError, StrongImmortalObjectSemanticPlanBuildError,
    StrongImmortalObjectSemanticPlanSetV1, StrongInitializationSchedulePlanV1,
    StrongInitializationUnitSemanticPlanBuildError, StrongInitializationUnitSemanticPlanSet,
    StrongInitializationUnitSemanticPlanSetV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongTypeDescriptorSemanticPlanBuildError,
    StrongTypeDescriptorSemanticPlanSet, StrongTypeDescriptorSemanticPlanSetV1,
};

/// Projects the only digest graph accepted by the single-Cone strong writer.
///
/// This function deliberately derives every node, edge, and patch from the
/// final LIR graph and its sealed foundation. Callers cannot supply a partial
/// graph or add an alternative digest path.
pub(crate) fn project_strong_digest_finalization_plan(
    module: &Module,
    foundation: &OdrFreeLirFoundation,
    entry_source: &EntryProductionSourceV1,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    if module.cone != foundation.producer() {
        return Err(StrongDigestProjectionError::ProducerMismatch {
            module: module.cone,
            foundation: foundation.producer(),
        });
    }

    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Safepoints)?;
    let types = StrongTypeDescriptorSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Types)?;
    let immortals = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::ImmortalObjects)?;
    let initialization = StrongInitializationUnitSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::InitializationUnits)?;

    DigestGraphWriter::new(foundation).project(
        &safepoints,
        &types,
        &immortals,
        &initialization,
        entry_source,
    )
}

/// Projects the same digest graph while validating the V2 descriptor
/// semantics. Initialization dependency payloads do not contribute digest
/// inputs, so their local semantic base can be used before external unit
/// definitions are joined to the completed digest identities.
pub(crate) fn project_strong_digest_finalization_plan_v2(
    module: &Module,
    foundation: &OdrFreeLirFoundation,
    entry_source: &EntryProductionSourceV1,
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
    meter: &mut BudgetMeter,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    if module.cone != foundation.producer() {
        return Err(StrongDigestProjectionError::ProducerMismatch {
            module: module.cone,
            foundation: foundation.producer(),
        });
    }
    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Safepoints)?;
    let types = crate::StrongTypeDescriptorSemanticPlanSetV2::from_module(module, selected, meter)
        .map_err(StrongDigestProjectionError::Types)?;
    let immortals = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::ImmortalObjects)?;
    let initialization = StrongInitializationUnitSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::InitializationUnits)?;
    DigestGraphWriter::new(foundation).project(
        &safepoints,
        &types,
        &immortals,
        &initialization,
        entry_source,
    )
}

#[derive(Default)]
struct DigestNodeDraft {
    inputs: BTreeSet<DigestNodeKey>,
    patches: BTreeSet<DigestPatchIntentKey>,
}

struct DigestGraphWriter<'foundation> {
    foundation: &'foundation OdrFreeLirFoundation,
    nodes: BTreeMap<DigestNodeKey, DigestNodeDraft>,
    registration_nodes: BTreeSet<DigestNodeKey>,
}

impl<'foundation> DigestGraphWriter<'foundation> {
    fn new(foundation: &'foundation OdrFreeLirFoundation) -> Self {
        Self {
            foundation,
            nodes: BTreeMap::new(),
            registration_nodes: BTreeSet::new(),
        }
    }

    fn project<D: Copy, C, I>(
        mut self,
        safepoints: &StrongSafepointSemanticPlanSetV1,
        types: &StrongTypeDescriptorSemanticPlanSet<D, C>,
        immortals: &StrongImmortalObjectSemanticPlanSetV1,
        initialization: &StrongInitializationUnitSemanticPlanSet<I>,
        entry_source: &EntryProductionSourceV1,
    ) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
        self.project_safepoints(safepoints)?;
        self.project_callables()?;
        self.project_types(types)?;
        self.project_immortal_objects(immortals)?;
        self.project_static_storages(initialization)?;
        self.project_initialization_units(initialization)?;
        self.project_entry(entry_source)?;
        self.project_image()?;
        self.finish()
    }

    fn project_safepoints(
        &mut self,
        semantics: &StrongSafepointSemanticPlanSetV1,
    ) -> Result<(), StrongDigestProjectionError> {
        for semantic in semantics.sites() {
            let registration = self.definition(
                StrongDefinitionEntity::safepoint_site(semantic.site()),
                StrongDefinitionRole::SafepointRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let stackmap = DigestNodeKey::stackmap_record(semantic.site());
            self.ensure(stackmap);
            self.patch(
                stackmap,
                registration.plan,
                DigestSemanticFieldRole::NormalizedStackmap,
            )?;

            self.registration(registration.plan, [registration_object, stackmap])?;

            let body = self.definition(
                StrongDefinitionEntity::callable_body(semantic.owner()),
                StrongDefinitionRole::CallableBody,
            )?;
            let body_node = DigestNodeKey::object_definition(body.primary);
            self.ensure(body_node);
            self.input(body_node, stackmap);
        }
        Ok(())
    }

    fn project_callables(&mut self) -> Result<(), StrongDigestProjectionError> {
        let bodies = self
            .foundation
            .callable_bodies()
            .iter()
            .map(|record| record.id())
            .collect::<Vec<_>>();
        for body in bodies {
            let definition = self.definition(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )?;
            let body_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(body_node);

            let registration = self.definition(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);
            self.patch(
                body_node,
                registration.plan,
                DigestSemanticFieldRole::CallableBodyDefinition,
            )?;
            self.registration(registration.plan, [registration_object, body_node])?;
        }
        Ok(())
    }

    fn project_types<D: Copy, C>(
        &mut self,
        semantics: &StrongTypeDescriptorSemanticPlanSet<D, C>,
    ) -> Result<(), StrongDigestProjectionError> {
        for semantic in semantics.descriptors() {
            let exact = semantic.exact_type();
            let registration = self.definition(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let descriptor = self.definition(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
            )?;
            let descriptor_node = DigestNodeKey::object_definition(descriptor.primary);
            self.ensure(descriptor_node);
            self.patch(
                descriptor_node,
                registration.plan,
                DigestSemanticFieldRole::DescriptorDefinition,
            )?;

            let layout_node = DigestNodeKey::layout(semantic.instance_layout());
            self.ensure(layout_node);
            self.patch(
                layout_node,
                registration.plan,
                DigestSemanticFieldRole::Layout,
            )?;
            self.registration(
                registration.plan,
                [registration_object, descriptor_node, layout_node],
            )?;
        }
        Ok(())
    }

    fn project_immortal_objects(
        &mut self,
        semantics: &StrongImmortalObjectSemanticPlanSetV1,
    ) -> Result<(), StrongDigestProjectionError> {
        for semantic in semantics.objects() {
            let object = semantic.object();
            let definition = self.definition(
                StrongDefinitionEntity::immortal_object(object),
                StrongDefinitionRole::ImmortalObject,
            )?;
            let object_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(object_node);

            let registration = self.definition(
                StrongDefinitionEntity::immortal_object(object),
                StrongDefinitionRole::ImmortalRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);
            self.registration(registration.plan, [registration_object, object_node])?;
        }
        Ok(())
    }

    fn project_static_storages<I>(
        &mut self,
        initialization: &StrongInitializationUnitSemanticPlanSet<I>,
    ) -> Result<(), StrongDigestProjectionError> {
        for semantic in initialization.static_storages().storages() {
            let storage = semantic.storage();
            let definition = self.definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::StaticStorage,
            )?;
            let storage_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(storage_node);

            let registration = self.definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let layout_node = DigestNodeKey::layout(semantic.layout());
            self.ensure(layout_node);
            self.patch(
                layout_node,
                registration.plan,
                DigestSemanticFieldRole::Layout,
            )?;

            let scan_node = DigestNodeKey::scan(semantic.scan());
            self.ensure(scan_node);
            self.patch(scan_node, registration.plan, DigestSemanticFieldRole::Scan)?;
            self.registration(
                registration.plan,
                [registration_object, storage_node, layout_node, scan_node],
            )?;
        }
        Ok(())
    }

    fn project_initialization_units<I>(
        &mut self,
        semantics: &StrongInitializationUnitSemanticPlanSet<I>,
    ) -> Result<(), StrongDigestProjectionError> {
        for semantic in semantics.units() {
            let entity = StrongDefinitionEntity::initialization_unit(semantic.unit());
            let registration =
                self.definition(entity, StrongDefinitionRole::InitializationRegistration)?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let cell = self.definition(entity, StrongDefinitionRole::InitializationCell)?;
            let cell_node = DigestNodeKey::object_definition(cell.primary);
            self.ensure(cell_node);

            let descriptor =
                self.definition(entity, StrongDefinitionRole::InitializationDescriptor)?;
            let descriptor_node = DigestNodeKey::object_definition(descriptor.primary);
            self.ensure(descriptor_node);

            let mut inputs = vec![registration_object, cell_node, descriptor_node];
            if let StrongInitializationSchedulePlanV1::EagerStartup { gateway } =
                semantic.schedule()
            {
                let gateway = self.definition(
                    StrongDefinitionEntity::callable_body(gateway),
                    StrongDefinitionRole::CallableBody,
                )?;
                let gateway_node = DigestNodeKey::object_definition(gateway.primary);
                self.ensure(gateway_node);
                self.patch(
                    gateway_node,
                    registration.plan,
                    DigestSemanticFieldRole::GatewayDefinition,
                )?;
                inputs.push(gateway_node);
            }
            self.registration(registration.plan, inputs)?;
        }
        Ok(())
    }

    fn project_entry(
        &mut self,
        source: &EntryProductionSourceV1,
    ) -> Result<(), StrongDigestProjectionError> {
        let EntryProductionSourceV1::Executable(entry) = source else {
            return Ok(());
        };
        if entry.root_cone() != self.foundation.producer() {
            return Err(StrongDigestProjectionError::EntryProducerMismatch {
                entry: entry.root_cone(),
                foundation: self.foundation.producer(),
            });
        }
        let descriptor = self.definition(
            StrongDefinitionEntity::root_entry(entry.root_cone()),
            StrongDefinitionRole::RootEntryDescriptor,
        )?;
        let source_node = DigestNodeKey::source_signature(entry.main().body());
        self.ensure(source_node);
        self.patch(
            source_node,
            descriptor.plan,
            DigestSemanticFieldRole::SourceSignature,
        )?;

        let gateway = PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(
            entry.root_cone(),
            entry.main(),
        ))
        .map_err(StrongDigestProjectionError::Identity)?;
        let gateway = self.definition(
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableBody,
        )?;
        let gateway_node = DigestNodeKey::object_definition(gateway.primary);
        self.ensure(gateway_node);
        self.patch(
            gateway_node,
            descriptor.plan,
            DigestSemanticFieldRole::GatewayDefinition,
        )
    }

    fn project_image(&mut self) -> Result<(), StrongDigestProjectionError> {
        let image = self.definition(
            StrongDefinitionEntity::cone_image(self.foundation.producer()),
            StrongDefinitionRole::ImageDescriptor,
        )?;
        let image_node = DigestNodeKey::runtime_image(self.foundation.producer());
        self.ensure(image_node);
        for registration in self.registration_nodes.clone() {
            self.input(image_node, registration);
        }
        self.patch(
            image_node,
            image.plan,
            DigestSemanticFieldRole::RuntimeImage,
        )
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

#[derive(Debug)]
pub enum StrongDigestProjectionError {
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    EntryProducerMismatch {
        entry: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    Safepoints(StrongSafepointSemanticPlanError),
    Types(StrongTypeDescriptorSemanticPlanBuildError),
    ImmortalObjects(StrongImmortalObjectSemanticPlanBuildError),
    InitializationUnits(StrongInitializationUnitSemanticPlanBuildError),
    DefinitionIdentity(ObjectDefinitionIdentityError),
    MissingDefinition {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    PrimaryAtom {
        plan: ObjectDefinitionPlanId,
        source: DefinitionAtomResolutionError,
    },
    Identity(HashError),
    Node(DigestNodeBuildError),
    Plan(StrongDigestPlanBuildError),
}

impl fmt::Display for StrongDigestProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project strong digest graph: {self:?}")
    }
}

impl std::error::Error for StrongDigestProjectionError {}
