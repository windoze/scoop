use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::effect_facts::{
    BodyEffectFacts, CallSiteEffectFacts, CallTargetMode, CaseTag, ClassCtorSiteEffectFacts,
    ConcreteOpKey, EffectFamilyKey, HandleSiteEffectFacts, ImplPlan, MaterializedEffectFacts,
    PerformSiteEffectFacts, ResumeSiteEffectFacts, SiteEffectFacts, StepCaseFact, StepSchema,
    StepSchemaId,
};
use crate::mir::{
    BasicBlockId, Body, CallArg, CallKind, LocalId, MaterializedMirPassView, MemberAccessMetadata,
    MemberTarget, Operand, PatternBindingStep, PerformArg, ResumeMetadata, Rvalue, SiteId,
    StatementKind, StoredContinuationRoutePublication, StoredContinuationValueRoute,
    TerminatorKind,
};
use crate::ty::{NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::EffectLoweringError;
use super::ir::{
    BoundaryId, BoundarySiteKind, ContinuationObjectId, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallBoundaryLowering,
    LateLoweredCallBoundaryOperandContract, LateLoweredClassCtorBoundaryLowering,
    LateLoweredCompleteStepDispatch, LateLoweredCompletionPayloadBinding,
    LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationCapture, LateLoweredContinuationContract, LateLoweredContinuationMethod,
    LateLoweredContinuationObject, LateLoweredContinuationResumeBody,
    LateLoweredContinuationSurfaceResume, LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema,
    LateLoweredFrameSlotKind, LateLoweredHandleArmDispatch, LateLoweredHandleBoundaryCaseRouting,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandleBoundaryLowering,
    LateLoweredHandleBoundaryRouting, LateLoweredHandleContinuationBinder,
    LateLoweredHandleDispatchCarrierContract, LateLoweredHandleDispatchContract,
    LateLoweredHandlePayloadBinder, LateLoweredHandlePendingCompletion,
    LateLoweredHandlePendingPayloadTransport, LateLoweredHandleStateRegion,
    LateLoweredHandleStateRegionEntry, LateLoweredLocalRuntimeErrorTerminalAction,
    LateLoweredOneShotPolicy, LateLoweredOperandSource, LateLoweredPerformBoundaryLowering,
    LateLoweredPerformBoundaryOperandContract, LateLoweredPublishedRuntimeEntry,
    LateLoweredResumeBoundaryLowering, LateLoweredResumeBoundaryOperandContract,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumePayloadBinding,
    LateLoweredRuntimeErrorBoundaryLowering, LateLoweredSourceStatementClassification,
    LateLoweredSourceStatementClassificationKind, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateRole, LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepCase,
    LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredStepType, ResumeInterfaceId, StateId,
};
use super::ir::{
    LateLoweredBodyVersionKey, LateLoweredBoundarySource, LateLoweredContinuationRoute,
    LateLoweredSurfaceResumeDispatchPublication,
};

pub(crate) struct StepMaterialization {
    pub(crate) step_types: Vec<LateLoweredStepType>,
    pub(crate) resume_packings: Vec<LateLoweredResumeInterface>,
    pub(crate) resume_packing_ids_by_step: BTreeMap<StepSchemaId, Vec<ResumeInterfaceId>>,
    pub(crate) resume_packing_ids_by_group:
        BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
}

pub(crate) struct BoundaryMaterializationInputs<'a> {
    pub(crate) root_fqn: &'a str,
    pub(crate) owner_version_key: &'a LateLoweredBodyVersionKey,
    pub(crate) body: &'a Body,
    pub(crate) body_facts: &'a BodyEffectFacts,
    pub(crate) step_type: &'a LateLoweredStepType,
    pub(crate) state_graph: &'a LateLoweredStateGraph,
    pub(crate) frame_schema: &'a LateLoweredFrameSchema,
    pub(crate) boundary_map: &'a LateLoweredBoundaryMap,
    pub(crate) continuation_object: ContinuationObjectId,
    pub(crate) step_types: &'a [LateLoweredStepType],
    pub(crate) types: &'a TypeStore,
    pub(crate) cross_callable_continuation_provenance:
        Option<&'a CrossCallableContinuationProvenance>,
}

#[derive(Debug, Clone)]
pub(crate) struct ContinuationRouteOwnerPlan {
    owner_version_key: LateLoweredBodyVersionKey,
    continuation_object: ContinuationObjectId,
}

impl ContinuationRouteOwnerPlan {
    pub(crate) fn new(
        owner_version_key: LateLoweredBodyVersionKey,
        continuation_object: ContinuationObjectId,
    ) -> Self {
        Self {
            owner_version_key,
            continuation_object,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CrossCallableContinuationProvenance {
    member_routes_by_callee: HashMap<String, Vec<CrossCallableContinuationMemberRoute>>,
}

impl CrossCallableContinuationProvenance {
    fn routes_for_callee(&self, callee_fqn: &str) -> &[CrossCallableContinuationMemberRoute] {
        self.member_routes_by_callee
            .get(callee_fqn)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone)]
struct CrossCallableContinuationMemberRoute {
    param_index: usize,
    member: ContinuationMemberIdentityKey,
    path: Vec<PatternBindingStep>,
    route: LateLoweredContinuationRoute,
}

pub(crate) struct ContinuationObjectMaterializationInputs<'a> {
    pub(crate) continuation_object_id: ContinuationObjectId,
    pub(crate) owner_version_key: LateLoweredBodyVersionKey,
    pub(crate) step_schema_id: StepSchemaId,
    pub(crate) step_schema: &'a StepSchema,
    pub(crate) implemented_packings: &'a [ResumeInterfaceId],
    pub(crate) resume_packing_ids_by_group:
        &'a BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
    pub(crate) captures: Vec<LateLoweredContinuationCapture>,
    pub(crate) effect_facts: &'a MaterializedEffectFacts,
}

struct CallBoundaryDispatchMaterialization {
    dispatch: LateLoweredStepDispatchPlan,
    continuation_compositions: Vec<LateLoweredCallBoundaryContinuationComposition>,
    consumed_runtime_error_case: Option<PendingConsumedRuntimeErrorCase>,
}

pub(crate) struct BoundaryMaterialization {
    pub(crate) state_graph: LateLoweredStateGraph,
    pub(crate) boundary_map: LateLoweredBoundaryMap,
}

struct PendingConsumedRuntimeErrorCase {
    input_case_tag: crate::effect_facts::CaseTag,
    input_concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

struct LocalRuntimeErrorStateTarget {
    boundary_id: BoundaryId,
    owner_state: StateId,
    target_state: StateId,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

struct CallBoundaryDispatchInputs<'a> {
    root_fqn: &'a str,
    boundary_id: BoundaryId,
    input_step: &'a LateLoweredStepType,
    output_step: &'a LateLoweredStepType,
    outward_case_tags: &'a [crate::effect_facts::CaseTag],
    continuation_object: ContinuationObjectId,
    target_state: StateId,
    result_local: Option<LocalId>,
    result_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
    types: &'a TypeStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ContinuationMemberIdentityKey {
    Value(String),
    Fun(String),
    ExtensionValue(String),
    ExtensionFun(String),
    Unresolved { name: String, receiver_ty: TypeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ContinuationMemberKey {
    receiver_local: LocalId,
    member: ContinuationMemberIdentityKey,
}

impl ContinuationMemberKey {
    fn from_metadata(receiver_local: LocalId, member: &MemberAccessMetadata) -> Self {
        let member = match member.resolved.as_ref() {
            Some(MemberTarget::Value { fqn }) => ContinuationMemberIdentityKey::Value(fqn.clone()),
            Some(MemberTarget::Fun { fqn }) => ContinuationMemberIdentityKey::Fun(fqn.clone()),
            Some(MemberTarget::ExtensionValue { fqn }) => {
                ContinuationMemberIdentityKey::ExtensionValue(fqn.clone())
            }
            Some(MemberTarget::ExtensionFun { fqn }) => {
                ContinuationMemberIdentityKey::ExtensionFun(fqn.clone())
            }
            None => ContinuationMemberIdentityKey::Unresolved {
                name: member.name.clone(),
                receiver_ty: member.receiver_ty,
            },
        };
        Self {
            receiver_local,
            member,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalContinuationOrigin {
    Seed(LateLoweredContinuationRoute),
    Copy(LocalId),
    AggregateElement {
        source: LocalId,
        path: Vec<PatternBindingStep>,
    },
    MemberRead(ContinuationMemberKey),
    PatternExtract {
        subject: LocalId,
        path: Vec<PatternBindingStep>,
    },
    PatternMemberRead {
        key: ContinuationMemberKey,
        path: Vec<PatternBindingStep>,
    },
}

#[derive(Default)]
struct PublishedContinuationProvenance {
    local_origins: HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    member_store_routes: HashMap<ContinuationMemberKey, Vec<PublishedMemberStoreRoute>>,
}

#[derive(Debug, Clone)]
struct ResolvedResumeLocalRoute {
    route: Option<LateLoweredContinuationRoute>,
    compatible_route_set: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PublishedMemberStoreRoute {
    None,
    Ambiguous,
    Unique(StoredContinuationValueRoute),
    Resolved {
        path: Vec<PatternBindingStep>,
        route: LateLoweredContinuationRoute,
    },
}

impl PublishedMemberStoreRoute {
    fn from_mir(publication: StoredContinuationRoutePublication) -> Self {
        match publication {
            StoredContinuationRoutePublication::None => Self::None,
            StoredContinuationRoutePublication::Ambiguous => Self::Ambiguous,
            StoredContinuationRoutePublication::Unique(route) => Self::Unique(route),
        }
    }
}

impl PublishedContinuationProvenance {
    fn build(
        root_fqn: &str,
        body: &Body,
        body_facts: &BodyEffectFacts,
        owner_version_key: &LateLoweredBodyVersionKey,
        continuation_object: ContinuationObjectId,
        cross_callable: Option<&CrossCallableContinuationProvenance>,
    ) -> Result<Self, EffectLoweringError> {
        let mut provenance = Self::default();

        for (&site_id, site_facts) in body_facts.sites() {
            let SiteEffectFacts::Handle(handle_facts) = site_facts else {
                continue;
            };
            let handle_arms = lookup_handle_arms(root_fqn, body, site_id)?;
            if handle_arms.len() != handle_facts.arm_facts().len() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致，无法为 continuation provenance 建 seed route",
                        handle_arms.len(),
                        handle_facts.arm_facts().len(),
                    ),
                ));
            }
            for (arm_ordinal, (arm, arm_facts)) in handle_arms
                .iter()
                .zip(handle_facts.arm_facts().iter())
                .enumerate()
            {
                let Some(local) = arm.continuation_local else {
                    continue;
                };
                push_local_origin(
                    &mut provenance.local_origins,
                    local,
                    LocalContinuationOrigin::Seed(LateLoweredContinuationRoute::new(
                        arm_facts.continuation_schema(),
                        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                            owner_version_key: owner_version_key.clone(),
                            owner_continuation_object: continuation_object,
                            site_id,
                            arm_ordinal: arm_ordinal as u32,
                            handled_case: arm_facts.handled_case(),
                        },
                    )),
                );
            }
        }

        for block in &body.blocks {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StatementKind::Assign {
                        target,
                        value: Rvalue::Use(Operand::Local(source)),
                    } => {
                        push_local_origin(
                            &mut provenance.local_origins,
                            *target,
                            LocalContinuationOrigin::Copy(*source),
                        );
                    }
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::EnumVariant {
                                variant_name, args, ..
                            },
                    } => {
                        for (field_index, arg) in args.iter().enumerate() {
                            let Operand::Local(source) = &arg.value else {
                                continue;
                            };
                            push_local_origin(
                                &mut provenance.local_origins,
                                *target,
                                LocalContinuationOrigin::AggregateElement {
                                    source: *source,
                                    path: vec![PatternBindingStep::VariantField {
                                        variant: variant_name.clone(),
                                        field_index,
                                    }],
                                },
                            );
                        }
                    }
                    StatementKind::Assign {
                        target,
                        value: Rvalue::MakeTuple { elements },
                    } => {
                        for (field_index, element) in elements.iter().enumerate() {
                            let Operand::Local(source) = element else {
                                continue;
                            };
                            push_local_origin(
                                &mut provenance.local_origins,
                                *target,
                                LocalContinuationOrigin::AggregateElement {
                                    source: *source,
                                    path: vec![PatternBindingStep::TupleIndex(field_index)],
                                },
                            );
                        }
                    }
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::MemberAccess {
                                receiver: Operand::Local(receiver_local),
                                member,
                                ..
                            },
                    } => {
                        push_local_origin(
                            &mut provenance.local_origins,
                            *target,
                            LocalContinuationOrigin::MemberRead(
                                ContinuationMemberKey::from_metadata(*receiver_local, member),
                            ),
                        );
                    }
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member,
                        continuation_route,
                        ..
                    } => {
                        provenance
                            .member_store_routes
                            .entry(ContinuationMemberKey::from_metadata(
                                *receiver_local,
                                member,
                            ))
                            .or_default()
                            .push(PublishedMemberStoreRoute::from_mir(
                                continuation_route.clone(),
                            ));
                    }
                    StatementKind::Nop
                    | StatementKind::Todo(_)
                    | StatementKind::Assign { .. }
                    | StatementKind::StoreMember { .. }
                    | StatementKind::StoreTopLevelVar { .. } => {}
                }
            }
        }

        if let Some(cross_callable) = cross_callable {
            provenance.add_cross_callable_member_routes(body, cross_callable);
        }

        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    target,
                    value:
                        Rvalue::PatternExtract {
                            subject: Operand::Local(subject),
                            path,
                        },
                } = &stmt.kind
                else {
                    continue;
                };
                push_local_origin(
                    &mut provenance.local_origins,
                    *target,
                    LocalContinuationOrigin::PatternExtract {
                        subject: *subject,
                        path: path.clone(),
                    },
                );
                let Some((key, mut prefix_path)) = member_derived_origin_for_local(
                    *subject,
                    &provenance.local_origins,
                    &mut HashSet::new(),
                ) else {
                    continue;
                };
                prefix_path.extend(path.iter().cloned());
                push_local_origin(
                    &mut provenance.local_origins,
                    *target,
                    LocalContinuationOrigin::PatternMemberRead {
                        key,
                        path: prefix_path,
                    },
                );
            }
        }

        Ok(provenance)
    }

    fn add_cross_callable_member_routes(
        &mut self,
        body: &Body,
        cross_callable: &CrossCallableContinuationProvenance,
    ) {
        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                for route in cross_callable.routes_for_callee(callee_fqn) {
                    let Some(arg) = args.get(route.param_index) else {
                        continue;
                    };
                    let Operand::Local(receiver_local) = &arg.value else {
                        continue;
                    };
                    let key = ContinuationMemberKey {
                        receiver_local: *receiver_local,
                        member: route.member.clone(),
                    };
                    self.member_store_routes.entry(key).or_default().push(
                        PublishedMemberStoreRoute::Resolved {
                            path: route.path.clone(),
                            route: route.route.clone(),
                        },
                    );
                }
            }
        }
    }

    fn resolve_resume_local_route(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
    ) -> Result<ResolvedResumeLocalRoute, EffectLoweringError> {
        let routes = self.resolve_local_routes(
            root_fqn,
            site_id,
            local,
            &mut HashSet::new(),
            &mut HashSet::new(),
        )?;
        match routes.as_slice() {
            [] => Ok(ResolvedResumeLocalRoute {
                route: None,
                compatible_route_set: false,
            }),
            [route] => Ok(ResolvedResumeLocalRoute {
                route: Some(route.clone()),
                compatible_route_set: false,
            }),
            routes if routes_share_dynamic_resume_shape(routes) => Ok(ResolvedResumeLocalRoute {
                route: routes.first().cloned(),
                compatible_route_set: true,
            }),
            _ => Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "continuation local{} 通过 published member write/read route 同时解析到多条互不兼容的 underlying continuation route",
                    local.as_u32(),
                ),
            )),
        }
    }

    fn resolve_local_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        if !visiting_locals.insert(local) {
            return Ok(Vec::new());
        }
        let mut routes = Vec::new();
        if let Some(origins) = self.local_origins.get(&local) {
            for origin in origins {
                match origin {
                    LocalContinuationOrigin::Seed(route) => {
                        push_unique_route(&mut routes, route.clone());
                    }
                    LocalContinuationOrigin::Copy(source) => {
                        for route in self.resolve_local_routes(
                            root_fqn,
                            site_id,
                            *source,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::AggregateElement { .. } => {}
                    LocalContinuationOrigin::MemberRead(key) => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            &[],
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternExtract { subject, path } => {
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *subject,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternMemberRead { key, path } => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                }
            }
        }
        visiting_locals.remove(&local);
        Ok(routes)
    }

    fn resolve_local_pattern_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
        path: &[PatternBindingStep],
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        if !visiting_locals.insert(local) {
            return Ok(Vec::new());
        }
        let mut routes = Vec::new();
        if let Some(origins) = self.local_origins.get(&local) {
            for origin in origins {
                match origin {
                    LocalContinuationOrigin::Seed(route) if path.is_empty() => {
                        push_unique_route(&mut routes, route.clone());
                    }
                    LocalContinuationOrigin::Seed(_) => {}
                    LocalContinuationOrigin::Copy(source) => {
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *source,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::AggregateElement {
                        source,
                        path: element_path,
                    } => {
                        let Some(remaining_path) = path.strip_prefix(element_path.as_slice())
                        else {
                            continue;
                        };
                        let source_routes = if remaining_path.is_empty() {
                            self.resolve_local_routes(
                                root_fqn,
                                site_id,
                                *source,
                                visiting_locals,
                                visiting_members,
                            )?
                        } else {
                            self.resolve_local_pattern_routes(
                                root_fqn,
                                site_id,
                                *source,
                                remaining_path,
                                visiting_locals,
                                visiting_members,
                            )?
                        };
                        for route in source_routes {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::MemberRead(key) => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternExtract {
                        subject,
                        path: prefix_path,
                    } => {
                        let mut combined_path = prefix_path.clone();
                        combined_path.extend_from_slice(path);
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *subject,
                            &combined_path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternMemberRead {
                        key,
                        path: prefix_path,
                    } => {
                        let mut combined_path = prefix_path.clone();
                        combined_path.extend_from_slice(path);
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            &combined_path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                }
            }
        }
        visiting_locals.remove(&local);
        Ok(routes)
    }

    fn resolve_member_path_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        key: &ContinuationMemberKey,
        path: &[PatternBindingStep],
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        let cycle_key = (key.clone(), path.to_vec());
        if !visiting_members.insert(cycle_key.clone()) {
            return Ok(Vec::new());
        }

        let publications = self.member_store_routes.get(key).ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 没有任何 published member write contract，无法把 readback route 接回 continuation provenance",
                    render_continuation_member_key(key),
                ),
            )
        })?;

        let mut routes = Vec::new();
        let mut saw_ambiguous_publication = false;
        let mut saw_matching_publication = false;
        for publication in publications {
            match publication {
                PublishedMemberStoreRoute::None => {}
                PublishedMemberStoreRoute::Ambiguous => {
                    saw_ambiguous_publication = true;
                }
                PublishedMemberStoreRoute::Unique(route) if route.path == path => {
                    saw_matching_publication = true;
                    let source_routes = self.resolve_local_routes(
                        root_fqn,
                        site_id,
                        route.source_local,
                        visiting_locals,
                        visiting_members,
                    )?;
                    if source_routes.is_empty() {
                        visiting_members.remove(&cycle_key);
                        return Err(invalid_boundary_operand_contract(
                            root_fqn,
                            site_id,
                            "Resume",
                            format!(
                                "member {} 的 published write path {} 指向 local{}，但该 source local 没有已发布的 continuation route",
                                render_continuation_member_key(key),
                                render_pattern_path(path),
                                route.source_local.as_u32(),
                            ),
                        ));
                    }
                    for source_route in source_routes {
                        push_unique_route(&mut routes, source_route);
                    }
                }
                PublishedMemberStoreRoute::Resolved {
                    path: route_path,
                    route,
                } if route_path == path => {
                    saw_matching_publication = true;
                    push_unique_route(&mut routes, route.clone());
                }
                PublishedMemberStoreRoute::Unique(_)
                | PublishedMemberStoreRoute::Resolved { .. } => {}
            }
        }

        visiting_members.remove(&cycle_key);

        if saw_ambiguous_publication {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 的 published member write contract 标记为 Ambiguous，无法唯一确定 readback path {} 的 continuation provenance",
                    render_continuation_member_key(key),
                    render_pattern_path(path),
                ),
            ));
        }
        if !saw_matching_publication || routes.is_empty() {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 没有与 readback path {} 对齐的 published continuation write/read provenance",
                    render_continuation_member_key(key),
                    render_pattern_path(path),
                ),
            ));
        }

        Ok(routes)
    }
}

pub(crate) fn build_cross_callable_continuation_provenance(
    pass_view: &MaterializedMirPassView<'_>,
    effect_facts: &MaterializedEffectFacts,
    owner_plans: &HashMap<String, ContinuationRouteOwnerPlan>,
) -> Result<CrossCallableContinuationProvenance, EffectLoweringError> {
    let mut member_routes_by_callee: HashMap<String, Vec<CrossCallableContinuationMemberRoute>> =
        HashMap::new();

    for family in pass_view.instances() {
        let root_fqn = family.root_fqn();
        let Some(fun) = family.root_body() else {
            continue;
        };
        let Some(body) = fun.body.as_ref() else {
            continue;
        };
        let Some(body_facts) = effect_facts.body(family.key()) else {
            continue;
        };
        let Some(owner_plan) = owner_plans.get(root_fqn) else {
            continue;
        };
        let provenance = PublishedContinuationProvenance::build(
            root_fqn,
            body,
            body_facts,
            &owner_plan.owner_version_key,
            owner_plan.continuation_object,
            None,
        )?;

        for (key, publications) in &provenance.member_store_routes {
            let Some(param_index) = fun
                .params
                .iter()
                .position(|param| param.local == key.receiver_local)
            else {
                continue;
            };
            for publication in publications {
                let PublishedMemberStoreRoute::Unique(route) = publication else {
                    continue;
                };
                let source_routes = provenance.resolve_local_routes(
                    root_fqn,
                    SiteId::from_raw(u32::MAX),
                    route.source_local,
                    &mut HashSet::new(),
                    &mut HashSet::new(),
                )?;
                for source_route in source_routes {
                    member_routes_by_callee
                        .entry(root_fqn.to_string())
                        .or_default()
                        .push(CrossCallableContinuationMemberRoute {
                            param_index,
                            member: key.member.clone(),
                            path: route.path.clone(),
                            route: source_route,
                        });
                }
            }
        }
    }

    Ok(CrossCallableContinuationProvenance {
        member_routes_by_callee,
    })
}

fn push_local_origin(
    origins: &mut HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    local: LocalId,
    origin: LocalContinuationOrigin,
) {
    let entry = origins.entry(local).or_default();
    if !entry.contains(&origin) {
        entry.push(origin);
    }
}

fn push_unique_route(
    routes: &mut Vec<LateLoweredContinuationRoute>,
    route: LateLoweredContinuationRoute,
) {
    if !routes.contains(&route) {
        routes.push(route);
    }
}

fn routes_share_dynamic_resume_shape(routes: &[LateLoweredContinuationRoute]) -> bool {
    let Some(first) = routes.first() else {
        return false;
    };
    routes
        .iter()
        .skip(1)
        .all(|route| same_dynamic_resume_route_shape(first, route))
}

fn same_dynamic_resume_route_shape(
    left: &LateLoweredContinuationRoute,
    right: &LateLoweredContinuationRoute,
) -> bool {
    if left.continuation_schema() != right.continuation_schema() {
        return false;
    }
    match (left.publication(), right.publication()) {
        (
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
        ) => true,
        (
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_version_key: left_owner,
                owner_continuation_object: left_object,
                ..
            },
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_version_key: right_owner,
                owner_continuation_object: right_object,
                ..
            },
        ) => left_owner == right_owner && left_object == right_object,
        _ => false,
    }
}

fn member_derived_origin_for_local(
    local: LocalId,
    local_origins: &HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    visiting: &mut HashSet<LocalId>,
) -> Option<(ContinuationMemberKey, Vec<PatternBindingStep>)> {
    if !visiting.insert(local) {
        return None;
    }
    let mut resolved = None;
    for origin in local_origins.get(&local)? {
        let next = match origin {
            LocalContinuationOrigin::MemberRead(key) => Some((key.clone(), Vec::new())),
            LocalContinuationOrigin::PatternMemberRead { key, path } => {
                Some((key.clone(), path.clone()))
            }
            LocalContinuationOrigin::Copy(source) => {
                member_derived_origin_for_local(*source, local_origins, visiting)
            }
            LocalContinuationOrigin::Seed(_)
            | LocalContinuationOrigin::AggregateElement { .. }
            | LocalContinuationOrigin::PatternExtract { .. } => None,
        };
        let Some(next) = next else {
            continue;
        };
        match &resolved {
            Some(existing) if existing != &next => {
                visiting.remove(&local);
                return None;
            }
            Some(_) => {}
            None => resolved = Some(next),
        }
    }
    visiting.remove(&local);
    resolved
}

fn render_continuation_member_key(key: &ContinuationMemberKey) -> String {
    let member = match &key.member {
        ContinuationMemberIdentityKey::Value(fqn)
        | ContinuationMemberIdentityKey::Fun(fqn)
        | ContinuationMemberIdentityKey::ExtensionValue(fqn)
        | ContinuationMemberIdentityKey::ExtensionFun(fqn) => fqn.clone(),
        ContinuationMemberIdentityKey::Unresolved { name, receiver_ty } => {
            format!("{}.{}", receiver_ty.as_u32(), name)
        }
    };
    format!("local{}.{}", key.receiver_local.as_u32(), member)
}

fn render_pattern_path(path: &[PatternBindingStep]) -> String {
    if path.is_empty() {
        return "<identity>".to_string();
    }
    path.iter()
        .map(|step| match step {
            PatternBindingStep::TupleIndex(index) => format!("tuple[{index}]"),
            PatternBindingStep::VariantField {
                variant,
                field_index,
            } => format!("{variant}[{field_index}]"),
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

pub(crate) fn materialize_step_and_resume_interfaces(
    effect_facts: &MaterializedEffectFacts,
) -> Result<StepMaterialization, EffectLoweringError> {
    let mut step_types = Vec::with_capacity(effect_facts.step_schemas().len());
    let mut resume_packings = Vec::new();
    let mut resume_packing_ids_by_step = BTreeMap::new();
    let mut resume_packing_ids_by_group = BTreeMap::new();
    let mut next_interface_raw = 0u32;

    for (&step_schema_id, step_schema) in effect_facts.step_schemas() {
        step_types.push(build_step_type(step_schema_id, step_schema, effect_facts)?);

        let grouped_cases = group_cases_by_effect_family(step_schema);
        let mut interface_ids = Vec::with_capacity(grouped_cases.len());
        for (effect_family, cases) in grouped_cases {
            let interface_id = ResumeInterfaceId::new(next_interface_raw);
            next_interface_raw = next_interface_raw.saturating_add(1);
            resume_packings.push(build_resume_interface(
                interface_id,
                effect_family.clone(),
                step_schema_id,
                step_schema,
                &cases,
                effect_facts,
            )?);
            resume_packing_ids_by_group.insert((step_schema_id, effect_family), interface_id);
            interface_ids.push(interface_id);
        }
        resume_packing_ids_by_step.insert(step_schema_id, interface_ids);
    }

    Ok(StepMaterialization {
        step_types,
        resume_packings,
        resume_packing_ids_by_step,
        resume_packing_ids_by_group,
    })
}

pub(crate) fn materialize_dynamic_invoke_entry(
    step_schema: StepSchemaId,
    step_type: &LateLoweredStepType,
    entry_state: StateId,
    complete_state: StateId,
) -> LateLoweredDynamicInvokeEntry {
    LateLoweredDynamicInvokeEntry::new(
        step_type.invoke_args_tuple_ty(),
        step_schema,
        entry_state,
        complete_state,
    )
}

pub(crate) fn materialize_continuation_object(
    inputs: ContinuationObjectMaterializationInputs<'_>,
) -> Result<LateLoweredContinuationObject, EffectLoweringError> {
    let ContinuationObjectMaterializationInputs {
        continuation_object_id,
        owner_version_key,
        step_schema_id,
        step_schema,
        implemented_packings,
        resume_packing_ids_by_group,
        captures,
        effect_facts,
    } = inputs;
    let surface_resumes = step_schema
        .cases()
        .iter()
        .map(|case| {
            let continuation_contract =
                build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredContinuationSurfaceResume::new(
                case.case_tag(),
                case.concrete_op_key().clone(),
                continuation_contract,
                continuation_resume_body(owner_version_key.impl_plan(), case.case_tag()),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let methods = step_schema
        .cases()
        .iter()
        .map(|case| {
            let interface_id = *resume_packing_ids_by_group
                .get(&(
                    step_schema_id,
                    case.concrete_op_key().effect_family().clone(),
                ))
                .ok_or_else(|| EffectLoweringError::MissingResumeInterfaceFamily {
                    step_schema: step_schema_id.as_u32(),
                    effect_fqn: case
                        .concrete_op_key()
                        .effect_family()
                        .effect_fqn()
                        .to_string(),
                })?;
            let continuation_contract =
                build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredContinuationMethod::new(
                interface_id,
                case.case_tag(),
                case.concrete_op_key().clone(),
                continuation_contract,
                continuation_resume_body(owner_version_key.impl_plan(), case.case_tag()),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredContinuationObject::new(
        continuation_object_id,
        owner_version_key,
        step_schema.continuation_obj_ty(),
        implemented_packings.to_vec(),
        captures,
        surface_resumes,
        methods,
    ))
}

pub(crate) fn materialize_boundary_map(
    inputs: BoundaryMaterializationInputs<'_>,
) -> Result<BoundaryMaterialization, EffectLoweringError> {
    let BoundaryMaterializationInputs {
        root_fqn,
        owner_version_key,
        body,
        body_facts,
        step_type,
        state_graph,
        frame_schema,
        boundary_map,
        continuation_object,
        step_types,
        types,
        cross_callable_continuation_provenance,
    } = inputs;

    let result_locals = collect_result_locals(body);
    let continuation_provenance = PublishedContinuationProvenance::build(
        root_fqn,
        body,
        body_facts,
        owner_version_key,
        continuation_object,
        cross_callable_continuation_provenance,
    )?;
    let (resume_boundaries, runtime_error_boundaries) = paired_resume_boundaries(boundary_map);
    let mut entries = Vec::with_capacity(boundary_map.entries().len());
    let mut local_runtime_error_targets = Vec::new();
    let mut next_state_raw = state_graph
        .states()
        .iter()
        .map(|state| state.state_id().as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    for boundary in boundary_map.entries() {
        let lowering = match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Call,
            } => {
                let facts = clone_call_site_facts(root_fqn, body_facts, site_id)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.callee_schema())?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "Call",
                    }
                })?;
                let result_frame_slot =
                    published_boundary_result_slot(frame_schema, boundary.boundary_id()).and_then(
                        |(slot_local, slot_id)| (slot_local == result_local).then_some(slot_id),
                    );
                let call_dispatch =
                    build_call_boundary_dispatch_plan(CallBoundaryDispatchInputs {
                        root_fqn,
                        boundary_id: boundary.boundary_id(),
                        input_step,
                        output_step: step_type,
                        outward_case_tags: facts.resolved_cases().tags(),
                        continuation_object,
                        target_state: boundary.resume_state(),
                        result_local: Some(result_local),
                        result_frame_slot,
                        types,
                    })?;
                let operand_contract = build_call_boundary_operand_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    &facts,
                    result_local,
                    types,
                )?;
                let consumed_runtime_error_case =
                    call_dispatch.consumed_runtime_error_case.map(|pending| {
                        let target_state = StateId::new(next_state_raw);
                        next_state_raw = next_state_raw.saturating_add(1);
                        local_runtime_error_targets.push(LocalRuntimeErrorStateTarget {
                            boundary_id: boundary.boundary_id(),
                            owner_state: boundary.owner_state(),
                            target_state,
                            payload_tuple_ty: pending.payload_tuple_ty,
                            terminal_action: pending.terminal_action,
                        });
                        LateLoweredConsumedRuntimeErrorCase::new(
                            pending.input_case_tag,
                            pending.input_concrete_op_key,
                            pending.payload_tuple_ty,
                            pending.terminal_action,
                            target_state,
                        )
                    });
                LateLoweredBoundaryLowering::Call(LateLoweredCallBoundaryLowering::new(
                    facts,
                    result_local,
                    operand_contract,
                    call_dispatch.dispatch,
                    call_dispatch.continuation_compositions,
                    consumed_runtime_error_case,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::ClassCtor,
            } => {
                let facts = clone_class_ctor_site_facts(root_fqn, body_facts, site_id)?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "ClassCtor",
                    }
                })?;
                let (class_fqn, source_consumption) = build_class_ctor_boundary_source_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    result_local,
                )?;
                let emitted_steps = facts
                    .emitted_cases()
                    .tags()
                    .iter()
                    .map(|case_tag| {
                        build_current_step_emission(
                            root_fqn,
                            step_type,
                            *case_tag,
                            continuation_object,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                LateLoweredBoundaryLowering::ClassCtor(LateLoweredClassCtorBoundaryLowering::new(
                    facts,
                    result_local,
                    class_fqn,
                    source_consumption,
                    emitted_steps,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Perform,
            } => {
                let facts = clone_perform_site_facts(root_fqn, body_facts, site_id)?;
                let emitted_step = build_current_step_emission(
                    root_fqn,
                    step_type,
                    facts.emitted_case(),
                    continuation_object,
                )?;
                let operand_payload_ty = if is_any_type(types, facts.payload_tuple_ty()) {
                    emitted_step.payload_tuple_ty()
                } else {
                    facts.payload_tuple_ty()
                };
                let operand_contract = build_perform_boundary_operand_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    operand_payload_ty,
                    types,
                )?;
                LateLoweredBoundaryLowering::Perform(LateLoweredPerformBoundaryLowering::new(
                    facts,
                    operand_contract,
                    emitted_step,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } => {
                let facts = clone_resume_site_facts(root_fqn, body_facts, site_id)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.out_step_schema())?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "Resume",
                    }
                })?;
                let runtime_error_boundary =
                    *runtime_error_boundaries.get(&site_id).ok_or_else(|| {
                        EffectLoweringError::MissingPairedRuntimeErrorBoundary {
                            root_fqn: root_fqn.to_string(),
                            site_id: site_id.as_u32(),
                        }
                    })?;
                let dispatch = build_step_dispatch_plan(
                    root_fqn,
                    input_step,
                    step_type,
                    facts.resolved_cases().tags(),
                    continuation_object,
                    boundary.resume_state(),
                    Some(result_local),
                )?;
                let result_frame_slot =
                    published_boundary_result_slot(frame_schema, boundary.boundary_id()).and_then(
                        |(slot_local, slot_id)| (slot_local == result_local).then_some(slot_id),
                    );
                let continuation_compositions = build_boundary_continuation_compositions(
                    root_fqn,
                    boundary.boundary_id(),
                    input_step,
                    &dispatch,
                    boundary.resume_state(),
                    result_local,
                    result_frame_slot,
                )?;
                let operand_contract = build_resume_boundary_operand_contract(
                    root_fqn,
                    owner_version_key,
                    body,
                    state_graph,
                    boundary,
                    &facts,
                    result_local,
                    &continuation_provenance,
                    continuation_object,
                    types,
                )?;
                LateLoweredBoundaryLowering::Resume(LateLoweredResumeBoundaryLowering::new(
                    facts,
                    result_local,
                    runtime_error_boundary,
                    operand_contract,
                    dispatch,
                    continuation_compositions,
                ))
            }
            LateLoweredBoundarySource::RuntimeError { origin_site } => {
                let resume_boundary = *resume_boundaries.get(&origin_site).ok_or_else(|| {
                    EffectLoweringError::MissingPairedResumeBoundary {
                        root_fqn: root_fqn.to_string(),
                        site_id: origin_site.as_u32(),
                    }
                })?;
                let resume_runtime_error_effect =
                    resume_runtime_error_effect_family(root_fqn, body, origin_site, types)?;
                let facts = clone_resume_site_facts(root_fqn, body_facts, origin_site)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.out_step_schema())?;
                let runtime_case = input_step
                    .cases()
                    .iter()
                    .find(|case| {
                        case.concrete_op_key().effect_family() == &resume_runtime_error_effect
                    })
                    .ok_or_else(
                        || EffectLoweringError::MissingRuntimeErrorCaseInResumeStep {
                            root_fqn: root_fqn.to_string(),
                            site_id: origin_site.as_u32(),
                            step_schema: facts.out_step_schema().as_u32(),
                        },
                    )?;
                let emitted_step = build_emission_from_concrete_op(
                    root_fqn,
                    input_step.step_schema(),
                    step_type,
                    runtime_case.concrete_op_key(),
                    continuation_object,
                )?;
                LateLoweredBoundaryLowering::RuntimeError(
                    LateLoweredRuntimeErrorBoundaryLowering::new(
                        origin_site,
                        resume_boundary,
                        emitted_step,
                    ),
                )
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Handle,
            } => {
                let facts = clone_handle_site_facts(root_fqn, body_facts, site_id)?;
                let outward_emissions = build_handle_outward_emissions(
                    root_fqn,
                    step_type,
                    &facts,
                    continuation_object,
                )?;
                LateLoweredBoundaryLowering::Handle(LateLoweredHandleBoundaryLowering::new(
                    facts,
                    outward_emissions,
                ))
            }
        };

        entries.push(boundary.clone().with_lowering(lowering));
    }

    let boundary_map = LateLoweredBoundaryMap::new(entries);
    let state_graph =
        attach_local_runtime_error_states(root_fqn, state_graph, &local_runtime_error_targets)?;
    let state_graph = attach_handle_dispatch_contracts(
        root_fqn,
        body,
        body_facts,
        types,
        &state_graph,
        frame_schema,
        &boundary_map,
        continuation_object,
    )?;

    Ok(BoundaryMaterialization {
        state_graph,
        boundary_map,
    })
}

fn attach_local_runtime_error_states(
    root_fqn: &str,
    state_graph: &LateLoweredStateGraph,
    targets: &[LocalRuntimeErrorStateTarget],
) -> Result<LateLoweredStateGraph, EffectLoweringError> {
    if targets.is_empty() {
        return Ok(state_graph.clone());
    }

    let mut states = state_graph.states().to_vec();
    let mut local_targets_by_owner = BTreeMap::<StateId, Vec<StateId>>::new();
    for target in targets {
        local_targets_by_owner
            .entry(target.owner_state)
            .or_default()
            .push(target.target_state);
        states.push(LateLoweredState::new(
            target.target_state,
            LateLoweredStateRole::Segment,
            Vec::new(),
            LateLoweredStateTerminator::LocalRuntimeError {
                payload_tuple_ty: target.payload_tuple_ty,
                terminal_action: target.terminal_action,
            },
        ));
    }

    let rewritten_states = states
        .into_iter()
        .map(|state| {
            let Some(local_runtime_error_states) = local_targets_by_owner.get(&state.state_id())
            else {
                return Ok(state);
            };
            let terminator = match state.terminator().clone() {
                LateLoweredStateTerminator::Suspend {
                    boundary_ids,
                    resume_state,
                    local_runtime_error_states: existing_local_runtime_error_states,
                    cleanup_state,
                    drop_state,
                } => {
                    let mut merged_local_runtime_error_states = existing_local_runtime_error_states;
                    merged_local_runtime_error_states
                        .extend(local_runtime_error_states.iter().copied());
                    merged_local_runtime_error_states.sort();
                    merged_local_runtime_error_states.dedup();
                    LateLoweredStateTerminator::Suspend {
                        boundary_ids,
                        resume_state,
                        local_runtime_error_states: merged_local_runtime_error_states,
                        cleanup_state,
                        drop_state,
                    }
                }
                _ => {
                    let boundary_id = targets
                        .iter()
                        .find(|target| target.owner_state == state.state_id())
                        .map(|target| target.boundary_id)
                        .expect(
                            "owner state with local runtime-error target should record boundary",
                        );
                    return Err(EffectLoweringError::InvalidLocalRuntimeErrorOwnerState {
                        root_fqn: root_fqn.to_string(),
                        boundary_id: boundary_id.as_u32(),
                        owner_state: state.state_id().as_u32(),
                    });
                }
            };
            Result::<_, EffectLoweringError>::Ok(LateLoweredState::new(
                state.state_id(),
                state.role(),
                state.source_slices().to_vec(),
                terminator,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        state_graph.drop_state(),
        rewritten_states,
    ))
}

#[allow(clippy::too_many_arguments)]
fn attach_handle_dispatch_contracts(
    root_fqn: &str,
    body: &Body,
    body_facts: &BodyEffectFacts,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStateGraph, EffectLoweringError> {
    let rewritten_states = state_graph
        .states()
        .iter()
        .map(|state| {
            let terminator = match state.terminator().clone() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    boundary_ids,
                    drop_state,
                    ..
                } => {
                    let facts = clone_handle_site_facts(root_fqn, body_facts, site_id)?;
                    let contract = build_handle_dispatch_contract(
                        root_fqn,
                        body,
                        state.state_id(),
                        body_state,
                        site_id,
                        &facts,
                        types,
                        state_graph,
                        &arm_states,
                        finally_state,
                        exit_state,
                        frame_schema,
                        &boundary_ids,
                        drop_state,
                        boundary_map,
                        continuation_object,
                    )?;
                    LateLoweredStateTerminator::HandleDispatch {
                        site_id,
                        body_state,
                        arm_states,
                        finally_state,
                        exit_state,
                        contract,
                        boundary_ids,
                        drop_state,
                    }
                }
                other => other,
            };
            Result::<_, EffectLoweringError>::Ok(LateLoweredState::new(
                state.state_id(),
                state.role(),
                state.source_slices().to_vec(),
                terminator,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        state_graph.drop_state(),
        rewritten_states,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_handle_dispatch_contract(
    root_fqn: &str,
    body: &Body,
    dispatch_state: StateId,
    body_state: StateId,
    site_id: SiteId,
    facts: &HandleSiteEffectFacts,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    arm_states: &[StateId],
    finally_state: Option<StateId>,
    exit_state: StateId,
    frame_schema: &LateLoweredFrameSchema,
    boundary_ids: &[BoundaryId],
    drop_state: Option<StateId>,
    boundary_map: &LateLoweredBoundaryMap,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredHandleDispatchContract, EffectLoweringError> {
    if arm_states.len() != facts.arm_facts().len() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "arm state 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致",
                arm_states.len(),
                facts.arm_facts().len(),
            ),
        ));
    }

    let body_complete_target = finally_state.unwrap_or(exit_state);
    let arm_complete_target = finally_state.unwrap_or(exit_state);
    let finally_complete_target = finally_state.map(|_| exit_state);
    let handle_arms = lookup_handle_arms(root_fqn, body, site_id)?;
    if handle_arms.len() != facts.arm_facts().len() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "canonical MIR handle arm 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致",
                handle_arms.len(),
                facts.arm_facts().len(),
            ),
        ));
    }
    let handled_arms = facts
        .arm_facts()
        .iter()
        .zip(arm_states.iter().copied())
        .zip(handle_arms.iter().enumerate())
        .map(|((arm_facts, arm_state), (arm_ordinal, arm))| {
            let published_payload_tuple_ty = arm.payload_tuple_ty.ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 缺少 payload tuple type，无法发布 authoritative binder contract",
                    ),
                )
            })?;
            if published_payload_tuple_ty != arm_facts.payload_tuple_ty() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 的 payload tuple ty t{} 与 HandleSiteEffectFacts 发布的 t{} 不一致",
                        published_payload_tuple_ty.as_u32(),
                        arm_facts.payload_tuple_ty().as_u32(),
                    ),
                ));
            }
            if arm.binder_count != arm.binder_locals.len() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 的 binder_count={} 与 binder_locals.len()={} 不一致",
                        arm.binder_count,
                        arm.binder_locals.len(),
                    ),
                ));
            }
            let payload_binders = arm
                .binder_locals
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, local)| {
                    LateLoweredHandlePayloadBinder::new(
                        ordinal as u32,
                        local,
                        frame_schema
                            .slot_for_kind(crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder {
                                site_id,
                                local,
                                ordinal: ordinal as u32,
                            })
                            .map(|slot| slot.slot_id()),
                    )
                })
                .collect::<Vec<_>>();
            let continuation_binder = arm.continuation_local.map(|local| {
                LateLoweredHandleContinuationBinder::new(
                    local,
                    find_frame_slot_for_local(frame_schema, local),
                    arm_facts.continuation_schema(),
                    continuation_object,
                )
            });
            let completion_payload_source = handle_arm_completion_payload_source(
                root_fqn,
                site_id,
                body,
                types,
                state_graph,
                arm_state,
                arm_states,
                finally_state,
                exit_state,
                arm.body_ty,
            )?;
            Ok(LateLoweredHandleArmDispatch::new(
                arm_facts.handled_case(),
                arm_state,
                arm_ordinal as u32,
                arm_facts.payload_tuple_ty(),
                completion_payload_source,
                payload_binders,
                continuation_binder,
                arm_facts.arm_outward_cases().tags().to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let body_outward_cases = facts.body_outward_cases().tags().to_vec();
    let finally_outward_cases = facts.finally_outward_cases().tags().to_vec();
    let expected_outward_case_tags = collect_handle_outward_case_tags(facts);
    let handle_boundary_lowering =
        find_handle_boundary_lowering(root_fqn, site_id, boundary_ids, boundary_map)?;
    let outward_emissions = handle_boundary_lowering
        .map(|lowering| lowering.outward_emissions().to_vec())
        .unwrap_or_default();
    let published_outward_case_tags = outward_emissions
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    if handle_boundary_lowering.is_some()
        && published_outward_case_tags != expected_outward_case_tags
    {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "published outward emissions {} 与 HandleSiteEffectFacts 期望的 outward cases {} 不一致",
                format_case_tag_set(&published_outward_case_tags),
                format_case_tag_set(&expected_outward_case_tags),
            ),
        ));
    }

    let mut pending_completions = Vec::new();
    if finally_state.is_some() {
        pending_completions.push(LateLoweredHandlePendingCompletion::ContinueToExit);
        pending_completions.push(LateLoweredHandlePendingCompletion::ReturnFromFunction);
        if handle_boundary_lowering.is_some() {
            let mut pending_outward_cases = facts
                .body_outward_cases()
                .tags()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            for arm in facts.arm_facts() {
                pending_outward_cases.extend(arm.arm_outward_cases().tags().iter().copied());
            }
            for case_tag in pending_outward_cases {
                pending_completions.push(LateLoweredHandlePendingCompletion::PropagateOutward(
                    case_tag,
                ));
            }
        }
    }
    let state_regions = build_handle_state_region_entries(
        root_fqn,
        site_id,
        state_graph,
        dispatch_state,
        body_state,
        &handled_arms,
        finally_state,
        exit_state,
    )?;
    let body_completion_payload_source = handle_body_completion_payload_source(
        root_fqn,
        site_id,
        body,
        types,
        state_graph,
        &state_regions,
        body_complete_target,
        facts.result_ty(),
    )?;
    let boundary_routings = build_handle_boundary_routings(
        root_fqn,
        site_id,
        &state_regions,
        &handled_arms,
        &body_outward_cases,
        &finally_outward_cases,
        &outward_emissions,
        &pending_completions,
        boundary_map,
    )?;
    let pending_payload_transports = build_handle_pending_payload_transports(
        root_fqn,
        site_id,
        &pending_completions,
        &outward_emissions,
        frame_schema,
    )?;

    Ok(LateLoweredHandleDispatchContract::new(
        LateLoweredHandleDispatchCarrierContract::new(
            crate::effect_lowered::ir::SystemSlotKind::StateTag,
            crate::effect_lowered::ir::SystemSlotKind::CompletionTag,
            crate::effect_lowered::ir::SystemSlotKind::ResumePayloadCarrier,
        ),
        body_complete_target,
        arm_complete_target,
        finally_complete_target,
        Some(body_completion_payload_source),
        handled_arms,
        body_outward_cases,
        finally_outward_cases,
        outward_emissions,
        pending_completions,
        pending_payload_transports,
        state_regions,
        boundary_routings,
        drop_state,
    ))
}

#[allow(clippy::too_many_arguments)]
fn handle_arm_completion_payload_source(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    arm_state: StateId,
    arm_states: &[StateId],
    finally_state: Option<StateId>,
    exit_state: StateId,
    body_ty: TypeId,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    let arm_complete_target = finally_state.unwrap_or(exit_state);
    if matches!(
        types.kind(body_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(body_ty));
    }

    let mut stop_states = BTreeSet::from([exit_state]);
    stop_states.extend(
        arm_states
            .iter()
            .copied()
            .filter(|state| *state != arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    let mut published = None;
    for state_id in
        collect_handle_region_states(root_fqn, site_id, state_graph, arm_state, &stop_states)?
    {
        let state = state_graph.state(state_id).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "handle arm completion payload source 引用了不存在的 arm state st{}",
                    state_id.as_u32()
                ),
            )
        })?;
        if !matches!(
            state.terminator(),
            LateLoweredStateTerminator::Goto { target } if *target == arm_complete_target
        ) {
            continue;
        }
        let candidate = handle_completion_payload_source_from_state(
            root_fqn,
            site_id,
            body,
            types,
            state_graph,
            state.state_id(),
            body_ty,
            "handle arm completion payload source",
        )?;
        if let Some(existing) = &published {
            if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle arm completion payload source 歧义：已发布 {:?}，又发现 {:?}",
                        existing, candidate
                    ),
                ));
            }
            continue;
        }
        published = Some(candidate);
    }

    published.ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "non-Unit handle arm completion payload source 从 st{} 到 st{} 缺少 completion payload source",
                arm_state.as_u32(),
                arm_complete_target.as_u32()
            ),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_body_completion_payload_source(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    state_regions: &[LateLoweredHandleStateRegionEntry],
    body_complete_target: StateId,
    result_ty: TypeId,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    if matches!(
        types.kind(result_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(result_ty));
    }

    let mut published = None;
    let mut return_fallback = None;
    for entry in state_regions {
        if entry.region() != LateLoweredHandleStateRegion::Body {
            continue;
        }
        let state = state_graph.state(entry.state_id()).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "handle body completion payload source 引用了不存在的 body state st{}",
                    entry.state_id().as_u32()
                ),
            )
        })?;
        let candidate = match state.terminator() {
            LateLoweredStateTerminator::Goto { target } if *target == body_complete_target => {
                handle_completion_payload_source_from_state(
                    root_fqn,
                    site_id,
                    body,
                    types,
                    state_graph,
                    state.state_id(),
                    result_ty,
                    "handle body completion payload source",
                )?
            }
            LateLoweredStateTerminator::Return { payload_source, .. } => {
                let candidate = payload_source.clone();
                if let Some(existing) = &return_fallback {
                    if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                        return Err(invalid_handle_dispatch_contract(
                            root_fqn,
                            site_id,
                            format!(
                                "handle body return fallback payload source 歧义：已发布 {:?}，又发现 {:?}",
                                existing, candidate
                            ),
                        ));
                    }
                    continue;
                }
                return_fallback = Some(candidate);
                continue;
            }
            _ => continue,
        };
        if let Some(existing) = &published {
            if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle body completion payload source 歧义：已发布 {:?}，又发现 {:?}",
                        existing, candidate
                    ),
                ));
            }
            continue;
        }
        published = Some(candidate);
    }

    if let Some(source) = published.or(return_fallback) {
        return Ok(source);
    }

    Err(invalid_handle_dispatch_contract(
        root_fqn,
        site_id,
        format!(
            "non-Unit handle body 缺少指向 st{} 的 completion payload source",
            body_complete_target.as_u32()
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn handle_completion_payload_source_from_state(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    state_id: StateId,
    complete_ty: TypeId,
    context: &str,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    if matches!(
        types.kind(complete_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(complete_ty));
    }
    let state = state_graph.state(state_id).ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!("{context} 引用了不存在的 state st{}", state_id.as_u32()),
        )
    })?;
    let mut skipped_type_mismatches = Vec::new();

    for slice in state.source_slices().iter().rev() {
        if slice.end_statement_index() == slice.start_statement_index() {
            continue;
        }
        let block = body
            .blocks
            .get(slice.block_id().as_u32() as usize)
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle arm completion payload source 引用了不存在的 block bb{}",
                        slice.block_id().as_u32()
                    ),
                )
            })?;
        for stmt_index in (slice.start_statement_index()..slice.end_statement_index()).rev() {
            let stmt = block.stmts.get(stmt_index as usize).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "{context} 引用了不存在的 bb{} stmt{}",
                        slice.block_id().as_u32(),
                        stmt_index
                    ),
                )
            })?;
            let StatementKind::Assign { target, .. } = &stmt.kind else {
                continue;
            };
            let local = body.locals.get(target.as_u32() as usize).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!("{context} 引用了不存在的 local{}", target.as_u32()),
                )
            })?;
            if local.ty != complete_ty && !is_any_type(types, complete_ty) {
                skipped_type_mismatches.push(format!(
                    "local{}:t{}",
                    target.as_u32(),
                    local.ty.as_u32()
                ));
                continue;
            }
            return Ok(LateLoweredCompletionPayloadSource::operand(
                LateLoweredOperandSource::new_local(*target, complete_ty, Some(stmt.span)),
            ));
        }
    }

    let skipped = if skipped_type_mismatches.is_empty() {
        String::new()
    } else {
        format!(
            "；已跳过非 completion 类型赋值 [{}]，目标 complete_ty=t{}",
            skipped_type_mismatches.join(", "),
            complete_ty.as_u32()
        )
    };
    Err(invalid_handle_dispatch_contract(
        root_fqn,
        site_id,
        format!(
            "non-Unit {context} state st{} 缺少 completion payload source{}",
            state_id.as_u32(),
            skipped
        ),
    ))
}

fn same_completion_payload_source_ignoring_span(
    left: &LateLoweredCompletionPayloadSource,
    right: &LateLoweredCompletionPayloadSource,
) -> bool {
    match (left, right) {
        (
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: left_ty,
            },
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredCompletionPayloadSource::Operand(left),
            LateLoweredCompletionPayloadSource::Operand(right),
        ) => left.source_ty() == right.source_ty() && left.value() == right.value(),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_handle_state_region_entries(
    root_fqn: &str,
    site_id: SiteId,
    state_graph: &LateLoweredStateGraph,
    dispatch_state: StateId,
    body_state: StateId,
    handled_arms: &[LateLoweredHandleArmDispatch],
    finally_state: Option<StateId>,
    exit_state: StateId,
) -> Result<Vec<LateLoweredHandleStateRegionEntry>, EffectLoweringError> {
    let mut memberships = BTreeMap::<StateId, LateLoweredHandleStateRegion>::new();
    insert_handle_state_region(
        root_fqn,
        site_id,
        &mut memberships,
        dispatch_state,
        LateLoweredHandleStateRegion::Dispatch,
    )?;
    insert_handle_state_region(
        root_fqn,
        site_id,
        &mut memberships,
        exit_state,
        LateLoweredHandleStateRegion::Exit,
    )?;

    let mut stop_states = BTreeSet::from([dispatch_state, exit_state]);
    stop_states.extend(
        handled_arms
            .iter()
            .map(LateLoweredHandleArmDispatch::arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    for state_id in
        collect_handle_region_states(root_fqn, site_id, state_graph, body_state, &stop_states)?
    {
        insert_handle_state_region(
            root_fqn,
            site_id,
            &mut memberships,
            state_id,
            LateLoweredHandleStateRegion::Body,
        )?;
    }

    for arm in handled_arms {
        let mut arm_stops = stop_states.clone();
        arm_stops.remove(&arm.arm_state());
        let region = LateLoweredHandleStateRegion::Arm {
            handled_case: arm.handled_case(),
            arm_ordinal: arm.arm_ordinal(),
        };
        for state_id in collect_handle_region_states(
            root_fqn,
            site_id,
            state_graph,
            arm.arm_state(),
            &arm_stops,
        )? {
            insert_handle_state_region(root_fqn, site_id, &mut memberships, state_id, region)?;
        }
    }

    if let Some(finally_state) = finally_state {
        let mut finally_stops = stop_states;
        finally_stops.remove(&finally_state);
        for state_id in collect_handle_region_states(
            root_fqn,
            site_id,
            state_graph,
            finally_state,
            &finally_stops,
        )? {
            insert_handle_state_region(
                root_fqn,
                site_id,
                &mut memberships,
                state_id,
                LateLoweredHandleStateRegion::Finally,
            )?;
        }
    }

    Ok(memberships
        .into_iter()
        .map(|(state_id, region)| LateLoweredHandleStateRegionEntry::new(state_id, region))
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn build_handle_boundary_routings(
    root_fqn: &str,
    site_id: SiteId,
    state_regions: &[LateLoweredHandleStateRegionEntry],
    handled_arms: &[LateLoweredHandleArmDispatch],
    body_outward_cases: &[CaseTag],
    finally_outward_cases: &[CaseTag],
    outward_emissions: &[LateLoweredStepCaseEmission],
    pending_completions: &[LateLoweredHandlePendingCompletion],
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredHandleBoundaryRouting>, EffectLoweringError> {
    let mut regions_by_state = BTreeMap::new();
    for entry in state_regions {
        if regions_by_state
            .insert(entry.state_id(), entry.region())
            .is_some()
        {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "state st{} 在 region contract 中重复发布",
                    entry.state_id().as_u32()
                ),
            ));
        }
    }
    let handled_arms_by_case = handled_arms
        .iter()
        .map(|arm| (arm.handled_case(), arm))
        .collect::<BTreeMap<_, _>>();
    let body_outward_cases = body_outward_cases.iter().copied().collect::<BTreeSet<_>>();
    let finally_outward_cases = finally_outward_cases
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let outward_emission_cases = outward_emissions
        .iter()
        .map(LateLoweredStepCaseEmission::case_tag)
        .collect::<BTreeSet<_>>();
    let pending_outward_cases = pending_completions
        .iter()
        .filter_map(|pending| match pending {
            LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => {
                Some((*case_tag, *pending))
            }
            LateLoweredHandlePendingCompletion::ContinueToExit
            | LateLoweredHandlePendingCompletion::ReturnFromFunction => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut routes = Vec::new();

    for boundary in boundary_map.entries() {
        let owner_region = regions_by_state
            .get(&boundary.owner_state())
            .copied()
            .unwrap_or(LateLoweredHandleStateRegion::OutsideHandle);
        if matches!(
            owner_region,
            LateLoweredHandleStateRegion::OutsideHandle | LateLoweredHandleStateRegion::Exit
        ) {
            continue;
        }
        if matches!(owner_region, LateLoweredHandleStateRegion::Dispatch)
            && !matches!(
                boundary.source(),
                LateLoweredBoundarySource::Site {
                    site_id: boundary_site,
                    kind: BoundarySiteKind::Handle,
                } if boundary_site == site_id
            )
        {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "dispatch state st{} 上的 boundary bd{} 不是当前 handle site 的 published Handle boundary：source={:?}",
                    boundary.owner_state().as_u32(),
                    boundary.boundary_id().as_u32(),
                    boundary.source(),
                ),
            ));
        }

        let case_tags = collect_handle_boundary_case_tags(root_fqn, site_id, boundary)?;
        let case_routings = case_tags
            .into_iter()
            .map(|case_tag| {
                route_handle_boundary_case(
                    root_fqn,
                    site_id,
                    boundary,
                    owner_region,
                    case_tag,
                    &handled_arms_by_case,
                    &body_outward_cases,
                    &finally_outward_cases,
                    &outward_emission_cases,
                    &pending_outward_cases,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        routes.push(LateLoweredHandleBoundaryRouting::new(
            boundary.boundary_id(),
            boundary.owner_state(),
            owner_region,
            boundary.resume_state(),
            case_routings,
        ));
    }

    Ok(routes)
}

fn build_handle_pending_payload_transports(
    root_fqn: &str,
    site_id: SiteId,
    pending_completions: &[LateLoweredHandlePendingCompletion],
    outward_emissions: &[LateLoweredStepCaseEmission],
    frame_schema: &LateLoweredFrameSchema,
) -> Result<Vec<LateLoweredHandlePendingPayloadTransport>, EffectLoweringError> {
    let mut transports = Vec::new();
    let mut seen = BTreeSet::new();

    for completion in pending_completions {
        let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) = completion else {
            continue;
        };
        if !seen.insert(*completion) {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "重复发布 pending payload transport {:?}，无法保持 cleanup/finally payload contract 唯一",
                    completion
                ),
            ));
        }
        let emission = outward_emissions
            .iter()
            .find(|emission| emission.case_tag() == *case_tag)
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "pending completion c{} 缺少 outward emission，无法发布 cleanup/finally payload transport",
                        case_tag.as_u32()
                    ),
                )
            })?;
        let slot = frame_schema
            .slot_for_kind(crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                site_id,
                case_tag: *case_tag,
            })
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "pending completion c{} 缺少 HandlePendingPayload frame slot，无法发布 cleanup/finally payload transport",
                        case_tag.as_u32()
                    ),
                )
            })?;
        if slot.ty() != emission.payload_tuple_ty() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "pending completion c{} 的 payload transport frame slot fs{} 类型漂移：slot=t{}，outward emission=t{}",
                    case_tag.as_u32(),
                    slot.slot_id().as_u32(),
                    slot.ty().as_u32(),
                    emission.payload_tuple_ty().as_u32(),
                ),
            ));
        }
        transports.push(LateLoweredHandlePendingPayloadTransport::new(
            *completion,
            emission.payload_tuple_ty(),
            slot.slot_id(),
        ));
    }

    Ok(transports)
}

fn collect_handle_region_states(
    root_fqn: &str,
    site_id: SiteId,
    state_graph: &LateLoweredStateGraph,
    entry_state: StateId,
    stop_states: &BTreeSet<StateId>,
) -> Result<BTreeSet<StateId>, EffectLoweringError> {
    if state_graph.state(entry_state).is_none() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "HandleDispatch region root st{} 不存在于 state graph 中",
                entry_state.as_u32()
            ),
        ));
    }

    let mut visited = BTreeSet::new();
    let mut worklist = vec![entry_state];
    while let Some(state_id) = worklist.pop() {
        if stop_states.contains(&state_id) || !visited.insert(state_id) {
            continue;
        }
        let state = state_graph.state(state_id).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "HandleDispatch region 遍历命中了不存在的 state st{}",
                    state_id.as_u32()
                ),
            )
        })?;
        worklist.extend(state.successors().iter().rev().copied());
    }

    Ok(visited)
}

fn insert_handle_state_region(
    root_fqn: &str,
    site_id: SiteId,
    memberships: &mut BTreeMap<StateId, LateLoweredHandleStateRegion>,
    state_id: StateId,
    region: LateLoweredHandleStateRegion,
) -> Result<(), EffectLoweringError> {
    match memberships.insert(state_id, region) {
        Some(existing) if existing != region => Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "state st{} 在 HandleDispatch region contract 中同时归属于 {:?} 和 {:?}",
                state_id.as_u32(),
                existing,
                region,
            ),
        )),
        Some(_) | None => Ok(()),
    }
}

fn collect_handle_boundary_case_tags(
    root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Result<Vec<CaseTag>, EffectLoweringError> {
    let mut tags = BTreeSet::new();
    let lowering = boundary.lowering().ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "boundary bd{} 缺少 lowering，无法发布 handle boundary routing contract",
                boundary.boundary_id().as_u32()
            ),
        )
    })?;
    let case_iter: Vec<CaseTag> = match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        LateLoweredBoundaryLowering::ClassCtor(lowering) => lowering
            .emitted_steps()
            .iter()
            .map(LateLoweredStepCaseEmission::case_tag)
            .collect(),
        LateLoweredBoundaryLowering::Perform(lowering) => vec![lowering.emitted_step().case_tag()],
        LateLoweredBoundaryLowering::Resume(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        LateLoweredBoundaryLowering::Handle(lowering) => lowering
            .outward_emissions()
            .iter()
            .map(LateLoweredStepCaseEmission::case_tag)
            .collect(),
    };
    for case_tag in case_iter {
        if !tags.insert(case_tag) {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 重复发布 outward case c{}，无法生成稳定 routing contract",
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                ),
            ));
        }
    }
    Ok(tags.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn route_handle_boundary_case(
    root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    owner_region: LateLoweredHandleStateRegion,
    case_tag: CaseTag,
    handled_arms_by_case: &BTreeMap<CaseTag, &LateLoweredHandleArmDispatch>,
    body_outward_cases: &BTreeSet<CaseTag>,
    finally_outward_cases: &BTreeSet<CaseTag>,
    outward_emission_cases: &BTreeSet<CaseTag>,
    pending_outward_cases: &BTreeMap<CaseTag, LateLoweredHandlePendingCompletion>,
) -> Result<LateLoweredHandleBoundaryCaseRouting, EffectLoweringError> {
    let action = match owner_region {
        LateLoweredHandleStateRegion::Body => {
            if let Some(arm) = handled_arms_by_case.get(&case_tag) {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state: arm.arm_state(),
                    arm_ordinal: arm.arm_ordinal(),
                    continuation_resume_state: boundary.resume_state(),
                }
            } else if body_outward_cases.contains(&case_tag) {
                pending_outward_cases.get(&case_tag).copied().map_or(
                    LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                    |completion| LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                        completion,
                    },
                )
            } else if finally_outward_cases.contains(&case_tag) {
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
            } else {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "body region 的 boundary bd{} 发布了未声明的 outward case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
        }
        LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => {
            let arm = handled_arms_by_case.get(&handled_case).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region ordinal {} handled case c{} 缺少 published handled-arm contract",
                        arm_ordinal,
                        handled_case.as_u32(),
                    ),
                )
            })?;
            if arm.arm_ordinal() != arm_ordinal {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region ordinal {} 与 handled case c{} 的 published arm ordinal {} 不一致",
                        arm_ordinal,
                        handled_case.as_u32(),
                        arm.arm_ordinal(),
                    ),
                ));
            }
            if !arm.arm_outward_cases().contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region(c{}, ordinal={}) 的 boundary bd{} 发布了未声明的 outward case c{}",
                        handled_case.as_u32(),
                        arm_ordinal,
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            pending_outward_cases.get(&case_tag).copied().map_or(
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                |completion| LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                    completion,
                },
            )
        }
        LateLoweredHandleStateRegion::Finally => {
            if !finally_outward_cases.contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "finally region 的 boundary bd{} 发布了未声明的 outward case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        LateLoweredHandleStateRegion::Dispatch => {
            if !outward_emission_cases.contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "dispatch region 的 handle boundary bd{} 发布了未声明的 outward emission case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        LateLoweredHandleStateRegion::Exit | LateLoweredHandleStateRegion::OutsideHandle => {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 的 owner state st{} 不在当前 HandleDispatch published region 内，却尝试生成 routing",
                    boundary.boundary_id().as_u32(),
                    boundary.owner_state().as_u32(),
                ),
            ));
        }
    };

    Ok(LateLoweredHandleBoundaryCaseRouting::new(case_tag, action))
}

fn lookup_handle_arms<'a>(
    root_fqn: &str,
    body: &'a Body,
    site_id: SiteId,
) -> Result<&'a [crate::mir::HandlerArm], EffectLoweringError> {
    let mut found = None;
    for block in &body.blocks {
        let crate::mir::TerminatorKind::Handle {
            site_id: handle_site,
            arms,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if *handle_site != site_id {
            continue;
        }
        if found.replace(arms.as_slice()).is_some() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                "canonical MIR 中同一 handle site 重复发布多个 Handle terminator".to_string(),
            ));
        }
    }
    found.ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            "缺少对应的 canonical MIR Handle terminator，无法发布 arm binder contract".to_string(),
        )
    })
}

pub(crate) fn materialize_resume_payload_bindings(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredResumePayloadBinding>, EffectLoweringError> {
    let (resume_boundaries, _) = paired_resume_boundaries(boundary_map);
    let mut bindings_by_boundary = BTreeMap::<BoundaryId, LateLoweredResumePayloadBinding>::new();
    let mut bindings_by_state = BTreeMap::<StateId, LateLoweredResumePayloadBinding>::new();

    for boundary in boundary_map.entries() {
        let Some(binding) = (match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Call,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Call(lowering)),
            ) => Some(build_resume_payload_binding_from_result_local(
                root_fqn,
                frame_schema,
                boundary,
                "Call",
                lowering.result_local(),
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Perform,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Perform(_)),
            ) => Some(build_resume_payload_binding_from_boundary_result_slot(
                root_fqn,
                frame_schema,
                boundary,
                "Perform",
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Resume,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Resume(lowering)),
            ) => Some(build_resume_payload_binding_from_result_local(
                root_fqn,
                frame_schema,
                boundary,
                "Resume",
                lowering.result_local(),
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Handle,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Handle(_)),
            )
            | (
                LateLoweredBoundarySource::RuntimeError { .. },
                Some(LateLoweredBoundaryLowering::RuntimeError(_)),
            ) => None,
            _ => None,
        }) else {
            continue;
        };

        insert_resume_payload_binding(
            root_fqn,
            &mut bindings_by_boundary,
            &mut bindings_by_state,
            binding,
        )?;
    }

    for boundary in boundary_map.entries() {
        let origin_site = match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::RuntimeError { origin_site },
                Some(LateLoweredBoundaryLowering::RuntimeError(_)),
            ) => origin_site,
            _ => continue,
        };
        let paired_resume_boundary = resume_boundaries.get(&origin_site).ok_or_else(|| {
            invalid_resume_payload_binding_contract(
                root_fqn,
                boundary.boundary_id(),
                format!(
                    "runtime-error route origin=site{} 缺少配对的 resume boundary，无法继承 resumed local/home binding",
                    origin_site.as_u32(),
                ),
            )
        })?;
        let paired_binding = bindings_by_boundary.get(paired_resume_boundary).copied().ok_or_else(
            || {
                invalid_resume_payload_binding_contract(
                    root_fqn,
                    boundary.boundary_id(),
                    format!(
                        "paired resume boundary bd{} 缺少 resumed local/home binding，无法为 runtime-error route 继承 authoritative consumer",
                        paired_resume_boundary.as_u32(),
                    ),
                )
            },
        )?;
        let binding = LateLoweredResumePayloadBinding::new(
            boundary.boundary_id(),
            boundary.resume_state(),
            paired_binding.consumer_local(),
            paired_binding.consumer_frame_slot(),
        );
        insert_resume_payload_binding(
            root_fqn,
            &mut bindings_by_boundary,
            &mut bindings_by_state,
            binding,
        )?;
    }

    Ok(boundary_map
        .entries()
        .iter()
        .filter_map(|boundary| bindings_by_boundary.get(&boundary.boundary_id()).copied())
        .collect())
}

pub(crate) fn materialize_completion_payload_bindings(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    types: &TypeStore,
) -> Result<Vec<LateLoweredCompletionPayloadBinding>, EffectLoweringError> {
    let mut bindings = BTreeMap::<StateId, LateLoweredCompletionPayloadBinding>::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } = state.terminator()
        else {
            continue;
        };
        if *complete_state != state_graph.complete_state() {
            return Err(invalid_completion_payload_contract(
                root_fqn,
                format!(
                    "return state st{} 指向 st{}，但 callable complete_state 是 st{}",
                    state.state_id().as_u32(),
                    complete_state.as_u32(),
                    state_graph.complete_state().as_u32(),
                ),
            ));
        }
        validate_completion_payload_source(root_fqn, step_type, payload_source, types)?;
        let payload_frame_slot =
            completion_payload_frame_slot(root_fqn, frame_schema, payload_source)?;
        let binding = LateLoweredCompletionPayloadBinding::new(
            state.state_id(),
            *complete_state,
            payload_source.clone(),
            payload_frame_slot,
        );
        if bindings.insert(state.state_id(), binding).is_some() {
            return Err(invalid_completion_payload_contract(
                root_fqn,
                format!(
                    "return state st{} 重复发布 completion payload source",
                    state.state_id().as_u32(),
                ),
            ));
        }
    }
    Ok(bindings.into_values().collect())
}

fn validate_completion_payload_source(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    payload_source: &LateLoweredCompletionPayloadSource,
    types: &TypeStore,
) -> Result<(), EffectLoweringError> {
    if payload_source.source_ty() != step_type.complete_ty() {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "payload source type t{} 与 StepSchema s{} complete_ty t{} 不一致",
                payload_source.source_ty().as_u32(),
                step_type.step_schema().as_u32(),
                step_type.complete_ty().as_u32(),
            ),
        ));
    }
    if payload_source.is_unit() && !is_unit_type(types, step_type.complete_ty()) {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "non-Unit complete_ty t{} 不能发布 Unit completion payload source",
                step_type.complete_ty().as_u32(),
            ),
        ));
    }
    Ok(())
}

fn completion_payload_frame_slot(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    payload_source: &LateLoweredCompletionPayloadSource,
) -> Result<Option<crate::effect_lowered::ir::FrameSlotId>, EffectLoweringError> {
    let Some(source) = payload_source.operand_source() else {
        return Ok(None);
    };
    let crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) = source.value()
    else {
        return Ok(None);
    };
    let Some(slot_id) = find_frame_slot_for_local(frame_schema, *local) else {
        return Ok(None);
    };
    let slot = frame_schema
        .slots()
        .iter()
        .find(|slot| slot.slot_id() == slot_id)
        .expect("frame slot id returned by find_frame_slot_for_local should exist");
    if slot.ty() != source.source_ty() {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "completion payload local{} 的 home slot{} 类型为 t{}，但 payload source type 为 t{}",
                local.as_u32(),
                slot.slot_id().as_u32(),
                slot.ty().as_u32(),
                source.source_ty().as_u32(),
            ),
        ));
    }
    Ok(Some(slot_id))
}

fn invalid_completion_payload_contract(root_fqn: &str, detail: String) -> EffectLoweringError {
    EffectLoweringError::InvalidCompletionPayloadContract {
        root_fqn: root_fqn.to_string(),
        detail,
    }
}

pub(crate) fn materialize_source_statement_classifications(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredSourceStatementClassification>, EffectLoweringError> {
    let boundary_statement_anchors = collect_boundary_statement_anchors(root_fqn, boundary_map)?;
    let handle_binder_locals = collect_handle_binder_locals(state_graph);
    let mut classifications = Vec::new();
    let mut seen_statements = BTreeSet::<(BasicBlockId, u32)>::new();
    let mut matched_boundary_statement_anchors = BTreeSet::<BoundaryId>::new();

    for state in state_graph.states() {
        for &source_slice in state.source_slices() {
            let block = body
                .blocks
                .get(source_slice.block_id().as_u32() as usize)
                .ok_or_else(|| {
                    invalid_source_slice_classification_contract(
                        root_fqn,
                        format!(
                            "state st{} source slice 指向缺失的 canonical MIR block bb{}",
                            state.state_id().as_u32(),
                            source_slice.block_id().as_u32(),
                        ),
                    )
                })?;
            let start = source_slice.start_statement_index() as usize;
            let end = source_slice.end_statement_index() as usize;
            if start > end || end > block.stmts.len() {
                return Err(invalid_source_slice_classification_contract(
                    root_fqn,
                    format!(
                        "state st{} source slice [{}..{}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                        state.state_id().as_u32(),
                        source_slice.start_statement_index(),
                        source_slice.end_statement_index(),
                        source_slice.block_id().as_u32(),
                        block.stmts.len(),
                    ),
                ));
            }

            for stmt_index in
                source_slice.start_statement_index()..source_slice.end_statement_index()
            {
                let key = (source_slice.block_id(), stmt_index);
                if !seen_statements.insert(key) {
                    return Err(invalid_source_slice_classification_contract(
                        root_fqn,
                        format!(
                            "source-slice statement bb{} stmt{} 被多个 state 覆盖，classification contract 不再唯一",
                            source_slice.block_id().as_u32(),
                            stmt_index,
                        ),
                    ));
                }
                let stmt = &block.stmts[stmt_index as usize];
                let kind = classify_source_statement(
                    state.state_id(),
                    body,
                    source_slice,
                    stmt_index,
                    stmt,
                    frame_schema,
                    &boundary_statement_anchors,
                    &handle_binder_locals,
                    &mut matched_boundary_statement_anchors,
                );
                classifications.push(LateLoweredSourceStatementClassification::new(
                    source_slice,
                    stmt_index,
                    kind,
                ));
            }
        }
    }

    for (key, boundary_id) in &boundary_statement_anchors {
        if !matched_boundary_statement_anchors.contains(boundary_id) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "boundary bd{} 的 statement anchor bb{} stmt{} 未落入任何 source-slice classification",
                    boundary_id.as_u32(),
                    key.0.as_u32(),
                    key.1,
                ),
            ));
        }
    }
    Ok(classifications)
}

fn collect_boundary_statement_anchors(
    root_fqn: &str,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<BTreeMap<(BasicBlockId, u32), BoundaryId>, EffectLoweringError> {
    let mut anchors = BTreeMap::new();
    for boundary in boundary_map.entries() {
        let Some(LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        }) = boundary_source_consumption(boundary)
        else {
            continue;
        };
        let key = (source_slice.block_id(), statement_index);
        if let Some(existing) = anchors.insert(key, boundary.boundary_id()) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "bb{} stmt{} 同时被 boundary bd{} 与 bd{} 声明为 consumed anchor",
                    source_slice.block_id().as_u32(),
                    statement_index,
                    existing.as_u32(),
                    boundary.boundary_id().as_u32(),
                ),
            ));
        }
    }
    Ok(anchors)
}

fn boundary_source_consumption(
    boundary: &LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.source_consumption()),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) | LateLoweredBoundaryLowering::Handle(_) => {
            None
        }
    }
}

fn collect_handle_binder_locals(
    state_graph: &LateLoweredStateGraph,
) -> BTreeMap<(StateId, LocalId), SiteId> {
    let mut locals = BTreeMap::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::HandleDispatch {
            site_id, contract, ..
        } = state.terminator()
        else {
            continue;
        };
        for arm in contract.handled_arms() {
            for binder in arm.payload_binders() {
                locals.insert((arm.arm_state(), binder.local()), *site_id);
            }
            if let Some(binder) = arm.continuation_binder() {
                locals.insert((arm.arm_state(), binder.local()), *site_id);
            }
        }
    }
    locals
}

#[allow(clippy::too_many_arguments)]
fn classify_source_statement(
    state_id: StateId,
    body: &Body,
    source_slice: LateLoweredStateSlice,
    stmt_index: u32,
    stmt: &crate::mir::Statement,
    frame_schema: &LateLoweredFrameSchema,
    boundary_statement_anchors: &BTreeMap<(BasicBlockId, u32), BoundaryId>,
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    matched_boundary_statement_anchors: &mut BTreeSet<BoundaryId>,
) -> LateLoweredSourceStatementClassificationKind {
    let key = (source_slice.block_id(), stmt_index);
    if let Some(boundary_id) = boundary_statement_anchors.get(&key).copied() {
        matched_boundary_statement_anchors.insert(boundary_id);
        return LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
            boundary_id,
        };
    }

    if let Some(binding) = resume_payload_injection_binding(frame_schema, stmt) {
        return LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
            boundary_id: binding.boundary_id(),
            resume_state: binding.resume_state(),
            consumer_local: binding.consumer_local(),
        };
    }

    if let Some(site_id) = handle_binder_statement(handle_binder_locals, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
            site_id,
            state_id,
        };
    }

    if let Some(binding) = boundary_result_injection_binding(frame_schema, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
            boundary_id: binding.boundary_id(),
            resume_state: binding.resume_state(),
            result_local: binding.consumer_local(),
        };
    }

    if let Some(binding) = completion_payload_injection_binding(frame_schema, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
            return_state: binding.return_state(),
            complete_state: binding.complete_state(),
        };
    }

    classify_effect_neutral_source_statement(body, stmt)
}

fn resume_payload_injection_binding(
    frame_schema: &LateLoweredFrameSchema,
    stmt: &crate::mir::Statement,
) -> Option<LateLoweredResumePayloadBinding> {
    let StatementKind::Assign {
        target,
        value: Rvalue::PerformResult { .. },
    } = &stmt.kind
    else {
        return None;
    };
    let mut matches = frame_schema
        .resume_payload_bindings()
        .iter()
        .copied()
        .filter(|binding| binding.consumer_local() == *target);
    let binding = matches.next()?;
    matches.next().is_none().then_some(binding)
}

fn boundary_result_injection_binding(
    frame_schema: &LateLoweredFrameSchema,
    state_id: StateId,
    stmt: &crate::mir::Statement,
) -> Option<LateLoweredResumePayloadBinding> {
    let StatementKind::Assign { target, .. } = &stmt.kind else {
        return None;
    };
    frame_schema
        .resume_payload_bindings()
        .iter()
        .copied()
        .find(|binding| binding.resume_state() == state_id && binding.consumer_local() == *target)
}

fn completion_payload_injection_binding<'a>(
    frame_schema: &'a LateLoweredFrameSchema,
    state_id: StateId,
    stmt: &crate::mir::Statement,
) -> Option<&'a LateLoweredCompletionPayloadBinding> {
    let binding = frame_schema.completion_payload_binding_for_state(state_id)?;
    let crate::effect_lowered::ir::LateLoweredCompletionPayloadSource::Operand(source) =
        binding.payload_source()
    else {
        return None;
    };
    let crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) = source.value()
    else {
        return None;
    };
    matches!(
        &stmt.kind,
        StatementKind::Assign { target, .. } if *target == *local
    )
    .then_some(binding)
}

fn handle_binder_statement(
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    state_id: StateId,
    stmt: &crate::mir::Statement,
) -> Option<SiteId> {
    let StatementKind::Assign { target, .. } = &stmt.kind else {
        return None;
    };
    handle_binder_locals.get(&(state_id, *target)).copied()
}

fn classify_effect_neutral_source_statement(
    body: &Body,
    stmt: &crate::mir::Statement,
) -> LateLoweredSourceStatementClassificationKind {
    match &stmt.kind {
        StatementKind::Nop => LateLoweredSourceStatementClassificationKind::ElidedUnreachable,
        StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        StatementKind::Assign { target, value } => {
            if matches!(value, Rvalue::Todo("missing expr"))
                && local_is_only_value_member_namespace_receiver(body, *target)
            {
                return LateLoweredSourceStatementClassificationKind::EffectNeutralValue;
            }
            classify_effect_neutral_rvalue(value)
        }
        StatementKind::Todo(reason) => {
            LateLoweredSourceStatementClassificationKind::Unsupported { reason }
        }
    }
}

fn classify_effect_neutral_rvalue(value: &Rvalue) -> LateLoweredSourceStatementClassificationKind {
    match value {
        Rvalue::Use(_)
        | Rvalue::TopLevelRef(_)
        | Rvalue::Unary { .. }
        | Rvalue::Binary { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::EnumVariant { .. }
        | Rvalue::ClassCtor { .. }
        | Rvalue::Call { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        Rvalue::UnresolvedName { .. } => {
            LateLoweredSourceStatementClassificationKind::Unsupported {
                reason: "unresolved name requires earlier lowering",
            }
        }
        Rvalue::PerformResult { .. } => LateLoweredSourceStatementClassificationKind::Unsupported {
            reason: "perform result requires published resume payload injection",
        },
        Rvalue::Todo(reason) => {
            LateLoweredSourceStatementClassificationKind::Unsupported { reason }
        }
    }
}

fn local_is_only_value_member_namespace_receiver(body: &Body, local: LocalId) -> bool {
    let mut saw_value_member = false;
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
            };
            if *target == local {
                continue;
            }
            if let Rvalue::MemberAccess {
                receiver: Operand::Local(receiver),
                member,
                ..
            } = value
                && *receiver == local
                && matches!(member.resolved, Some(MemberTarget::Value { .. }))
            {
                saw_value_member = true;
                continue;
            }
            if rvalue_mentions_local(value, local) {
                return false;
            }
        }
    }
    saw_value_member
}

fn operand_mentions_local(operand: &Operand, local: LocalId) -> bool {
    matches!(operand, Operand::Local(found) if *found == local)
}

fn call_args_mention_local(args: &[CallArg], local: LocalId) -> bool {
    args.iter()
        .any(|arg| operand_mentions_local(&arg.value, local))
}

fn call_kind_mentions_local(kind: &CallKind, local: LocalId) -> bool {
    match kind {
        CallKind::Direct { .. } => false,
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => {
            operand_mentions_local(callee, local)
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            operand_mentions_local(receiver, local)
        }
        CallKind::Resume { continuation, .. } => operand_mentions_local(continuation, local),
    }
}

fn rvalue_mentions_local(value: &Rvalue, local: LocalId) -> bool {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Unary { operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::CaptureBoxNew { value: operand }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
        }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        }
        | Rvalue::MakeClosure { env: operand, .. } => operand_mentions_local(operand, local),
        Rvalue::Binary { lhs, rhs, .. } => {
            operand_mentions_local(lhs, local) || operand_mentions_local(rhs, local)
        }
        Rvalue::MemberAccess { receiver, .. } => operand_mentions_local(receiver, local),
        Rvalue::EnumVariant { args, .. } | Rvalue::ClassCtor { args, .. } => {
            call_args_mention_local(args, local)
        }
        Rvalue::Call { kind, args, .. } => {
            call_kind_mentions_local(kind, local) || call_args_mention_local(args, local)
        }
        Rvalue::MakeTuple { elements } => elements
            .iter()
            .any(|operand| operand_mentions_local(operand, local)),
        Rvalue::StructLit { fields } => fields
            .iter()
            .any(|field| operand_mentions_local(&field.value, local)),
        Rvalue::InterpolatedString { parts, .. } => parts.iter().any(|part| match part {
            crate::mir::InterpolatedStringPart::Text { .. } => false,
            crate::mir::InterpolatedStringPart::Expr { value, .. } => {
                operand_mentions_local(value, local)
            }
        }),
        Rvalue::CaptureBoxSet { box_operand, value } => {
            operand_mentions_local(box_operand, local) || operand_mentions_local(value, local)
        }
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

fn invalid_source_slice_classification_contract(
    root_fqn: &str,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidSourceSliceClassificationContract {
        root_fqn: root_fqn.to_string(),
        detail,
    }
}

fn is_unit_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Value(ValueTypeKind::Unit))
}

fn is_any_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Any))
}

fn build_resume_payload_binding_from_result_local(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary: &LateLoweredBoundary,
    kind: &'static str,
    result_local: LocalId,
) -> Result<LateLoweredResumePayloadBinding, EffectLoweringError> {
    let boundary_result_slot = published_boundary_result_slot(frame_schema, boundary.boundary_id());
    if let Some((slot_local, _)) = boundary_result_slot
        && slot_local != result_local
    {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            boundary.boundary_id(),
            format!(
                "{kind} boundary 的 BoundaryResult slot 绑定到了 local{}，但 published result local 为 local{}",
                slot_local.as_u32(),
                result_local.as_u32(),
            ),
        ));
    }
    let consumer_frame_slot = boundary_result_slot
        .map(|(_, slot_id)| slot_id)
        .or_else(|| find_frame_slot_for_local(frame_schema, result_local));
    Ok(LateLoweredResumePayloadBinding::new(
        boundary.boundary_id(),
        boundary.resume_state(),
        result_local,
        consumer_frame_slot,
    ))
}

fn build_resume_payload_binding_from_boundary_result_slot(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary: &LateLoweredBoundary,
    kind: &'static str,
) -> Result<LateLoweredResumePayloadBinding, EffectLoweringError> {
    let Some((consumer_local, consumer_frame_slot)) =
        published_boundary_result_slot(frame_schema, boundary.boundary_id())
    else {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            boundary.boundary_id(),
            format!(
                "{kind} boundary 缺少 BoundaryResult frame slot，无法 authoritative 发布 resumed local/home",
            ),
        ));
    };
    Ok(LateLoweredResumePayloadBinding::new(
        boundary.boundary_id(),
        boundary.resume_state(),
        consumer_local,
        Some(consumer_frame_slot),
    ))
}

fn insert_resume_payload_binding(
    root_fqn: &str,
    bindings_by_boundary: &mut BTreeMap<BoundaryId, LateLoweredResumePayloadBinding>,
    bindings_by_state: &mut BTreeMap<StateId, LateLoweredResumePayloadBinding>,
    binding: LateLoweredResumePayloadBinding,
) -> Result<(), EffectLoweringError> {
    if bindings_by_boundary
        .insert(binding.boundary_id(), binding)
        .is_some()
    {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            binding.boundary_id(),
            "重复发布多个 resumed local/home binding".to_string(),
        ));
    }
    match bindings_by_state.get(&binding.resume_state()) {
        Some(existing)
            if existing.consumer_local() == binding.consumer_local()
                && existing.consumer_frame_slot() == binding.consumer_frame_slot() => {}
        Some(existing) => {
            return Err(invalid_resume_payload_binding_contract(
                root_fqn,
                binding.boundary_id(),
                format!(
                    "resume state st{} 同时映射到不兼容的 resumed local/home：已发布 {}，当前尝试发布 {}",
                    binding.resume_state().as_u32(),
                    render_resume_payload_binding_target(
                        existing.consumer_local(),
                        existing.consumer_frame_slot(),
                    ),
                    render_resume_payload_binding_target(
                        binding.consumer_local(),
                        binding.consumer_frame_slot(),
                    ),
                ),
            ));
        }
        None => {
            bindings_by_state.insert(binding.resume_state(), binding);
        }
    }
    Ok(())
}

fn published_boundary_result_slot(
    frame_schema: &LateLoweredFrameSchema,
    boundary_id: BoundaryId,
) -> Option<(LocalId, crate::effect_lowered::ir::FrameSlotId)> {
    frame_schema
        .slots()
        .iter()
        .find_map(|slot| match slot.kind() {
            LateLoweredFrameSlotKind::BoundaryResult { boundary, local }
                if boundary == boundary_id =>
            {
                Some((local, slot.slot_id()))
            }
            _ => None,
        })
}

fn render_resume_payload_binding_target(
    consumer_local: LocalId,
    consumer_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
) -> String {
    match consumer_frame_slot {
        Some(slot_id) => format!(
            "local{} / slot{}",
            consumer_local.as_u32(),
            slot_id.as_u32()
        ),
        None => format!("local{} / <no-frame-slot>", consumer_local.as_u32()),
    }
}

fn invalid_resume_payload_binding_contract(
    root_fqn: &str,
    boundary_id: BoundaryId,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidResumePayloadBindingContract {
        root_fqn: root_fqn.to_string(),
        boundary_id: boundary_id.as_u32(),
        detail,
    }
}

fn find_frame_slot_for_local(
    frame_schema: &LateLoweredFrameSchema,
    local: LocalId,
) -> Option<crate::effect_lowered::ir::FrameSlotId> {
    frame_schema.slots().iter().find_map(|slot| {
        let slot_local = match slot.kind() {
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(slot_local)
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(slot_local)
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder {
                local: slot_local,
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::BoundaryResult {
                local: slot_local,
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::JoinValue {
                local: slot_local,
                ..
            } => Some(slot_local),
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(_) => None,
        };
        (slot_local == Some(local)).then_some(slot.slot_id())
    })
}

fn find_handle_boundary_lowering<'a>(
    root_fqn: &str,
    site_id: SiteId,
    boundary_ids: &[BoundaryId],
    boundary_map: &'a LateLoweredBoundaryMap,
) -> Result<Option<&'a LateLoweredHandleBoundaryLowering>, EffectLoweringError> {
    let mut lowering = None;
    for boundary_id in boundary_ids {
        let Some(boundary) = boundary_map.boundary(*boundary_id) else {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!("boundary_ids 中引用了不存在的 bd{}", boundary_id.as_u32()),
            ));
        };
        let (boundary_site, handle_lowering) = match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::Site {
                    site_id: boundary_site,
                    kind: BoundarySiteKind::Handle,
                },
                Some(LateLoweredBoundaryLowering::Handle(lowering)),
            ) => (boundary_site, lowering),
            (source, lowering) => {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "boundary bd{} 不是当前 handle site 的 published Handle lowering：source={source:?} lowering={lowering:?}",
                        boundary_id.as_u32(),
                    ),
                ));
            }
        };
        if boundary_site != site_id {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 属于 site{}，但当前 HandleDispatch 属于 site{}",
                    boundary_id.as_u32(),
                    boundary_site.as_u32(),
                    site_id.as_u32(),
                ),
            ));
        }
        if lowering.replace(handle_lowering).is_some() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                "同一 HandleDispatch 绑定了多个 handle boundary lowering".to_string(),
            ));
        }
    }
    Ok(lowering)
}

fn collect_handle_outward_case_tags(facts: &HandleSiteEffectFacts) -> BTreeSet<CaseTag> {
    let mut tags = facts
        .body_outward_cases()
        .tags()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    tags
}

fn invalid_handle_dispatch_contract(
    root_fqn: &str,
    site_id: SiteId,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidHandleDispatchContract {
        root_fqn: root_fqn.to_string(),
        site_id: site_id.as_u32(),
        detail,
    }
}

fn format_case_tag_set(tags: &BTreeSet<CaseTag>) -> String {
    if tags.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        tags.iter()
            .map(|case_tag| format!("c{}", case_tag.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn build_step_type(
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredStepType, EffectLoweringError> {
    Ok(LateLoweredStepType::new(
        step_schema_id,
        step_schema.invoke_args_tuple_ty(),
        step_schema.complete_ty(),
        step_schema.continuation_obj_ty(),
        step_schema
            .cases()
            .iter()
            .map(|case| {
                let continuation_contract =
                    build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
                Result::<_, EffectLoweringError>::Ok(LateLoweredStepCase::new(
                    case.case_tag(),
                    case.concrete_op_key().clone(),
                    case.payload_tuple_ty(),
                    continuation_contract,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn build_resume_interface(
    interface_id: ResumeInterfaceId,
    effect_family: EffectFamilyKey,
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    cases: &[&StepCaseFact],
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredResumeInterface, EffectLoweringError> {
    let mut methods = Vec::with_capacity(cases.len());
    let mut return_step_schema = None;
    for case in cases {
        let continuation_contract =
            build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
        return_step_schema.get_or_insert(continuation_contract.out_step_schema());
        methods.push(LateLoweredResumeMethod::new(
            case.case_tag(),
            case.concrete_op_key().clone(),
            continuation_contract,
        ));
    }
    Ok(LateLoweredResumeInterface::new(
        interface_id,
        effect_family,
        return_step_schema.unwrap_or(step_schema_id),
        methods,
    ))
}

fn build_continuation_contract(
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    case: &StepCaseFact,
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredContinuationContract, EffectLoweringError> {
    let continuation_schema = effect_facts
        .continuation_schemas()
        .get(&case.continuation_schema())
        .ok_or_else(|| EffectLoweringError::MissingContinuationSchema {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
        })?;

    if continuation_schema.out_step_schema() != step_schema_id {
        return Err(EffectLoweringError::ContinuationOutStepSchemaMismatch {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
            out_step_schema: continuation_schema.out_step_schema().as_u32(),
        });
    }

    if continuation_schema.answer_ty() != step_schema.complete_ty() {
        return Err(EffectLoweringError::ContinuationAnswerTyMismatch {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
            answer_ty: continuation_schema.answer_ty().as_u32(),
            complete_ty: step_schema.complete_ty().as_u32(),
        });
    }

    Ok(LateLoweredContinuationContract::new(
        case.continuation_schema(),
        continuation_schema.resume_tuple_ty(),
        continuation_schema.answer_ty(),
        continuation_schema.out_step_schema(),
        continuation_schema.surface_ty(),
    ))
}

fn group_cases_by_effect_family(
    step_schema: &StepSchema,
) -> BTreeMap<EffectFamilyKey, Vec<&StepCaseFact>> {
    let mut grouped = BTreeMap::<EffectFamilyKey, Vec<&StepCaseFact>>::new();
    for case in step_schema.cases() {
        grouped
            .entry(case.concrete_op_key().effect_family().clone())
            .or_default()
            .push(case);
    }
    grouped
}

fn continuation_resume_body(
    impl_plan: ImplPlan,
    case_tag: crate::effect_facts::CaseTag,
) -> LateLoweredContinuationResumeBody {
    match impl_plan {
        ImplPlan::NoOutward => LateLoweredContinuationResumeBody::Unreachable,
        ImplPlan::SingleCase(selected) if selected == case_tag => {
            LateLoweredContinuationResumeBody::ResumeCapturedState {
                repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
            }
        }
        ImplPlan::SingleCase(_) => LateLoweredContinuationResumeBody::Unreachable,
        ImplPlan::CanonicalFull => LateLoweredContinuationResumeBody::ResumeCapturedState {
            repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
        },
    }
}

#[derive(Default)]
struct BoundaryResultLocals {
    call_results: HashMap<SiteId, LocalId>,
}

fn invalid_boundary_operand_contract(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    detail: impl Into<String>,
) -> EffectLoweringError {
    EffectLoweringError::InvalidBoundaryOperandContract {
        root_fqn: root_fqn.to_string(),
        site_id: site_id.as_u32(),
        kind,
        detail: detail.into(),
    }
}

fn expected_source_types_for_carrier(
    types: &TypeStore,
    carrier_ty: crate::ty::TypeId,
    source_count: usize,
) -> Result<Vec<crate::ty::TypeId>, String> {
    match source_count {
        0 => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Vec::new()),
            _ => Err(format!(
                "只有 Unit carrier 才允许 0 个 source，但 published carrier 为 t{}",
                carrier_ty.as_u32(),
            )),
        },
        1 => Ok(vec![carrier_ty]),
        _ => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.len() == source_count => {
                Ok(elements.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => Err(format!(
                "published tuple carrier t{} 期望 {} 个 source，实际为 {source_count}",
                carrier_ty.as_u32(),
                elements.len(),
            )),
            _ => Err(format!(
                "published carrier t{} 期望单一 source，实际数量为 {source_count}",
                carrier_ty.as_u32(),
            )),
        },
    }
}

fn call_kind_matches_facts(kind: &CallKind, facts: &CallSiteEffectFacts) -> bool {
    matches!(
        (kind, facts.kind()),
        (
            CallKind::Direct { .. },
            crate::effect_facts::CallSiteKind::Direct
        ) | (
            CallKind::Closure { .. },
            crate::effect_facts::CallSiteKind::Closure
        ) | (
            CallKind::FunValue { .. },
            crate::effect_facts::CallSiteKind::FunValue
        ) | (
            CallKind::Virtual { .. },
            crate::effect_facts::CallSiteKind::Virtual
        ) | (
            CallKind::Interface { .. },
            crate::effect_facts::CallSiteKind::Interface
        )
    )
}

fn local_decl_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    local: LocalId,
) -> Result<crate::ty::TypeId, EffectLoweringError> {
    body.locals
        .get(local.as_u32() as usize)
        .map(|decl| decl.ty)
        .ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                kind,
                format!("operand 引用了缺失的 local{}", local.as_u32()),
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn operand_source_with_expected_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    types: &TypeStore,
    operand: &Operand,
    expected_ty: crate::ty::TypeId,
    span: Option<crate::span::Span>,
) -> Result<LateLoweredOperandSource, EffectLoweringError> {
    match operand {
        Operand::Local(local) => {
            let local_ty = local_decl_ty(root_fqn, site_id, kind, body, *local)?;
            if local_ty != expected_ty
                && !local_defines_static_member_value_of_type(body, types, *local, expected_ty)
                && !function_value_source_type_compatible(types, local_ty, expected_ty)
            {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    kind,
                    format!(
                        "local{} 的类型为 t{}，但 published operand contract 期望 t{}",
                        local.as_u32(),
                        local_ty.as_u32(),
                        expected_ty.as_u32(),
                    ),
                ));
            }
            Ok(LateLoweredOperandSource::new_local(
                *local,
                expected_ty,
                span,
            ))
        }
        Operand::Const(value) => Ok(LateLoweredOperandSource::new_const(
            value.clone(),
            expected_ty,
            span,
        )),
    }
}

fn function_value_source_type_compatible(
    types: &TypeStore,
    local_ty: TypeId,
    expected_ty: TypeId,
) -> bool {
    let (
        TypeKind::Ref(RefTypeKind::Function(local_fun)),
        TypeKind::Ref(RefTypeKind::Function(expected_fun)),
    ) = (types.kind(local_ty), types.kind(expected_ty))
    else {
        return false;
    };
    local_fun.receiver == expected_fun.receiver
        && local_fun.params == expected_fun.params
        && local_fun.effects == expected_fun.effects
        && local_fun.effects_closed == expected_fun.effects_closed
        && (local_fun.return_ty == expected_fun.return_ty
            || matches!(
                types.kind(local_fun.return_ty),
                TypeKind::Value(ValueTypeKind::Nothing)
            ))
}

fn local_defines_static_member_value_of_type(
    body: &Body,
    types: &TypeStore,
    local: LocalId,
    expected_ty: TypeId,
) -> bool {
    let expected_fqn = match types.kind(expected_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
        _ => return false,
    };
    body.blocks
        .iter()
        .flat_map(|block| &block.stmts)
        .any(|stmt| {
            let StatementKind::Assign {
                target,
                value: Rvalue::MemberAccess { member, .. },
            } = &stmt.kind
            else {
                return false;
            };
            if *target != local {
                return false;
            }
            let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                return false;
            };
            fqn.strip_prefix(expected_fqn)
                .is_some_and(|suffix| suffix.starts_with('.'))
        })
}

fn operand_source_with_inferred_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    operand: &Operand,
    span: Option<crate::span::Span>,
) -> Result<LateLoweredOperandSource, EffectLoweringError> {
    match operand {
        Operand::Local(local) => Ok(LateLoweredOperandSource::new_local(
            *local,
            local_decl_ty(root_fqn, site_id, kind, body, *local)?,
            span,
        )),
        Operand::Const(_) => Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            "当前 boundary contract 无法为 carrier/continuation 常量来源恢复稳定 source_ty",
        )),
    }
}

fn build_ordered_call_arg_sources(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    args: &[CallArg],
    expected_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
) -> Result<Vec<LateLoweredOperandSource>, EffectLoweringError> {
    let expected_components =
        expected_source_types_for_carrier(types, expected_tuple_ty, args.len())
            .map_err(|detail| invalid_boundary_operand_contract(root_fqn, site_id, kind, detail))?;
    if args.len() != expected_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "ordered args 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                args.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    args.iter()
        .zip(expected_components)
        .map(|(arg, expected_ty)| {
            operand_source_with_expected_ty(
                root_fqn,
                site_id,
                kind,
                body,
                types,
                &arg.value,
                expected_ty,
                Some(arg.span),
            )
        })
        .collect()
}

fn expected_source_components_for_carrier(types: &TypeStore, carrier_ty: TypeId) -> Vec<TypeId> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Unit) => Vec::new(),
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.clone(),
        _ => vec![carrier_ty],
    }
}

fn local_assignment(body: &Body, local: LocalId) -> Option<&Rvalue> {
    body.blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| match &stmt.kind {
            StatementKind::Assign { target, value } if *target == local => Some(value),
            _ => None,
        })
}

fn resolve_closure_env_operand<'a>(body: &'a Body, callee: &Operand) -> Option<&'a Operand> {
    let &Operand::Local(mut current) = callee else {
        return None;
    };
    for _ in 0..32 {
        match local_assignment(body, current)? {
            Rvalue::MakeClosure { env, .. } => return Some(env),
            Rvalue::Use(Operand::Local(next)) => current = *next,
            _ => return None,
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn build_known_instance_closure_call_arg_sources(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    types: &TypeStore,
    callee: &Operand,
    args: &[CallArg],
    expected_tuple_ty: TypeId,
) -> Result<Option<Vec<LateLoweredOperandSource>>, EffectLoweringError> {
    let expected_components = expected_source_components_for_carrier(types, expected_tuple_ty);
    let Some(env_operand) = resolve_closure_env_operand(body, callee) else {
        return Ok(None);
    };
    if args.is_empty()
        && let Ok(source) = operand_source_with_expected_ty(
            root_fqn,
            site_id,
            kind,
            body,
            types,
            env_operand,
            expected_tuple_ty,
            None,
        )
    {
        return Ok(Some(vec![source]));
    }
    let decompose_env_tuple = matches!(
        types.kind(expected_tuple_ty),
        TypeKind::Value(ValueTypeKind::Tuple(_))
    );
    let env_sources = match env_operand {
        Operand::Local(local) => match local_assignment(body, *local) {
            Some(Rvalue::MakeTuple { elements }) if decompose_env_tuple => {
                if elements.len() > expected_components.len() {
                    return Err(invalid_boundary_operand_contract(
                        root_fqn,
                        site_id,
                        kind,
                        format!(
                            "closure env component 数量({}) 超过 published invoke carrier t{} 的 component 数量({})",
                            elements.len(),
                            expected_tuple_ty.as_u32(),
                            expected_components.len(),
                        ),
                    ));
                }
                elements
                    .iter()
                    .zip(expected_components.iter().copied())
                    .map(|(element, expected_ty)| {
                        operand_source_with_expected_ty(
                            root_fqn,
                            site_id,
                            kind,
                            body,
                            types,
                            element,
                            expected_ty,
                            None,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            Some(Rvalue::Use(source)) if expected_components.len() == 1 => {
                vec![operand_source_with_expected_ty(
                    root_fqn,
                    site_id,
                    kind,
                    body,
                    types,
                    source,
                    expected_components[0],
                    None,
                )?]
            }
            _ if expected_components.len() == 1 => vec![operand_source_with_expected_ty(
                root_fqn,
                site_id,
                kind,
                body,
                types,
                env_operand,
                expected_components[0],
                None,
            )?],
            _ if expected_components.is_empty() => Vec::new(),
            _ => return Ok(None),
        },
        Operand::Const(_) if expected_components.is_empty() => Vec::new(),
        Operand::Const(_) => return Ok(None),
    };
    let explicit_components = &expected_components[env_sources.len()..];
    if args.len() != explicit_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "closure env component 数量({}) + ordered args 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                env_sources.len(),
                args.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    let mut sources = env_sources;
    for (arg, expected_ty) in args.iter().zip(explicit_components.iter().copied()) {
        sources.push(operand_source_with_expected_ty(
            root_fqn,
            site_id,
            kind,
            body,
            types,
            &arg.value,
            expected_ty,
            Some(arg.span),
        )?);
    }
    Ok(Some(sources))
}

fn build_ordered_perform_payload_sources(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    args: &[PerformArg],
    payload_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
) -> Result<Vec<LateLoweredOperandSource>, EffectLoweringError> {
    let expected_components =
        expected_source_types_for_carrier(types, payload_tuple_ty, args.len()).map_err(
            |detail| invalid_boundary_operand_contract(root_fqn, site_id, "Perform", detail),
        )?;
    if args.len() != expected_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!(
                "payload source 数量({}) 与 published payload tuple t{} 的 component 数量({}) 不一致",
                args.len(),
                payload_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    args.iter()
        .zip(expected_components)
        .map(|(arg, expected_ty)| {
            operand_source_with_expected_ty(
                root_fqn,
                site_id,
                "Perform",
                body,
                types,
                &arg.value,
                expected_ty,
                Some(arg.span),
            )
        })
        .collect()
}

fn validate_source_slice_bounds(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    source_slice: LateLoweredStateSlice,
) -> Result<(), EffectLoweringError> {
    let block = body
        .blocks
        .get(source_slice.block_id().as_u32() as usize)
        .ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                kind,
                format!(
                    "source slice 指向缺失的 canonical MIR block bb{}",
                    source_slice.block_id().as_u32(),
                ),
            )
        })?;
    let start = source_slice.start_statement_index() as usize;
    let end = source_slice.end_statement_index() as usize;
    if start > end || end > block.stmts.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "source slice [{}..{}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                source_slice.start_statement_index(),
                source_slice.end_statement_index(),
                source_slice.block_id().as_u32(),
                block.stmts.len(),
            ),
        ));
    }
    Ok(())
}

fn build_call_boundary_operand_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    facts: &CallSiteEffectFacts,
    result_local: LocalId,
    types: &TypeStore,
) -> Result<LateLoweredCallBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Call,
    } = boundary.source()
    else {
        unreachable!("Call boundary helper 只能消费 Call site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Call", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
            let StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        site_id: stmt_site_id,
                        kind,
                        args,
                    },
            } = &stmt.kind
            else {
                continue;
            };
            if *stmt_site_id != site_id {
                continue;
            }
            if !call_kind_matches_facts(kind, facts) {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Call",
                    format!(
                        "canonical MIR call kind {kind:?} 与 published Call facts kind {:?} 不一致",
                        facts.kind(),
                    ),
                ));
            }
            if *target != result_local {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Call",
                    format!(
                        "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                        target.as_u32(),
                        result_local.as_u32(),
                    ),
                ));
            }
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let carrier_source = match kind {
                CallKind::Direct { .. } => None,
                CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => Some(
                    operand_source_with_inferred_ty(root_fqn, site_id, "Call", body, callee, None)?,
                ),
                CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
                    Some(operand_source_with_inferred_ty(
                        root_fqn, site_id, "Call", body, receiver, None,
                    )?)
                }
                CallKind::Resume { .. } => {
                    return Err(invalid_boundary_operand_contract(
                        root_fqn,
                        site_id,
                        "Call",
                        "boundary anchor 意外指向了 Resume MIR call kind",
                    ));
                }
            };
            let arg_sources = match kind {
                CallKind::Closure { callee, .. }
                    if facts.target_mode() == CallTargetMode::KnownInstance =>
                {
                    match build_known_instance_closure_call_arg_sources(
                        root_fqn,
                        site_id,
                        "Call",
                        body,
                        types,
                        callee,
                        args,
                        facts.invoke_args_tuple_ty(),
                    )? {
                        Some(sources) => sources,
                        None => build_ordered_call_arg_sources(
                            root_fqn,
                            site_id,
                            "Call",
                            body,
                            args,
                            facts.invoke_args_tuple_ty(),
                            types,
                        )?,
                    }
                }
                _ => build_ordered_call_arg_sources(
                    root_fqn,
                    site_id,
                    "Call",
                    body,
                    args,
                    facts.invoke_args_tuple_ty(),
                    types,
                )?,
            };
            let contract = LateLoweredCallBoundaryOperandContract::new(
                LateLoweredBoundarySourceConsumption::statement(
                    source_slice,
                    statement_index,
                    statement_index.saturating_add(1) == source_slice.end_statement_index(),
                ),
                carrier_source,
                arg_sources,
            );
            if published.replace(contract).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Call",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 call statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
}

fn build_class_ctor_boundary_source_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    result_local: LocalId,
) -> Result<(String, LateLoweredBoundarySourceConsumption), EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::ClassCtor,
    } = boundary.source()
    else {
        unreachable!("ClassCtor boundary helper 只能消费 ClassCtor site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "ClassCtor", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
            };
            let source_fqn = match value {
                Rvalue::ClassCtor {
                    site_id: stmt_site_id,
                    class_fqn,
                    ..
                } if *stmt_site_id == site_id => class_fqn.clone(),
                Rvalue::TopLevelRef(top_level)
                    if top_level.site_id == Some(site_id)
                        && !top_level.hidden_effects.is_pure() =>
                {
                    top_level.fqn.clone()
                }
                Rvalue::MemberAccess {
                    site_id: Some(stmt_site_id),
                    member,
                    ..
                } if *stmt_site_id == site_id && !member.hidden_effects.is_pure() => {
                    let Some(crate::mir::MemberTarget::Value { fqn }) = member.resolved.as_ref()
                    else {
                        return Err(invalid_boundary_operand_contract(
                            root_fqn,
                            site_id,
                            "ClassCtor",
                            "hidden member init boundary source 不是 resolved value member",
                        ));
                    };
                    fqn.clone()
                }
                _ => continue,
            };
            if *target != result_local {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "ClassCtor",
                    format!(
                        "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                        target.as_u32(),
                        result_local.as_u32(),
                    ),
                ));
            }
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let consumption = LateLoweredBoundarySourceConsumption::statement(
                source_slice,
                statement_index,
                statement_index.saturating_add(1) == source_slice.end_statement_index(),
            );
            if published.replace((source_fqn, consumption)).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "ClassCtor",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 class ctor statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
}

fn build_perform_boundary_operand_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    payload_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
) -> Result<LateLoweredPerformBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Perform,
    } = boundary.source()
    else {
        unreachable!("Perform boundary helper 只能消费 Perform site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Perform", body, source_slice)?;
        if !source_slice.includes_terminator() {
            continue;
        }
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let TerminatorKind::Perform {
            site_id: term_site_id,
            args,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if *term_site_id != site_id {
            continue;
        }
        let payload_sources = build_ordered_perform_payload_sources(
            root_fqn,
            site_id,
            body,
            args,
            payload_tuple_ty,
            types,
        )?;
        let contract = LateLoweredPerformBoundaryOperandContract::new(
            LateLoweredBoundarySourceConsumption::terminator(source_slice),
            payload_sources,
        );
        if published.replace(contract).is_some() {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Perform",
                "owner state source_slices 中匹配到了多个 terminator anchor",
            ));
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 perform terminator anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn build_resume_boundary_operand_contract(
    root_fqn: &str,
    owner_version_key: &LateLoweredBodyVersionKey,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    facts: &ResumeSiteEffectFacts,
    result_local: LocalId,
    continuation_provenance: &PublishedContinuationProvenance,
    continuation_object: ContinuationObjectId,
    types: &TypeStore,
) -> Result<LateLoweredResumeBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Resume,
    } = boundary.source()
    else {
        unreachable!("Resume boundary helper 只能消费 Resume site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Resume", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
            let StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        site_id: stmt_site_id,
                        kind:
                            CallKind::Resume {
                                continuation,
                                resume,
                            },
                        args,
                    },
            } = &stmt.kind
            else {
                continue;
            };
            if *stmt_site_id != site_id {
                continue;
            }
            if *target != result_local {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Resume",
                    format!(
                        "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                        target.as_u32(),
                        result_local.as_u32(),
                    ),
                ));
            }
            if resume.resume_ty != facts.resume_tuple_ty() || resume.answer_ty != facts.answer_ty()
            {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Resume",
                    format!(
                        "canonical MIR resume metadata 与 published facts 漂移：resume_tuple=t{} answer_ty=t{}，facts=(t{}, t{})",
                        resume.resume_ty.as_u32(),
                        resume.answer_ty.as_u32(),
                        facts.resume_tuple_ty().as_u32(),
                        facts.answer_ty().as_u32(),
                    ),
                ));
            }
            let continuation_source = operand_source_with_expected_ty(
                root_fqn,
                site_id,
                "Resume",
                body,
                types,
                continuation,
                resume.continuation_ty,
                None,
            )?;
            let resolved_continuation_route = match continuation_source.value() {
                crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) => {
                    continuation_provenance.resolve_resume_local_route(root_fqn, site_id, *local)?
                }
                crate::effect_lowered::ir::LateLoweredOperandValueSource::Const(_) => {
                    ResolvedResumeLocalRoute {
                        route: None,
                        compatible_route_set: false,
                    }
                }
            };
            // Even when there is no deeper binder/member provenance to follow, the boundary must
            // still publish an authoritative self-route so later LLVM lowering never falls back to
            // source-type guesses for `k.resume(...)`.
            let underlying_continuation_route =
                resolved_continuation_route.route.unwrap_or_else(|| {
                    LateLoweredContinuationRoute::new(
                        facts.continuation_schema(),
                        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                            owner_version_key: owner_version_key.clone(),
                            owner_continuation_object: continuation_object,
                            site_id,
                        },
                    )
                });
            let arg_sources = build_ordered_call_arg_sources(
                root_fqn,
                site_id,
                "Resume",
                body,
                args,
                facts.resume_tuple_ty(),
                types,
            )?;
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let contract = LateLoweredResumeBoundaryOperandContract::new(
                LateLoweredBoundarySourceConsumption::statement(
                    source_slice,
                    statement_index,
                    statement_index.saturating_add(1) == source_slice.end_statement_index(),
                ),
                continuation_source,
                arg_sources,
                underlying_continuation_route,
                resolved_continuation_route.compatible_route_set,
            );
            if published.replace(contract).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Resume",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 resume statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
}

fn collect_result_locals(body: &Body) -> BoundaryResultLocals {
    let mut call_results = HashMap::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { target, value } = &stmt.kind {
                match value {
                    Rvalue::Call { site_id, .. } | Rvalue::ClassCtor { site_id, .. } => {
                        call_results.insert(*site_id, *target);
                    }
                    Rvalue::TopLevelRef(top_level)
                        if top_level.site_id.is_some() && !top_level.hidden_effects.is_pure() =>
                    {
                        call_results.insert(top_level.site_id.expect("checked above"), *target);
                    }
                    Rvalue::MemberAccess {
                        site_id: Some(site_id),
                        member,
                        ..
                    } if !member.hidden_effects.is_pure() => {
                        call_results.insert(*site_id, *target);
                    }
                    _ => {}
                }
            }
        }
    }
    BoundaryResultLocals { call_results }
}

fn paired_resume_boundaries(
    boundary_map: &LateLoweredBoundaryMap,
) -> (HashMap<SiteId, BoundaryId>, HashMap<SiteId, BoundaryId>) {
    let mut resume_boundaries = HashMap::new();
    let mut runtime_error_boundaries = HashMap::new();
    for boundary in boundary_map.entries() {
        match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } => {
                resume_boundaries.insert(site_id, boundary.boundary_id());
            }
            LateLoweredBoundarySource::RuntimeError { origin_site } => {
                runtime_error_boundaries.insert(origin_site, boundary.boundary_id());
            }
            LateLoweredBoundarySource::Site { .. } => {}
        }
    }
    (resume_boundaries, runtime_error_boundaries)
}

fn clone_call_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<CallSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Call(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Call",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_class_ctor_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<ClassCtorSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::ClassCtor(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "ClassCtor",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_perform_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<PerformSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Perform(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Perform",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_resume_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<ResumeSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Resume(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Resume",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_handle_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<HandleSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Handle(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Handle",
            actual: site_facts_kind(other),
        }),
    }
}

fn site_facts_kind(site: &SiteEffectFacts) -> &'static str {
    match site {
        SiteEffectFacts::Call(_) => "Call",
        SiteEffectFacts::ClassCtor(_) => "ClassCtor",
        SiteEffectFacts::Perform(_) => "Perform",
        SiteEffectFacts::Resume(_) => "Resume",
        SiteEffectFacts::Handle(_) => "Handle",
    }
}

fn lookup_step_type<'a>(
    root_fqn: &str,
    step_types: &'a [LateLoweredStepType],
    step_schema: StepSchemaId,
) -> Result<&'a LateLoweredStepType, EffectLoweringError> {
    step_types
        .iter()
        .find(|step_type| step_type.step_schema() == step_schema)
        .ok_or_else(|| EffectLoweringError::MissingStepSchema {
            root_fqn: root_fqn.to_string(),
            step_schema: step_schema.as_u32(),
        })
}

fn build_step_dispatch_plan(
    root_fqn: &str,
    input_step: &LateLoweredStepType,
    output_step: &LateLoweredStepType,
    outward_case_tags: &[crate::effect_facts::CaseTag],
    continuation_object: ContinuationObjectId,
    target_state: StateId,
    result_local: Option<LocalId>,
) -> Result<LateLoweredStepDispatchPlan, EffectLoweringError> {
    let complete =
        LateLoweredCompleteStepDispatch::new(input_step.complete_ty(), target_state, result_local);
    let outward_cases = outward_case_tags
        .iter()
        .map(|case_tag| {
            let input_case = input_step.case(*case_tag).ok_or_else(|| {
                EffectLoweringError::MissingInputStepCase {
                    root_fqn: root_fqn.to_string(),
                    step_schema: input_step.step_schema().as_u32(),
                    case_tag: case_tag.as_u32(),
                }
            })?;
            let emission = build_emission_from_concrete_op(
                root_fqn,
                input_step.step_schema(),
                output_step,
                input_case.concrete_op_key(),
                continuation_object,
            )?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredStepCaseForwarding::new(
                input_case.case_tag(),
                input_case.concrete_op_key().clone(),
                emission,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LateLoweredStepDispatchPlan::new(
        input_step.step_schema(),
        complete,
        outward_cases,
    ))
}

fn build_call_boundary_dispatch_plan(
    inputs: CallBoundaryDispatchInputs<'_>,
) -> Result<CallBoundaryDispatchMaterialization, EffectLoweringError> {
    let CallBoundaryDispatchInputs {
        root_fqn,
        boundary_id,
        input_step,
        output_step,
        outward_case_tags,
        continuation_object,
        target_state,
        result_local,
        result_frame_slot,
        types,
    } = inputs;
    let complete =
        LateLoweredCompleteStepDispatch::new(input_step.complete_ty(), target_state, result_local);
    let mut outward_cases = Vec::with_capacity(outward_case_tags.len());
    let mut continuation_compositions = Vec::with_capacity(outward_case_tags.len());
    let mut consumed_runtime_error_case = None;
    let caller_result_local =
        result_local.ok_or_else(
            || EffectLoweringError::InvalidResumePayloadBindingContract {
                root_fqn: root_fqn.to_string(),
                boundary_id: boundary_id.as_u32(),
                detail: "call-boundary continuation composition 缺少 caller result local"
                    .to_string(),
            },
        )?;

    for case_tag in outward_case_tags {
        let input_case = input_step.case(*case_tag).ok_or_else(|| {
            EffectLoweringError::MissingInputStepCase {
                root_fqn: root_fqn.to_string(),
                step_schema: input_step.step_schema().as_u32(),
                case_tag: case_tag.as_u32(),
            }
        })?;
        let projected_case = output_step
            .cases()
            .iter()
            .find(|case| case.concrete_op_key() == input_case.concrete_op_key());
        if let Some(projected_case) = projected_case {
            let forwarding = LateLoweredStepCaseForwarding::new(
                input_case.case_tag(),
                input_case.concrete_op_key().clone(),
                LateLoweredStepCaseEmission::new(
                    projected_case.case_tag(),
                    projected_case.concrete_op_key().clone(),
                    projected_case.payload_tuple_ty(),
                    projected_case.continuation_contract(),
                    continuation_object,
                ),
            );
            continuation_compositions.push(LateLoweredCallBoundaryContinuationComposition::new(
                boundary_id,
                input_step.step_schema(),
                input_case.case_tag(),
                projected_case.case_tag(),
                input_case.continuation_contract(),
                projected_case.continuation_contract(),
                target_state,
                caller_result_local,
                result_frame_slot,
                input_step.complete_ty(),
            ));
            outward_cases.push(forwarding);
            continue;
        }

        // Pure caller 仍需保留 call boundary，但 compiler-generated RuntimeError case
        // 由 boundary 本地消费，不应被强行投影回 caller outward StepSchema。
        if is_runtime_error_raise_case(input_case, types) {
            consumed_runtime_error_case.get_or_insert_with(|| PendingConsumedRuntimeErrorCase {
                input_case_tag: input_case.case_tag(),
                input_concrete_op_key: input_case.concrete_op_key().clone(),
                payload_tuple_ty: input_case.payload_tuple_ty(),
                terminal_action: local_runtime_error_terminal_action(),
            });
            continue;
        }

        return Err(EffectLoweringError::MissingProjectedStepCase {
            root_fqn: root_fqn.to_string(),
            input_step_schema: input_step.step_schema().as_u32(),
            output_step_schema: output_step.step_schema().as_u32(),
            concrete_op: input_case
                .concrete_op_key()
                .instance_key()
                .template
                .fqn
                .clone(),
        });
    }

    Ok(CallBoundaryDispatchMaterialization {
        dispatch: LateLoweredStepDispatchPlan::new(
            input_step.step_schema(),
            complete,
            outward_cases,
        ),
        continuation_compositions,
        consumed_runtime_error_case,
    })
}

fn build_boundary_continuation_compositions(
    root_fqn: &str,
    boundary_id: BoundaryId,
    input_step: &LateLoweredStepType,
    dispatch: &LateLoweredStepDispatchPlan,
    target_state: StateId,
    caller_result_local: LocalId,
    caller_result_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
) -> Result<Vec<LateLoweredCallBoundaryContinuationComposition>, EffectLoweringError> {
    dispatch
        .outward_cases()
        .iter()
        .map(|forwarding| {
            let input_case = input_step
                .case(forwarding.input_case_tag())
                .ok_or_else(|| EffectLoweringError::MissingInputStepCase {
                    root_fqn: root_fqn.to_string(),
                    step_schema: input_step.step_schema().as_u32(),
                    case_tag: forwarding.input_case_tag().as_u32(),
                })?;
            Ok(LateLoweredCallBoundaryContinuationComposition::new(
                boundary_id,
                input_step.step_schema(),
                input_case.case_tag(),
                forwarding.emission().case_tag(),
                input_case.continuation_contract(),
                forwarding.emission().continuation_contract(),
                target_state,
                caller_result_local,
                caller_result_frame_slot,
                input_step.complete_ty(),
            ))
        })
        .collect()
}

fn local_runtime_error_terminal_action() -> LateLoweredLocalRuntimeErrorTerminalAction {
    LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
        runtime_entry: LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
    }
}

fn build_current_step_emission(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    case_tag: crate::effect_facts::CaseTag,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStepCaseEmission, EffectLoweringError> {
    let case =
        step_type
            .case(case_tag)
            .ok_or_else(|| EffectLoweringError::MissingInputStepCase {
                root_fqn: root_fqn.to_string(),
                step_schema: step_type.step_schema().as_u32(),
                case_tag: case_tag.as_u32(),
            })?;
    Ok(LateLoweredStepCaseEmission::new(
        case.case_tag(),
        case.concrete_op_key().clone(),
        case.payload_tuple_ty(),
        case.continuation_contract(),
        continuation_object,
    ))
}

fn build_emission_from_concrete_op(
    root_fqn: &str,
    input_step_schema: StepSchemaId,
    output_step: &LateLoweredStepType,
    concrete_op_key: &ConcreteOpKey,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStepCaseEmission, EffectLoweringError> {
    let case = output_step
        .cases()
        .iter()
        .find(|case| case.concrete_op_key() == concrete_op_key)
        .ok_or_else(|| EffectLoweringError::MissingProjectedStepCase {
            root_fqn: root_fqn.to_string(),
            input_step_schema: input_step_schema.as_u32(),
            output_step_schema: output_step.step_schema().as_u32(),
            concrete_op: concrete_op_key.instance_key().template.fqn.clone(),
        })?;
    Ok(LateLoweredStepCaseEmission::new(
        case.case_tag(),
        case.concrete_op_key().clone(),
        case.payload_tuple_ty(),
        case.continuation_contract(),
        continuation_object,
    ))
}

fn build_handle_outward_emissions(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    facts: &HandleSiteEffectFacts,
    continuation_object: ContinuationObjectId,
) -> Result<Vec<LateLoweredStepCaseEmission>, EffectLoweringError> {
    let mut tags = BTreeSet::new();
    tags.extend(facts.body_outward_cases().tags().iter().copied());
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.into_iter()
        .map(|case_tag| {
            build_current_step_emission(root_fqn, step_type, case_tag, continuation_object)
        })
        .collect()
}

fn resume_runtime_error_effect_family(
    root_fqn: &str,
    body: &Body,
    site_id: SiteId,
    types: &TypeStore,
) -> Result<EffectFamilyKey, EffectLoweringError> {
    let resume = find_resume_metadata(body, site_id).ok_or_else(|| {
        EffectLoweringError::MissingResumeSiteMetadata {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        }
    })?;
    let runtime_error_ty = resume.runtime_error_effect_ty.ok_or_else(|| {
        EffectLoweringError::MissingResumeRuntimeErrorEffect {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        }
    })?;
    effect_family_for_effect_ty(runtime_error_ty, types).ok_or_else(|| {
        EffectLoweringError::UnsupportedEffectFamilyType {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            ty: runtime_error_ty.as_u32(),
        }
    })
}

fn find_resume_metadata(body: &Body, site_id: SiteId) -> Option<&ResumeMetadata> {
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign {
                value:
                    Rvalue::Call {
                        site_id: stmt_site,
                        kind: CallKind::Resume { resume, .. },
                        ..
                    },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            if *stmt_site == site_id {
                return Some(resume);
            }
        }
    }
    None
}

fn effect_family_for_effect_ty(
    effect_ty: crate::ty::TypeId,
    types: &TypeStore,
) -> Option<EffectFamilyKey> {
    match types.kind(effect_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(NominalType { fqn, args, .. })) => {
            Some(EffectFamilyKey::new(fqn.clone(), args.clone()))
        }
        _ => None,
    }
}

fn is_runtime_error_raise_case(case: &LateLoweredStepCase, types: &TypeStore) -> bool {
    if case.concrete_op_key().instance_key().template.fqn != "scoop.core.Raise.raise" {
        return false;
    }

    types.display(case.payload_tuple_ty()).to_string() == "scoop.core.RuntimeError"
        || case
            .concrete_op_key()
            .effect_family()
            .type_args()
            .iter()
            .any(|&ty| types.display(ty).to_string() == "scoop.core.RuntimeError")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::effect_facts::{
        CallTargetMode, CallableAbiKind, ImplPlan, NestedHandleClassification, SiteEffectFacts,
    };
    use crate::effect_lowered::LateLoweredProgramBuilder;
    use crate::effect_lowered::ir::{
        BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredBoundarySourceConsumption,
        LateLoweredCompletionPayloadSource, LateLoweredContinuationMethodReachability,
        LateLoweredContinuationResumeBody, LateLoweredFrameSlotKind,
        LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandlePendingCompletion,
        LateLoweredHandleStateRegion, LateLoweredOneShotPolicy, LateLoweredOperandValueSource,
        LateLoweredSourceStatementClassificationKind, LateLoweredStateTerminator,
        LateLoweredStepType, LateLoweredSurfaceResumeDispatchPublication,
        LateLoweredSurfaceResumeDispatchSourceKind,
        LateLoweredSurfaceResumeWrapperCompletePayloadSource, SystemSlotKind,
    };
    use crate::effect_refactor_pipeline::load_effect_facts_stage_output_for_dump;
    use crate::mir::SiteId;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    struct RawMaterializedOutput {
        effect_facts_stage_output: crate::effect_refactor_pipeline::RefactorEffectFactsStageOutput,
        program: crate::effect_lowered::LateLoweredProgram,
    }

    impl RawMaterializedOutput {
        fn program(&self) -> &crate::effect_lowered::LateLoweredProgram {
            &self.program
        }

        fn types(&self) -> &crate::ty::TypeStore {
            self.effect_facts_stage_output.types()
        }
    }

    fn load_output(source: &SourceFile) -> RawMaterializedOutput {
        let session = refactor_session();
        let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(&session, source)
            .expect("fixture 应可通过 refactor effect-facts stage");
        let program = LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .build()
        .expect("fixture 应可通过 raw late-lowering builder");
        RawMaterializedOutput {
            effect_facts_stage_output,
            program,
        }
    }

    fn callable<'a>(
        output: &'a RawMaterializedOutput,
        fqn: &str,
    ) -> &'a crate::effect_lowered::LateLoweredCallable {
        output
            .program()
            .callable(fqn)
            .unwrap_or_else(|| panic!("late-lowered program 应发布 {fqn}"))
    }

    fn site_boundary(
        callable: &crate::effect_lowered::LateLoweredCallable,
        kind: BoundarySiteKind,
    ) -> &crate::effect_lowered::ir::LateLoweredBoundary {
        callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::Site { kind: boundary_kind, .. }
                        if boundary_kind == kind
                )
            })
            .expect("应找到指定 kind 的 boundary")
    }

    fn handle_dispatch_state(
        callable: &crate::effect_lowered::LateLoweredCallable,
        site_id: SiteId,
    ) -> &crate::effect_lowered::ir::LateLoweredState {
        callable
            .state_graph()
            .states()
            .iter()
            .find(|state| {
                matches!(
                    state.terminator(),
                    LateLoweredStateTerminator::HandleDispatch { site_id: state_site, .. }
                        if *state_site == site_id
                )
            })
            .expect("应找到指定 site 的 HandleDispatch state")
    }

    fn handle_site_facts<'a>(
        output: &'a RawMaterializedOutput,
        callable: &crate::effect_lowered::LateLoweredCallable,
        site_id: SiteId,
    ) -> &'a crate::effect_facts::HandleSiteEffectFacts {
        let body_facts = output
            .effect_facts_stage_output
            .effect_facts()
            .body(callable.instance_key())
            .expect("callable 应发布 body effect facts");
        match body_facts.site(site_id) {
            Some(SiteEffectFacts::Handle(facts)) => facts,
            other => panic!("应找到指定 site 的 Handle facts，而不是 {other:?}"),
        }
    }

    #[test]
    fn refactor_step_materialization_keeps_canonical_cases_and_dynamic_entry_states() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let step_type = output
            .program()
            .step_type(leaf.step_schema())
            .expect("callable 应能回查 canonical Step shell");
        let case_fqns = step_type
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            case_fqns,
            [
                "sample.Ping.hit".to_string(),
                "scoop.core.Raise.raise".to_string()
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().step_schema(),
            leaf.step_schema()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().entry_state(),
            leaf.state_graph().entry_state()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().complete_state(),
            leaf.state_graph().complete_state()
        );
    }

    #[test]
    fn refactor_resume_interface_completeness_groups_methods_by_effect_family() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let interfaces = leaf
            .resume_packings()
            .iter()
            .map(|interface_id| {
                output
                    .program()
                    .resume_packing(*interface_id)
                    .expect("callable 应能回查 resume interface")
            })
            .collect::<Vec<_>>();

        assert_eq!(interfaces.len(), 2);
        assert_eq!(
            interfaces
                .iter()
                .map(|interface| interface.effect_family().effect_fqn().to_string())
                .collect::<BTreeSet<_>>(),
            ["sample.Ping".to_string(), "scoop.core.Raise".to_string()]
                .into_iter()
                .collect()
        );
        assert!(
            interfaces
                .iter()
                .all(|interface| interface.return_step_schema() == leaf.step_schema())
        );
        assert_eq!(
            interfaces
                .iter()
                .map(|interface| interface.methods().len())
                .sum::<usize>(),
            output
                .program()
                .step_type(leaf.step_schema())
                .expect("callable 应能回查 step shell")
                .cases()
                .len()
        );
    }

    #[test]
    fn refactor_continuation_object_materializes_surface_resume_and_one_shot_contracts() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let object = output
            .program()
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object");

        assert_eq!(object.surface_resumes().len(), 2);
        assert_eq!(object.methods().len(), 2);
        assert_eq!(
            object
                .methods()
                .iter()
                .filter(|method| {
                    method.reachability() == LateLoweredContinuationMethodReachability::Reachable
                })
                .count(),
            1
        );
        assert!(object.surface_resumes().iter().any(|surface| {
            output.types().display(surface.surface_ty()).to_string()
                == "scoop.core.Continuation<Unit, Unit, eff sample.Ping>"
                && matches!(
                    surface.body(),
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward
                    }
                )
        }));
        assert!(object.surface_resumes().iter().any(|surface| {
            surface.concrete_op_key().instance_key().template.fqn == "scoop.core.Raise.raise"
                && surface.reachability() == LateLoweredContinuationMethodReachability::Unreachable
        }));
    }

    #[test]
    fn refactor_surface_resume_dispatch_inventory_marks_shared_schema_object_method_sources() {
        let output = load_output(&load_fixture(
            "build",
            "effect_refactor_step_enum_single_case.scoop",
        ));
        let worker = callable(&output, "fixtures.build.singleCaseWorker");
        let step = output
            .program()
            .step_type(worker.step_schema())
            .expect("worker step schema 应可回查");
        let shared_schema = step
            .case(crate::effect_facts::CaseTag::new(0))
            .expect("worker c0 应存在")
            .continuation_schema();
        assert_eq!(
            shared_schema,
            step.case(crate::effect_facts::CaseTag::new(1))
                .expect("worker c1 应存在")
                .continuation_schema()
        );

        let entry = output
            .program()
            .surface_resume_dispatch(shared_schema)
            .expect("shared schema 应发布 dispatch inventory");
        assert_eq!(
            entry.source_kind(),
            LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
        );

        let mut saw_surface_c0 = false;
        let mut saw_surface_c1 = false;
        let mut saw_method_c0 = false;
        for publication in entry.publications() {
            match publication {
                LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
                    object_id,
                    case_tag,
                    reachability,
                } if *object_id == worker.continuation_object()
                    && *case_tag == crate::effect_facts::CaseTag::new(0) =>
                {
                    assert_eq!(
                        *reachability,
                        LateLoweredContinuationMethodReachability::Reachable
                    );
                    saw_surface_c0 = true;
                }
                LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
                    object_id,
                    case_tag,
                    reachability,
                } if *object_id == worker.continuation_object()
                    && *case_tag == crate::effect_facts::CaseTag::new(1) =>
                {
                    assert_eq!(
                        *reachability,
                        LateLoweredContinuationMethodReachability::Unreachable
                    );
                    saw_surface_c1 = true;
                }
                LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                    object_id,
                    case_tag,
                    reachability,
                    ..
                } if *object_id == worker.continuation_object()
                    && *case_tag == crate::effect_facts::CaseTag::new(0) =>
                {
                    assert_eq!(
                        *reachability,
                        LateLoweredContinuationMethodReachability::Reachable
                    );
                    saw_method_c0 = true;
                }
                _ => {}
            }
        }

        assert!(
            saw_surface_c0,
            "shared schema 应保留 c0 surface publication"
        );
        assert!(
            saw_surface_c1,
            "shared schema 应保留 c1 surface publication"
        );
        assert!(
            saw_method_c0,
            "shared schema 应明确发布唯一可达的 internal method source"
        );
    }

    #[test]
    fn refactor_surface_resume_dispatch_inventory_covers_resume_site_only_and_handle_binder_schema()
    {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let run = callable(&output, "run");

        let resume_schema = run
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    Some(lowering.facts().continuation_schema())
                }
                _ => None,
            })
            .expect("fixture 应至少包含一个 resume boundary schema");
        let resume_entry = output
            .program()
            .surface_resume_dispatch(resume_schema)
            .expect("resume-site-only schema 应发布 dispatch inventory");
        assert_eq!(
            resume_entry.source_kind(),
            LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
        );
        assert!(resume_entry.publications().iter().any(|publication| {
            matches!(
                publication,
                LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                    owner_continuation_object,
                    site_id,
                    ..
                } if *owner_continuation_object == run.continuation_object() && site_id.as_u32() == 9
            )
        }));

        let handle_schema = run
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch { contract, .. } => {
                    contract.handled_arms().iter().find_map(|arm| {
                        arm.continuation_binder()
                            .map(|binder| binder.continuation_schema())
                    })
                }
                _ => None,
            })
            .expect("fixture 应至少包含一个 handle continuation binder schema");
        let handle_entry = output
            .program()
            .surface_resume_dispatch(handle_schema)
            .expect("handle binder schema 应发布 dispatch inventory");
        assert_eq!(
            handle_entry.source_kind(),
            LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
        );
        assert!(handle_entry.publications().iter().any(|publication| {
            matches!(
                publication,
                LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                    owner_continuation_object,
                    site_id,
                    arm_ordinal,
                    handled_case,
                    ..
                } if *owner_continuation_object == run.continuation_object()
                    && site_id.as_u32() == 0
                    && *arm_ordinal == 0
                    && *handled_case == crate::effect_facts::CaseTag::new(0)
            )
        }));
    }

    #[test]
    fn refactor_boundary_lowering_materializes_effectful_call_dispatch_contract() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let call_value = callable(&output, "sample.callValue");
        let boundary = site_boundary(call_value, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(lowering) = boundary
            .lowering()
            .expect("call boundary 应发布 lowering contract")
        else {
            panic!("call boundary 应物化成 Call lowering")
        };

        assert_eq!(
            lowering.facts().target_mode(),
            CallTargetMode::DynamicFallback
        );
        assert_eq!(
            lowering.dispatch().input_step_schema(),
            lowering.facts().callee_schema()
        );
        assert_eq!(
            lowering.dispatch().complete().target_state(),
            boundary.resume_state()
        );
        assert_eq!(lowering.dispatch().outward_cases().len(), 2);
        assert!(lowering.consumed_runtime_error_case().is_none());
        assert_eq!(
            lowering
                .dispatch()
                .outward_cases()
                .iter()
                .map(|forwarding| {
                    forwarding
                        .emission()
                        .concrete_op_key()
                        .instance_key()
                        .template
                        .fqn
                        .clone()
                })
                .collect::<BTreeSet<_>>(),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn refactor_effect_lowered_boundary_operand_contract_publishes_direct_dynamic_and_perform_sources()
     {
        let direct_output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let main = callable(&direct_output, "main");
        let direct_boundary = site_boundary(main, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(direct_lowering) = direct_boundary
            .lowering()
            .expect("direct call boundary 应发布 lowering contract")
        else {
            panic!("main 的 boundary 应物化成 Call lowering")
        };
        assert!(matches!(
            direct_lowering.operand_contract().source_consumption(),
            LateLoweredBoundarySourceConsumption::Statement {
                consumes_last_statement: true,
                ..
            }
        ));
        assert!(
            direct_lowering
                .operand_contract()
                .carrier_source()
                .is_none()
        );
        assert_eq!(direct_lowering.operand_contract().arg_sources().len(), 1);
        assert_eq!(
            direct_output
                .types()
                .display(direct_lowering.operand_contract().arg_sources()[0].source_ty())
                .to_string(),
            "Bool"
        );
        assert!(matches!(
            direct_lowering.operand_contract().arg_sources()[0].value(),
            LateLoweredOperandValueSource::Local(_)
                | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Bool(_))
        ));
        assert!(
            direct_lowering.operand_contract().arg_sources()[0]
                .span()
                .is_some()
        );

        let dynamic_output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let call_value = callable(&dynamic_output, "sample.callValue");
        let dynamic_boundary = site_boundary(call_value, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(dynamic_lowering) = dynamic_boundary
            .lowering()
            .expect("dynamic call boundary 应发布 lowering contract")
        else {
            panic!("callValue 的 boundary 应物化成 Call lowering")
        };
        assert_eq!(dynamic_lowering.operand_contract().arg_sources().len(), 0);
        assert!(matches!(
            dynamic_lowering.operand_contract().source_consumption(),
            LateLoweredBoundarySourceConsumption::Statement { .. }
        ));
        assert!(matches!(
            dynamic_lowering
                .operand_contract()
                .carrier_source()
                .expect("dynamic call 应发布 carrier source")
                .value(),
            LateLoweredOperandValueSource::Local(_)
        ));

        let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
        let handled_main = callable(&perform_output, "a.main");
        let perform_boundary = site_boundary(handled_main, BoundarySiteKind::Perform);
        let LateLoweredBoundaryLowering::Perform(perform_lowering) = perform_boundary
            .lowering()
            .expect("perform boundary 应发布 lowering contract")
        else {
            panic!("perform boundary 应物化成 Perform lowering")
        };
        assert!(matches!(
            perform_lowering.operand_contract().source_consumption(),
            LateLoweredBoundarySourceConsumption::Terminator { .. }
        ));
        assert_eq!(
            perform_lowering.operand_contract().payload_sources().len(),
            1
        );
        assert_eq!(
            perform_output
                .types()
                .display(perform_lowering.operand_contract().payload_sources()[0].source_ty())
                .to_string(),
            "Int"
        );
        assert!(matches!(
            perform_lowering.operand_contract().payload_sources()[0].value(),
            LateLoweredOperandValueSource::Local(_)
                | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
        ));
        assert!(
            perform_lowering.operand_contract().payload_sources()[0]
                .span()
                .is_some()
        );
    }

    #[test]
    fn refactor_effect_lowered_boundary_operand_contract_publishes_known_closure_env_sources() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_callee_suspend_matrix.scoop",
        ));
        let main = callable(&output, "main");
        let closure_boundary = main
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.lowering(),
                    Some(LateLoweredBoundaryLowering::Call(lowering))
                        if lowering.facts().kind() == crate::effect_facts::CallSiteKind::Closure
                            && lowering.facts().target_mode() == CallTargetMode::KnownInstance
                )
            })
            .expect("fixture 应包含 known-instance closure call boundary");
        let LateLoweredBoundaryLowering::Call(lowering) = closure_boundary
            .lowering()
            .expect("closure boundary 应发布 lowering contract")
        else {
            panic!("closure boundary 应物化成 Call lowering")
        };

        assert!(
            lowering.operand_contract().carrier_source().is_some(),
            "closure call 仍应发布 callable carrier source"
        );
        assert_eq!(
            lowering.operand_contract().arg_sources().len(),
            1,
            "known-instance closure direct args 应由 closure env carrier 发布为单一 source"
        );
        assert_eq!(
            lowering.operand_contract().arg_sources()[0].source_ty(),
            lowering.facts().invoke_args_tuple_ty()
        );
        assert!(matches!(
            lowering.operand_contract().arg_sources()[0].value(),
            LateLoweredOperandValueSource::Local(_)
        ));
    }

    #[test]
    fn refactor_effect_lowered_boundary_operand_contract_publishes_resume_sources() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "dispatch_and_resume_call.scoop",
        ));
        let callable = callable(&output, "fixtures.mir.resumeBoom");
        let resume_boundary = site_boundary(callable, BoundarySiteKind::Resume);
        let LateLoweredBoundaryLowering::Resume(resume_lowering) = resume_boundary
            .lowering()
            .expect("resume boundary 应发布 lowering contract")
        else {
            panic!("resume boundary 应物化成 Resume lowering")
        };
        assert!(matches!(
            resume_lowering.operand_contract().source_consumption(),
            LateLoweredBoundarySourceConsumption::Statement {
                consumes_last_statement: true,
                ..
            }
        ));
        assert!(matches!(
            resume_lowering
                .operand_contract()
                .continuation_source()
                .value(),
            LateLoweredOperandValueSource::Local(_)
        ));
        assert!(
            output
                .types()
                .display(
                    resume_lowering
                        .operand_contract()
                        .continuation_source()
                        .source_ty(),
                )
                .to_string()
                .contains("Continuation")
        );
        assert_eq!(resume_lowering.operand_contract().arg_sources().len(), 1);
        assert_eq!(
            output
                .types()
                .display(resume_lowering.operand_contract().arg_sources()[0].source_ty())
                .to_string(),
            "Int"
        );
        assert!(matches!(
            resume_lowering.operand_contract().arg_sources()[0].value(),
            LateLoweredOperandValueSource::Local(_)
                | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
        ));
        assert!(
            resume_lowering.operand_contract().arg_sources()[0]
                .span()
                .is_some()
        );
    }

    #[test]
    fn refactor_boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let main = callable(&output, "main");
        let step_type = output
            .program()
            .step_type(main.step_schema())
            .expect("main 应能回查 canonical Step shell");
        let call_boundaries = main
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Call(lowering)) => Some((boundary, lowering)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(main.resolved_outward_cases().is_empty());
        assert!(step_type.cases().is_empty());
        assert_eq!(call_boundaries.len(), 2);
        assert!(
            call_boundaries
                .iter()
                .all(|(_, lowering)| lowering.dispatch().outward_cases().is_empty())
        );
        for (boundary, lowering) in call_boundaries {
            let runtime_error_case = lowering
                .consumed_runtime_error_case()
                .expect("pure caller 的 call boundary 应显式发布本地 runtime-error contract");
            assert_eq!(runtime_error_case.input_case_tag().as_u32(), 1);
            assert_eq!(
                runtime_error_case
                    .input_concrete_op_key()
                    .instance_key()
                    .template
                    .fqn,
                "scoop.core.Raise.raise"
            );
            assert_eq!(
                output
                    .types()
                    .display(runtime_error_case.payload_tuple_ty())
                    .to_string(),
                "scoop.core.RuntimeError"
            );
            assert_eq!(
                runtime_error_case.terminal_action(),
                crate::effect_lowered::ir::LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry:
                        crate::effect_lowered::ir::LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
                }
            );
            let target_state = main
                .state_graph()
                .states()
                .iter()
                .find(|state| state.state_id() == runtime_error_case.target_state())
                .expect("本地 runtime-error contract 应发布 dedicated target state");
            assert!(main.state_graph().states().iter().any(|state| {
                state.state_id() == boundary.owner_state()
                    && matches!(
                        state.terminator(),
                        crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend {
                            local_runtime_error_states,
                            ..
                        } if local_runtime_error_states.contains(&runtime_error_case.target_state())
                    )
            }));
            assert!(matches!(
                target_state.terminator(),
                crate::effect_lowered::ir::LateLoweredStateTerminator::LocalRuntimeError {
                    payload_tuple_ty,
                    terminal_action,
                } if *payload_tuple_ty == runtime_error_case.payload_tuple_ty()
                    && *terminal_action == runtime_error_case.terminal_action()
            ));
        }
    }

    #[test]
    fn refactor_boundary_lowering_materializes_resume_and_runtime_error_contracts() {
        let output = load_output(&load_fixture(
            "mir_refactor",
            "dispatch_and_resume_call.scoop",
        ));
        let callable = callable(&output, "fixtures.mir.resumeBoom");
        let resume_boundary = site_boundary(callable, BoundarySiteKind::Resume);
        let runtime_error_boundary = callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::RuntimeError { .. }
                )
            })
            .expect("resume callable 应发布 runtime-error boundary");

        let LateLoweredBoundaryLowering::Resume(resume_lowering) = resume_boundary
            .lowering()
            .expect("resume boundary 应发布 lowering contract")
        else {
            panic!("resume boundary 应物化成 Resume lowering")
        };
        let LateLoweredBoundaryLowering::RuntimeError(runtime_error_lowering) =
            runtime_error_boundary
                .lowering()
                .expect("runtime-error boundary 应发布 lowering contract")
        else {
            panic!("runtime-error boundary 应物化成 RuntimeError lowering")
        };

        assert_eq!(
            resume_lowering.runtime_error_boundary(),
            runtime_error_boundary.boundary_id()
        );
        assert_eq!(
            runtime_error_lowering.resume_boundary(),
            resume_boundary.boundary_id()
        );
        assert_eq!(
            resume_lowering.dispatch().input_step_schema(),
            resume_lowering.facts().out_step_schema()
        );
        assert_eq!(resume_lowering.dispatch().outward_cases().len(), 2);
        assert_eq!(
            runtime_error_lowering
                .emitted_step()
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );
    }

    #[test]
    fn refactor_boundary_lowering_publishes_member_readback_resume_route() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("main site0 应保持 HandleDispatch terminator");
        };
        let binder = contract.handled_arms()[0]
            .continuation_binder()
            .expect("Ask handle arm 应发布 continuation binder");

        let resume_routes = callable
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    let crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                        site_id,
                        kind: BoundarySiteKind::Resume,
                    } = boundary.source()
                    else {
                        return None;
                    };
                    Some((site_id, lowering))
                }
                _ => None,
            })
            .map(|(site_id, lowering)| {
                let route = lowering.operand_contract().underlying_continuation_route();
                (site_id, route)
            })
            .collect::<Vec<_>>();

        assert_eq!(
            resume_routes
                .iter()
                .map(|(site_id, _)| site_id.as_u32())
                .collect::<Vec<_>>(),
            vec![25, 30, 35, 40]
        );
        for (_site_id, route) in resume_routes {
            assert_eq!(route.continuation_schema(), binder.continuation_schema());
            assert!(matches!(
                route.publication(),
                LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                    owner_continuation_object,
                    site_id,
                    arm_ordinal,
                    handled_case,
                    ..
                } if *owner_continuation_object == callable.continuation_object()
                    && site_id.as_u32() == 0
                    && *arm_ordinal == 0
                    && *handled_case == contract.handled_arms()[0].handled_case()
            ));
        }
    }

    #[test]
    fn refactor_boundary_lowering_publishes_local_option_continuation_readback_route() {
        let output = load_output(&load_fixture(
            "run-pass",
            "continuation_resume_continuation.scoop",
        ));
        let callable = callable(&output, "main");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("main site0 应保持 Outer.getK HandleDispatch terminator");
        };
        let binder = contract.handled_arms()[0]
            .continuation_binder()
            .expect("Outer.getK arm 应发布 continuation binder");
        let route = callable
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.source() {
                crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Resume,
                } if site_id.as_u32() == 15 => match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some(lowering.operand_contract().underlying_continuation_route())
                    }
                    _ => None,
                },
                _ => None,
            })
            .expect("ok.resume(ik) 应发布 resume boundary route");

        assert_eq!(route.continuation_schema(), binder.continuation_schema());
        assert!(matches!(
            route.publication(),
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_continuation_object,
                site_id,
                arm_ordinal,
                handled_case,
                ..
            } if *owner_continuation_object == callable.continuation_object()
                && site_id.as_u32() == 0
                && *arm_ordinal == 0
                && *handled_case == contract.handled_arms()[0].handled_case()
        ));
    }

    #[test]
    fn refactor_surface_resume_dispatch_inventory_publishes_shared_wrapper_projection() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("main site0 应保持 HandleDispatch terminator");
        };
        let binder = contract.handled_arms()[0]
            .continuation_binder()
            .expect("Ask handle arm 应发布 continuation binder");
        let resume_lowering = callable
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                _ => None,
            })
            .expect("fixture 应至少包含一个 resume boundary");
        let wrapper_schema = resume_lowering.facts().continuation_schema();
        let inventory_entry = output
            .program()
            .surface_resume_dispatch(wrapper_schema)
            .expect("shared wrapper schema 应发布 authoritative inventory");
        let projection = inventory_entry
            .wrapper_projection()
            .expect("shared wrapper schema 应发布 owner-step -> wrapper-step projection");
        let outward = projection
            .outward_cases()
            .first()
            .expect("wrapper projection 应至少包含一个 outward case");
        let forwarded = resume_lowering
            .dispatch()
            .outward_cases()
            .first()
            .expect("resume boundary dispatch 应至少包含一个 forwarded outward case");

        assert_eq!(
            projection.underlying_route().continuation_schema(),
            binder.continuation_schema()
        );
        assert!(matches!(
            projection.underlying_route().publication(),
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_continuation_object,
                site_id,
                arm_ordinal,
                handled_case,
                ..
            } if *owner_continuation_object == callable.continuation_object()
                && site_id.as_u32() == 0
                && *arm_ordinal == 0
                && *handled_case == contract.handled_arms()[0].handled_case()
        ));
        assert_eq!(projection.owner_step_schema(), callable.step_schema());
        assert_eq!(
            projection.wrapper_step_schema(),
            resume_lowering.facts().out_step_schema()
        );
        assert_eq!(
            projection.complete().wrapper_answer_ty(),
            resume_lowering.dispatch().complete().answer_ty()
        );
        assert_eq!(outward.owner_case_tag(), forwarded.emission().case_tag());
        assert_eq!(
            outward.owner_concrete_op_key(),
            forwarded.emission().concrete_op_key()
        );
        assert_eq!(outward.wrapper_case_tag(), forwarded.input_case_tag());
        assert_eq!(
            outward.wrapper_concrete_op_key(),
            forwarded.input_concrete_op_key()
        );
        assert_eq!(
            outward.wrapper_continuation_contract().out_step_schema(),
            resume_lowering.facts().out_step_schema()
        );
    }

    #[test]
    fn refactor_surface_resume_dispatch_inventory_publishes_wrapper_outward_continuation_schema() {
        let output = load_output(&load_fixture(
            "build",
            "effect_refactor_direct_handle_resume_emit_llvm.scoop",
        ));
        let callable = callable(&output, "fixtures.build.main");
        let resume_lowering = callable
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                _ => None,
            })
            .expect("fixture 应包含 resume boundary");
        let projection = output
            .program()
            .surface_resume_dispatch(resume_lowering.facts().continuation_schema())
            .and_then(|entry| entry.wrapper_projection())
            .expect("resume wrapper schema 应发布 owner-step -> wrapper-step projection");
        let outward = projection
            .outward_cases()
            .first()
            .expect("wrapper projection 应发布 outward case continuation contract");
        let contract = outward.wrapper_continuation_contract();
        let entry = output
            .program()
            .surface_resume_dispatch(contract.continuation_schema())
            .expect("wrapper outward continuation schema 应发布 surface-resume inventory");

        assert_eq!(
            entry.contract().resume_tuple_ty(),
            contract.resume_tuple_ty()
        );
        assert_eq!(entry.contract().answer_ty(), contract.answer_ty());
        assert_eq!(
            entry.contract().out_step_schema(),
            contract.out_step_schema()
        );
        assert_eq!(entry.wrapper_projection(), Some(projection));
        assert_eq!(
            entry.source_kind(),
            LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
        );
        assert!(entry.publications().iter().any(|publication| matches!(
            publication,
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_continuation_object,
                ..
            } if *owner_continuation_object == callable.continuation_object()
        )));
    }

    #[test]
    fn refactor_surface_resume_dispatch_dump_exposes_shared_wrapper_projection() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("wrapper_projection:"));
        assert!(dump.contains("underlying_route: continuation_schema=k3"));
        assert!(dump.contains("owner_step_schema: s1"));
        assert!(dump.contains("wrapper_step_schema: s4"));
        assert!(
            dump.contains(
                "owner c2 op=scoop.core.Raise.raise<t217> payload_tuple_ty=t217 -> wrapper c0"
            ),
            "shared wrapper projection 应直接暴露 owner -> wrapper 映射\n{dump}"
        );
    }

    #[test]
    fn refactor_effect_lowered_surface_resume_wrapper_completion_publishes_handle_arm_payload_source()
     {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("main site0 应保持 HandleDispatch terminator");
        };
        let arm_source = contract.handled_arms()[0].completion_payload_source();
        let resume_schema = callable
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    Some(lowering.facts().continuation_schema())
                }
                _ => None,
            })
            .expect("fixture 应包含 shared wrapper resume schema");
        let projection = output
            .program()
            .surface_resume_dispatch(resume_schema)
            .and_then(|entry| entry.wrapper_projection())
            .expect("shared wrapper schema 应发布 complete projection");

        assert_eq!(
            projection.complete().wrapper_answer_ty(),
            arm_source.source_ty()
        );
        assert_eq!(
            projection
                .complete()
                .payload_source()
                .wrapper_payload_source(),
            Some(arm_source),
            "wrapper Complete(Int) 应直接引用 top-level handle arm 的 completion payload source"
        );
        assert!(matches!(
            arm_source,
            LateLoweredCompletionPayloadSource::Operand(source)
                if matches!(source.value(), LateLoweredOperandValueSource::Local(_))
        ));
        let dump = output.program().stable_dump();
        assert!(
            dump.contains("complete: owner_answer_ty=t2 -> wrapper_answer_ty=t5 payload=local")
        );
        assert!(dump.contains("completion_payload: local"));
    }

    #[test]
    fn refactor_effect_lowered_surface_resume_wrapper_completion_uses_owner_complete_for_matching_answer_type()
     {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let projection = output
            .program()
            .surface_resume_dispatch_inventory()
            .iter()
            .find_map(|entry| entry.wrapper_projection())
            .expect("matching answer type fixture 应发布 wrapper projection");

        assert_eq!(
            projection.complete().owner_answer_ty(),
            projection.complete().wrapper_answer_ty(),
            "fixture 应覆盖 owner/wrapper answer type 相同的投影路径"
        );
        assert!(matches!(
            projection.complete().payload_source(),
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty }
                if *answer_ty == projection.complete().wrapper_answer_ty()
        ));
        assert!(
            projection
                .complete()
                .payload_source()
                .wrapper_payload_source()
                .is_none(),
            "同型 Complete 投影应直接复用 owner Complete payload，而不是发布 wrapper payload source"
        );
        let dump = output.program().stable_dump();
        assert!(dump.contains("payload=owner_complete:"));
    }

    #[test]
    fn refactor_effect_lowered_resume_boundary_self_route_publishes_step_projection() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_escape_continuation_resume_later_exit.scoop",
        ));
        let callable = callable(&output, "main");
        let resume_lowering = callable
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                _ => None,
            })
            .expect("fixture 应包含 resume boundary");
        let projection = output
            .program()
            .surface_resume_dispatch(resume_lowering.facts().continuation_schema())
            .and_then(|entry| entry.wrapper_projection())
            .expect("same-schema 但不同 StepSchema 的 resume boundary 应发布 wrapper projection");

        assert_eq!(projection.owner_step_schema(), callable.step_schema());
        assert_eq!(
            projection.wrapper_step_schema(),
            resume_lowering.facts().out_step_schema()
        );
        assert_ne!(
            projection.owner_step_schema(),
            projection.wrapper_step_schema()
        );
        assert!(matches!(
            projection.complete().payload_source(),
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. }
        ));
    }

    #[test]
    fn refactor_boundary_lowering_publishes_direct_resume_self_route() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "dispatch_and_resume_call.scoop",
        ));

        for callable_fqn in ["fixtures.mir.resumeBoom", "fixtures.mir.resumeOnce"] {
            let callable = callable(&output, callable_fqn);
            let boundary = site_boundary(callable, BoundarySiteKind::Resume);
            let site_id = match boundary.source() {
                crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Resume,
                } => site_id,
                other => panic!("{callable_fqn} 应发布 resume boundary，而不是 {other:?}"),
            };
            let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
                panic!("{callable_fqn} resume boundary 应带 lowering");
            };

            let route = lowering.operand_contract().underlying_continuation_route();
            assert_eq!(
                route.continuation_schema(),
                lowering.facts().continuation_schema()
            );
            assert!(matches!(
                route.publication(),
                LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                    owner_version_key,
                    owner_continuation_object,
                    site_id: route_site_id,
                } if owner_version_key == callable.body_version_key()
                    && *owner_continuation_object == callable.continuation_object()
                    && *route_site_id == site_id
            ));
        }
    }

    #[test]
    fn refactor_effect_lowered_resume_payload_binding_covers_call_and_resume_boundaries() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));

        let main = callable(&output, "main");
        let call_boundary = site_boundary(main, BoundarySiteKind::Call);
        let call_binding = main
            .frame_schema()
            .resume_payload_binding(call_boundary.boundary_id())
            .expect("call boundary 应发布 resumed local/home contract");
        let call_slot = main
            .frame_schema()
            .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
                boundary: call_boundary.boundary_id(),
                local: call_binding.consumer_local(),
            })
            .expect("call boundary 应保留 BoundaryResult home slot");

        assert_eq!(call_binding.resume_state(), call_boundary.resume_state());
        assert_eq!(
            call_binding.consumer_frame_slot(),
            Some(call_slot.slot_id())
        );

        let run = callable(&output, "run");
        let resume_boundary = site_boundary(run, BoundarySiteKind::Resume);
        let resume_binding = run
            .frame_schema()
            .resume_payload_binding(resume_boundary.boundary_id())
            .expect("resume boundary 应发布 resumed local/home contract");
        let resume_slot = run
            .frame_schema()
            .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
                boundary: resume_boundary.boundary_id(),
                local: resume_binding.consumer_local(),
            })
            .expect("resume boundary 应保留 BoundaryResult home slot");

        assert_eq!(
            resume_binding.resume_state(),
            resume_boundary.resume_state()
        );
        assert_eq!(
            resume_binding.consumer_frame_slot(),
            Some(resume_slot.slot_id())
        );
    }

    #[test]
    fn refactor_effect_lowered_resume_payload_binding_covers_perform_and_runtime_error_paths() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));

        let fetch = callable(&output, "fetch");
        let perform_boundary = site_boundary(fetch, BoundarySiteKind::Perform);
        let perform_binding = fetch
            .frame_schema()
            .resume_payload_binding(perform_boundary.boundary_id())
            .expect("perform boundary 应发布 resumed local/home contract");
        let perform_slot = fetch
            .frame_schema()
            .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
                boundary: perform_boundary.boundary_id(),
                local: perform_binding.consumer_local(),
            })
            .expect("perform boundary 应保留 PerformResult 对应的 BoundaryResult slot");

        assert_eq!(
            perform_binding.resume_state(),
            perform_boundary.resume_state()
        );
        assert_eq!(
            perform_binding.consumer_frame_slot(),
            Some(perform_slot.slot_id())
        );

        let main = callable(&output, "main");
        let resume_boundary = site_boundary(main, BoundarySiteKind::Resume);
        let runtime_error_boundary = main
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::RuntimeError {
                        origin_site
                    } if origin_site == SiteId::from_raw(25)
                )
            })
            .expect("site25 的 paired runtime-error boundary 应存在");
        let resume_binding = main
            .frame_schema()
            .resume_payload_binding(resume_boundary.boundary_id())
            .expect("resume boundary 应发布 resumed local/home contract");
        let runtime_error_binding = main
            .frame_schema()
            .resume_payload_binding(runtime_error_boundary.boundary_id())
            .expect("runtime-error boundary 应显式继承 resumed local/home contract");

        assert_eq!(
            runtime_error_binding.resume_state(),
            runtime_error_boundary.resume_state()
        );
        assert_eq!(
            runtime_error_binding.consumer_local(),
            resume_binding.consumer_local()
        );
        assert_eq!(
            runtime_error_binding.consumer_frame_slot(),
            resume_binding.consumer_frame_slot(),
        );
    }

    #[test]
    fn refactor_effect_lowered_call_boundary_continuation_composition() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));

        let main = callable(&output, "main");
        let (boundary, lowering) = main
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| {
                let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                    return None;
                };
                (!lowering.continuation_compositions().is_empty()).then_some((boundary, lowering))
            })
            .expect("main 的 fetch call boundary 应发布 continuation composition");
        let input_step = output
            .program()
            .step_type(lowering.dispatch().input_step_schema())
            .expect("call boundary input step schema 应可回查");
        let result_binding = main
            .frame_schema()
            .resume_payload_binding(boundary.boundary_id())
            .expect("call boundary 应发布 caller result home binding");

        assert_eq!(result_binding.resume_state(), boundary.resume_state());
        assert_eq!(result_binding.consumer_local(), lowering.result_local());
        assert_eq!(
            lowering.continuation_compositions().len(),
            lowering.dispatch().outward_cases().len(),
            "每个 call-boundary outward forwarding 都必须有 composition contract"
        );

        for composition in lowering.continuation_compositions() {
            assert_eq!(composition.boundary_id(), boundary.boundary_id());
            assert_eq!(composition.input_step_schema(), input_step.step_schema());
            assert_eq!(composition.caller_resume_state(), boundary.resume_state());
            assert_eq!(
                composition.caller_result_local(),
                result_binding.consumer_local()
            );
            assert_eq!(
                composition.caller_result_frame_slot(),
                result_binding.consumer_frame_slot()
            );
            assert_eq!(composition.caller_result_ty(), input_step.complete_ty());

            let input_case = input_step
                .case(composition.input_case_tag())
                .expect("composition input case 应存在于 callee Step_F");
            let forwarding = lowering
                .dispatch()
                .outward_cases()
                .iter()
                .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
                .expect("composition 应对应一个 dispatch forwarding");

            assert_eq!(
                composition.callee_continuation_contract(),
                input_case.continuation_contract()
            );
            assert_eq!(
                composition.output_case_tag(),
                forwarding.emission().case_tag()
            );
            assert_eq!(
                composition.caller_continuation_contract(),
                forwarding.emission().continuation_contract()
            );
        }

        let rendered = crate::effect_lowered::render_late_lowered_program(output.program());
        assert!(
            rendered.contains("continuation_compositions:"),
            "dump-effect-lowered 应渲染 call-boundary continuation composition handoff"
        );
    }

    #[test]
    fn refactor_effect_lowered_resume_boundary_continuation_composition_for_cross_call_escape() {
        let output = load_output(&load_fixture(
            "run-pass",
            "continuation_escape_binder_resume_effect_row_runtime_basic.scoop",
        ));

        let main = callable(&output, "main");
        let (boundary, lowering) = main
            .boundary_map()
            .entries()
            .iter()
            .find_map(|boundary| {
                let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                else {
                    return None;
                };
                let route = lowering.operand_contract().underlying_continuation_route();
                matches!(
                    route.publication(),
                    LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        owner_version_key,
                        ..
                    } if owner_version_key.surface_instance().template.fqn == "start"
                )
                .then_some((boundary, lowering))
            })
            .expect("main 的 saved continuation resume boundary 应接回 start 的 binder route");
        let input_step = output
            .program()
            .step_type(lowering.dispatch().input_step_schema())
            .expect("resume boundary input step schema 应可回查");
        let result_binding = main
            .frame_schema()
            .resume_payload_binding(boundary.boundary_id())
            .expect("resume boundary 应发布 caller result home binding");

        assert_eq!(result_binding.resume_state(), boundary.resume_state());
        assert_eq!(result_binding.consumer_local(), lowering.result_local());
        assert_eq!(
            lowering.continuation_compositions().len(),
            lowering.dispatch().outward_cases().len(),
            "每个 resume-boundary outward forwarding 都必须有 composition contract"
        );

        for composition in lowering.continuation_compositions() {
            assert_eq!(composition.boundary_id(), boundary.boundary_id());
            assert_eq!(composition.input_step_schema(), input_step.step_schema());
            assert_eq!(composition.caller_resume_state(), boundary.resume_state());
            assert_eq!(
                composition.caller_result_local(),
                result_binding.consumer_local()
            );
            assert_eq!(
                composition.caller_result_frame_slot(),
                result_binding.consumer_frame_slot()
            );
            assert_eq!(composition.caller_result_ty(), input_step.complete_ty());

            let input_case = input_step
                .case(composition.input_case_tag())
                .expect("composition input case 应存在于 resume wrapper Step_F");
            let forwarding = lowering
                .dispatch()
                .outward_cases()
                .iter()
                .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
                .expect("composition 应对应一个 resume dispatch forwarding");

            assert_eq!(
                composition.callee_continuation_contract(),
                input_case.continuation_contract()
            );
            assert_eq!(
                composition.output_case_tag(),
                forwarding.emission().case_tag()
            );
            assert_eq!(
                composition.caller_continuation_contract(),
                forwarding.emission().continuation_contract()
            );
        }

        let rendered = crate::effect_lowered::render_late_lowered_program(output.program());
        assert!(
            rendered.contains("lowering: Resume")
                && rendered.contains("continuation_compositions:"),
            "dump-effect-lowered 应渲染 resume-boundary continuation composition handoff"
        );
    }

    #[test]
    fn refactor_effect_lowered_resume_payload_binding_dump_exposes_consumers() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("resume_payload_bindings:"));
        assert!(dump.contains("bd0 resume=st2"));
        assert!(dump.contains("home=slot"));
    }

    #[test]
    fn refactor_effect_lowered_completion_payload_contract_publishes_non_unit_return_source() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let run = callable(&output, "run");
        let step_type = output
            .program()
            .step_type(run.step_schema())
            .expect("run 应能回查 Step shell");
        let (return_state, payload_source, complete_state) = run
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::Return {
                    payload_source,
                    complete_state,
                } => Some((state.state_id(), payload_source, *complete_state)),
                _ => None,
            })
            .expect("run(): Int 应发布 Return terminator");
        let binding = run
            .frame_schema()
            .completion_payload_binding_for_state(return_state)
            .expect("return state 应发布 completion payload binding");

        assert_eq!(complete_state, run.state_graph().complete_state());
        assert_eq!(binding.complete_state(), complete_state);
        assert_eq!(binding.payload_source(), payload_source);
        assert_eq!(payload_source.source_ty(), step_type.complete_ty());
        assert_eq!(
            output
                .types()
                .display(payload_source.source_ty())
                .to_string(),
            "Int"
        );
        assert!(
            !payload_source.is_unit(),
            "non-Unit completion 不应退化成 Unit payload source"
        );
        assert!(matches!(
            payload_source,
            LateLoweredCompletionPayloadSource::Operand(source)
                if matches!(source.value(), LateLoweredOperandValueSource::Local(_))
        ));
    }

    #[test]
    fn refactor_effect_lowered_completion_payload_contract_dump_exposes_sources() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("completion_payload_bindings:"));
        assert!(dump.contains("root: run"));
        assert!(dump.contains("payload=local"));
    }

    #[test]
    fn refactor_effect_lowered_source_slice_classification_publishes_statement_purposes() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let classes = output
            .program()
            .callables()
            .iter()
            .flat_map(|callable| callable.source_statement_classifications())
            .map(|classification| classification.kind())
            .collect::<Vec<_>>();

        assert!(classes.iter().any(|kind| matches!(
            kind,
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        )));
        assert!(classes.iter().any(|kind| matches!(
            kind,
            LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
        )));
        assert!(classes.iter().any(|kind| matches!(
            kind,
            LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
        )));

        let dump = output.program().stable_dump();
        assert!(dump.contains("statement_classification:"));
        assert!(dump.contains("effect-neutral-value"));
        assert!(dump.contains("boundary-consumed-anchor"));
    }

    #[test]
    fn refactor_effect_lowered_completion_payload_contract_rejects_type_drift() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let run = callable(&output, "run");
        let step_type = output
            .program()
            .step_type(run.step_schema())
            .expect("run 应能回查 Step shell");
        let builtins = output.types().builtins().expect("builtins 应已 intern");
        let wrong_step_type = LateLoweredStepType::new(
            step_type.step_schema(),
            step_type.invoke_args_tuple_ty(),
            builtins.unit,
            step_type.continuation_obj_ty(),
            step_type.cases().to_vec(),
        );

        let err = super::materialize_completion_payload_bindings(
            run.root_fqn(),
            &wrong_step_type,
            run.state_graph(),
            run.frame_schema(),
            output.types(),
        )
        .expect_err("completion payload type drift 必须 fail fast");
        assert!(
            err.to_string().contains("completion payload contract")
                && err.to_string().contains("complete_ty"),
            "错误消息应指出 completion payload complete_ty 漂移: {err}"
        );
    }

    #[test]
    fn published_continuation_provenance_rejects_ambiguous_member_routes() {
        let mut types = crate::ty::TypeStore::default();
        let builtins = types.intern_builtins();
        let span = crate::span::Span::new(0, 0);
        let step_schema = crate::effect_facts::StepSchemaId::new(0);
        let empty_cases = crate::effect_facts::CaseSet::new(step_schema, Vec::new());

        let mut body = crate::mir::Body::new_empty();
        let cell = body.push_local(crate::mir::LocalDecl {
            span,
            name: Some("cell".to_string()),
            ty: builtins.any,
            source: crate::mir::LocalSourceKind::SourceLocal,
        });
        let k0 = body.push_local(crate::mir::LocalDecl {
            span,
            name: Some("k0".to_string()),
            ty: builtins.any,
            source: crate::mir::LocalSourceKind::SourceLocal,
        });
        let k1 = body.push_local(crate::mir::LocalDecl {
            span,
            name: Some("k1".to_string()),
            ty: builtins.any,
            source: crate::mir::LocalSourceKind::SourceLocal,
        });
        let read_local = body.push_local(crate::mir::LocalDecl {
            span,
            name: Some("read".to_string()),
            ty: builtins.any,
            source: crate::mir::LocalSourceKind::CompilerTemporary,
        });
        let resume_local = body.push_local(crate::mir::LocalDecl {
            span,
            name: Some("resume".to_string()),
            ty: builtins.any,
            source: crate::mir::LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: crate::mir::Terminator {
                span,
                kind: crate::mir::TerminatorKind::Unreachable,
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        });
        let bb1 = body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: crate::mir::Terminator {
                span,
                kind: crate::mir::TerminatorKind::Unreachable,
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        });
        let bb2 = body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: crate::mir::Terminator {
                span,
                kind: crate::mir::TerminatorKind::Unreachable,
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        });
        let bb3 = body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: crate::mir::Terminator {
                span,
                kind: crate::mir::TerminatorKind::Unreachable,
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        let member = crate::mir::MemberAccessMetadata {
            name: "k".to_string(),
            receiver_ty: builtins.any,
            resolved: Some(crate::mir::MemberTarget::Value {
                fqn: "Cell.k".to_string(),
            }),
            hidden_effects: crate::ty::EffectRow::pure(),
        };
        body.blocks[bb0.as_u32() as usize].stmts = vec![
            crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::StoreMember {
                    receiver: crate::mir::Operand::Local(cell),
                    member: member.clone(),
                    value: crate::mir::Operand::Local(k0),
                    value_ty: builtins.any,
                    continuation_route: crate::mir::StoredContinuationRoutePublication::Unique(
                        crate::mir::StoredContinuationValueRoute {
                            source_local: k0,
                            source_ty: builtins.any,
                            path: vec![crate::mir::PatternBindingStep::VariantField {
                                variant: "Some".to_string(),
                                field_index: 0,
                            }],
                        },
                    ),
                },
            },
            crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::StoreMember {
                    receiver: crate::mir::Operand::Local(cell),
                    member: member.clone(),
                    value: crate::mir::Operand::Local(k1),
                    value_ty: builtins.any,
                    continuation_route: crate::mir::StoredContinuationRoutePublication::Unique(
                        crate::mir::StoredContinuationValueRoute {
                            source_local: k1,
                            source_ty: builtins.any,
                            path: vec![crate::mir::PatternBindingStep::VariantField {
                                variant: "Some".to_string(),
                                field_index: 0,
                            }],
                        },
                    ),
                },
            },
            crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::Assign {
                    target: read_local,
                    value: crate::mir::Rvalue::MemberAccess {
                        site_id: None,
                        receiver: crate::mir::Operand::Local(cell),
                        member: member.clone(),
                    },
                },
            },
            crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::Assign {
                    target: resume_local,
                    value: crate::mir::Rvalue::PatternExtract {
                        subject: crate::mir::Operand::Local(read_local),
                        path: vec![crate::mir::PatternBindingStep::VariantField {
                            variant: "Some".to_string(),
                            field_index: 0,
                        }],
                    },
                },
            },
        ];
        body.blocks[bb0.as_u32() as usize].terminator = crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Handle {
                site_id: SiteId::from_raw(0),
                metadata: crate::mir::HandleMetadata {
                    result_ty: builtins.any,
                    body_result_ty: builtins.any,
                    finally_result_ty: None,
                },
                arms: vec![
                    crate::mir::HandlerArm {
                        op_fqn: "sample.Ask.ask".to_string(),
                        binder_count: 0,
                        binder_locals: Vec::new(),
                        continuation_local: Some(k0),
                        handled_effect_ty: builtins.any,
                        payload_tuple_ty: Some(builtins.unit),
                        payload_component_tys: Vec::new(),
                        body_ty: builtins.any,
                        kind: crate::mir::HandlerArmKind::EscapeContinuation,
                    },
                    crate::mir::HandlerArm {
                        op_fqn: "sample.Ask.ask".to_string(),
                        binder_count: 0,
                        binder_locals: Vec::new(),
                        continuation_local: Some(k1),
                        handled_effect_ty: builtins.any,
                        payload_tuple_ty: Some(builtins.unit),
                        payload_component_tys: Vec::new(),
                        body_ty: builtins.any,
                        kind: crate::mir::HandlerArmKind::EscapeContinuation,
                    },
                ],
                has_finally: false,
                body_target: bb1,
                arm_targets: vec![bb2, bb3],
                finally_target: None,
                exit_target: bb1,
            },
            unwind: crate::mir::UnwindAction::NoUnwind,
        };

        let body_facts = crate::effect_facts::BodyEffectFacts::new(
            std::collections::BTreeMap::new(),
            std::collections::BTreeMap::from([(
                SiteId::from_raw(0),
                crate::effect_facts::SiteEffectFacts::Handle(
                    crate::effect_facts::HandleSiteEffectFacts::new(
                        builtins.any,
                        crate::effect_facts::CaseSet::new(
                            step_schema,
                            vec![
                                crate::effect_facts::CaseTag::new(0),
                                crate::effect_facts::CaseTag::new(1),
                            ],
                        ),
                        empty_cases.clone(),
                        vec![
                            crate::effect_facts::HandleArmEffectFacts::new(
                                crate::effect_facts::CaseTag::new(0),
                                builtins.unit,
                                crate::effect_facts::ContinuationSchemaId::new(0),
                                empty_cases.clone(),
                            ),
                            crate::effect_facts::HandleArmEffectFacts::new(
                                crate::effect_facts::CaseTag::new(1),
                                builtins.unit,
                                crate::effect_facts::ContinuationSchemaId::new(1),
                                empty_cases.clone(),
                            ),
                        ],
                        empty_cases,
                        crate::effect_facts::NestedHandleClassification::SelfContained,
                    ),
                ),
            )]),
        );
        let owner_version_key = crate::effect_lowered::ir::LateLoweredBodyVersionKey::new(
            crate::mir::InstanceKey {
                template: crate::mir::TemplateKey {
                    fqn: "synthetic.main".to_string(),
                    source_path: PathBuf::from("<synthetic>"),
                    decl_span: span,
                },
                type_args: Vec::new(),
                eff_args: Vec::new(),
            },
            crate::ty::EffectRow::pure(),
            crate::effect_facts::ImplPlan::NoOutward,
            false,
        );
        let provenance = super::PublishedContinuationProvenance::build(
            "synthetic.main",
            &body,
            &body_facts,
            &owner_version_key,
            crate::effect_lowered::ir::ContinuationObjectId::new(0),
            None,
        )
        .expect("synthetic provenance builder 应成功");

        let err = provenance
            .resolve_resume_local_route("synthetic.main", SiteId::from_raw(9), resume_local)
            .expect_err("多个不兼容 source route 必须显式拒绝");
        let message = err.to_string();
        assert!(
            message.contains("多条互不兼容") || message.contains("无法唯一确定"),
            "错误消息应指出 member readback provenance 歧义: {message}"
        );
    }

    #[test]
    fn refactor_boundary_lowering_materializes_perform_and_handle_contracts() {
        let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
        let handled_main = callable(&perform_output, "a.main");
        let perform_boundary = site_boundary(handled_main, BoundarySiteKind::Perform);
        let LateLoweredBoundaryLowering::Perform(perform_lowering) = perform_boundary
            .lowering()
            .expect("perform boundary 应发布 lowering contract")
        else {
            panic!("perform boundary 应物化成 Perform lowering")
        };
        assert_eq!(
            perform_lowering.facts().emitted_case(),
            perform_lowering.emitted_step().case_tag()
        );
        assert_eq!(
            perform_lowering
                .emitted_step()
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );

        let handle_output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let outward = callable(&handle_output, "sample.nested_may_suspend_outward");
        let handle_boundary = site_boundary(outward, BoundarySiteKind::Handle);
        let LateLoweredBoundaryLowering::Handle(handle_lowering) = handle_boundary
            .lowering()
            .expect("handle boundary 应发布 lowering contract")
        else {
            panic!("handle boundary 应物化成 Handle lowering")
        };
        assert_eq!(
            handle_lowering.facts().nested_handle_classification(),
            NestedHandleClassification::MaySuspendOutward
        );
        assert_eq!(handle_lowering.outward_emissions().len(), 1);
        assert_eq!(
            handle_lowering.outward_emissions()[0]
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "sample.Outer.again"
        );
    }

    #[test]
    fn refactor_handle_dispatch_contract_publishes_body_arm_finally_and_outward_routes() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let callable = callable(&output, "sample.nested_may_suspend_outward");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
        let LateLoweredStateTerminator::HandleDispatch {
            arm_states,
            finally_state,
            exit_state,
            contract,
            ..
        } = handle_state.terminator()
        else {
            panic!("指定 state 应保持 HandleDispatch terminator");
        };

        assert_eq!(
            contract.carrier().state_tag_slot(),
            SystemSlotKind::StateTag
        );
        assert_eq!(
            contract.carrier().completion_tag_slot(),
            SystemSlotKind::CompletionTag
        );
        assert_eq!(
            contract.carrier().payload_carrier_slot(),
            SystemSlotKind::ResumePayloadCarrier
        );
        assert_eq!(
            contract.body_complete_target(),
            finally_state.expect("fixture 应保留 finally state")
        );
        assert_eq!(
            contract.arm_complete_target(),
            finally_state.expect("fixture 应保留 finally state")
        );
        assert_eq!(contract.finally_complete_target(), Some(*exit_state));
        assert_eq!(
            contract.abandon_target(),
            callable.state_graph().drop_state()
        );
        assert_eq!(contract.handled_arms().len(), 1);
        assert_eq!(contract.handled_arms()[0].handled_case().as_u32(), 0);
        assert_eq!(contract.handled_arms()[0].arm_state(), arm_states[0]);
        assert!(contract.handled_arms()[0].arm_outward_cases().is_empty());
        assert!(contract.body_outward_cases().is_empty());
        assert_eq!(
            contract.finally_outward_cases(),
            &[crate::effect_facts::CaseTag::new(1)]
        );
        assert!(
            contract
                .outward_emission(crate::effect_facts::CaseTag::new(1))
                .is_some(),
            "finally outward case 应能回查 published outward emission"
        );
        assert!(
            contract
                .pending_completions()
                .contains(&LateLoweredHandlePendingCompletion::ContinueToExit)
        );
        assert!(
            contract
                .pending_completions()
                .contains(&LateLoweredHandlePendingCompletion::ReturnFromFunction)
        );
        assert!(
            !contract.pending_completions().contains(
                &LateLoweredHandlePendingCompletion::PropagateOutward(
                    crate::effect_facts::CaseTag::new(1)
                )
            ),
            "仅 finally outward 的 case 不应被误发布成 pending completion tag"
        );
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_publishes_body_routing_for_handled_perform() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let callable = callable(&output, "run");
        let (_site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("run 应发布 HandleDispatch contract");
        let handled_arm = contract
            .handled_arms()
            .first()
            .expect("single-perform fixture 应发布唯一 handled arm");
        let body_route = contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                    && callable
                        .boundary_map()
                        .boundary(routing.boundary_id())
                        .is_some_and(|boundary| {
                            matches!(
                                boundary.source(),
                                crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                                    kind: BoundarySiteKind::Perform,
                                    ..
                                }
                            )
                        })
            })
            .expect("handle body 内的 perform boundary 应发布 body-region routing");
        let route = body_route
            .case_routing(handled_arm.handled_case())
            .expect("handled perform case 应发布 consume-to-arm routing");

        assert_eq!(
            contract.state_region(body_route.owner_state()),
            LateLoweredHandleStateRegion::Body
        );
        assert_eq!(
            contract.state_region(body_route.resume_state()),
            LateLoweredHandleStateRegion::Body
        );
        assert!(matches!(
            route.action(),
            LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                arm_state,
                arm_ordinal,
                continuation_resume_state,
            } if arm_state == handled_arm.arm_state()
                && arm_ordinal == handled_arm.arm_ordinal()
                && continuation_resume_state == body_route.resume_state()
        ));
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_tracks_multi_resume_routes_and_arm_regions() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let (_site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("main 应发布 HandleDispatch contract");
        let ask_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_some())
            .expect("Ask arm 应发布 escape continuation binder");
        let consume_routes = contract
            .boundary_routings()
            .iter()
            .filter_map(|routing| {
                routing
                    .case_routing(ask_arm.handled_case())
                    .map(|route| (routing, route))
            })
            .collect::<Vec<_>>();
        let resume_states = consume_routes
            .iter()
            .map(|(routing, route)| match route.action() {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    continuation_resume_state,
                } => {
                    assert_eq!(arm_state, ask_arm.arm_state());
                    assert_eq!(arm_ordinal, ask_arm.arm_ordinal());
                    assert_eq!(continuation_resume_state, routing.resume_state());
                    continuation_resume_state
                }
                other => panic!("Ask handled case 应走 consume-to-arm，而不是 {other:?}"),
            })
            .collect::<BTreeSet<_>>();

        assert!(
            consume_routes.len() >= 2,
            "indirect/direct mixed fixture 应至少发布两个 Ask consume route"
        );
        assert!(
            resume_states.len() >= 2,
            "不同 body boundary 的 continuation resume_state 应被稳定区分"
        );
        assert!(
            resume_states
                .iter()
                .all(|state_id| contract.state_region(*state_id)
                    == LateLoweredHandleStateRegion::Body)
        );
        assert!(contract.state_regions().iter().any(|entry| matches!(
            entry.region(),
            LateLoweredHandleStateRegion::Arm { arm_ordinal: 0, .. }
        )));
        assert!(contract.state_regions().iter().any(|entry| matches!(
            entry.region(),
            LateLoweredHandleStateRegion::Arm { arm_ordinal: 1, .. }
        )));
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_tracks_pending_and_finally_routing() {
        let pending_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_region_pending.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let pending_callable = callable(&pending_output, "sample.propagate_before_finally");
        let pending_contract =
            match handle_dispatch_state(pending_callable, SiteId::from_raw(1)).terminator() {
                LateLoweredStateTerminator::HandleDispatch { contract, .. } => contract,
                other => panic!("期望 HandleDispatch terminator，而不是 {other:?}"),
            };
        let pending_case = pending_contract.body_outward_cases()[0];
        let pending_route = pending_contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                    && routing.case_routing(pending_case).is_some()
            })
            .expect("body outward case 应发布 pending routing");
        assert!(matches!(
            pending_route
                .case_routing(pending_case)
                .expect("pending case 应可回查")
                .action(),
            LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                completion: LateLoweredHandlePendingCompletion::PropagateOutward(case_tag),
            } if case_tag == pending_case
        ));

        let finally_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_region_finally_outward.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun finally_outward(): Int / (Outer) {
    return handle {
        Inner.go()
        0
    } with {
        Inner.go() -> 1
    } finally {
        Outer.again()
    }
}
"#,
        ));
        let finally_callable = callable(&finally_output, "sample.finally_outward");
        let (_site_id, finally_contract) = finally_callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("finally_outward 应发布 HandleDispatch contract");
        let finally_case = finally_contract.finally_outward_cases()[0];
        let finally_route = finally_contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(
                    routing.owner_region(),
                    LateLoweredHandleStateRegion::Finally
                ) && routing.case_routing(finally_case).is_some()
            })
            .expect("finally outward case 应发布 finally-region routing");
        assert!(matches!(
            finally_route
                .case_routing(finally_case)
                .expect("finally case 应可回查")
                .action(),
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        ));
    }

    #[test]
    fn refactor_handle_arm_binding_contract_publishes_payload_and_escape_continuation_binding() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_arm_binding_single.scoop",
            r#"
package sample

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun run(): Int {
    return handle {
        Edge.visit("alpha", 1)
    } with {
        Edge.visit(from, to), k -> {
            k.resume(to + 1)
        }
    }
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.run");
        let (site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("run 应发布 HandleDispatch contract");
        let arm = contract
            .handled_arms()
            .first()
            .expect("单 arm fixture 应发布唯一 handled arm");
        let facts = handle_site_facts(&output, callable, site_id);
        let expected = &facts.arm_facts()[0];

        assert_eq!(arm.arm_ordinal(), 0);
        assert_eq!(arm.payload_tuple_ty(), expected.payload_tuple_ty());
        assert_eq!(arm.payload_binders().len(), 2);
        assert_eq!(arm.payload_binders()[0].ordinal(), 0);
        assert_eq!(arm.payload_binders()[1].ordinal(), 1);
        assert_ne!(
            arm.payload_binders()[0].local(),
            arm.payload_binders()[1].local(),
            "不同 payload binder 必须稳定绑定到不同 local"
        );
        let continuation_binder = arm
            .continuation_binder()
            .expect("escape continuation arm 必须发布 continuation binder contract");
        assert_eq!(
            continuation_binder.continuation_schema(),
            expected.continuation_schema()
        );
        assert_eq!(
            continuation_binder.continuation_object(),
            callable.continuation_object()
        );
    }

    #[test]
    fn refactor_handle_arm_binding_contract_publishes_mixed_multi_arm_bindings_without_ambiguity() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let (site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("main 应发布 HandleDispatch contract");
        let facts = handle_site_facts(&output, callable, site_id);

        assert_eq!(contract.handled_arms().len(), 2);
        let mut arm_ordinals = contract
            .handled_arms()
            .iter()
            .map(|arm| arm.arm_ordinal())
            .collect::<Vec<_>>();
        arm_ordinals.sort();
        assert_eq!(arm_ordinals, vec![0, 1]);

        let escape_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_some())
            .expect("mixed fixture 应发布带 continuation binder 的 arm");
        let payload_only_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_none())
            .expect("mixed fixture 应发布纯 payload arm");
        assert_eq!(escape_arm.payload_binders().len(), 1);
        assert_eq!(payload_only_arm.payload_binders().len(), 1);

        let expected_by_case = facts
            .arm_facts()
            .iter()
            .map(|arm| (arm.handled_case(), arm.continuation_schema()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            escape_arm
                .continuation_binder()
                .expect("escape arm 应带 continuation binder")
                .continuation_schema(),
            *expected_by_case
                .get(&escape_arm.handled_case())
                .expect("handled case 应能回查 arm facts continuation schema")
        );
        assert_eq!(
            payload_only_arm.payload_tuple_ty(),
            facts
                .arm_facts()
                .iter()
                .find(|arm| arm.handled_case() == payload_only_arm.handled_case())
                .expect("payload-only arm handled case 应能回查 facts")
                .payload_tuple_ty()
        );
    }

    #[test]
    fn refactor_completion_state_contract_tracks_body_outward_cases_across_finally() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_body_outward_finally.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let callable = callable(&output, "sample.propagate_before_finally");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("指定 state 应保持 HandleDispatch terminator");
        };

        assert_eq!(contract.body_outward_cases().len(), 1);
        let outward_case = contract.body_outward_cases()[0];
        assert!(contract.finally_outward_cases().is_empty());
        assert!(contract.pending_completions().contains(
            &LateLoweredHandlePendingCompletion::PropagateOutward(outward_case,)
        ));
        assert!(contract.outward_emission(outward_case).is_some());
    }

    #[test]
    fn refactor_handle_dispatch_contract_publishes_pending_payload_transport_across_finally() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_pending_payload_transport.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let callable = callable(&output, "sample.propagate_before_finally");
        let (site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } if contract.pending_completions().iter().any(|completion| {
                    matches!(
                        completion,
                        LateLoweredHandlePendingCompletion::PropagateOutward(_)
                    )
                }) =>
                {
                    Some((*site_id, contract))
                }
                _ => None,
            })
            .expect("fixture 应发布带 pending outward completion 的 HandleDispatch contract");

        let pending_case = *contract
            .body_outward_cases()
            .first()
            .expect("fixture 应发布 body outward case");
        let transport = contract
            .pending_payload_transport(LateLoweredHandlePendingCompletion::PropagateOutward(
                pending_case,
            ))
            .expect("pending outward case 应发布 typed payload transport");
        let slot = callable
            .frame_schema()
            .slot_for_kind(
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                    site_id,
                    case_tag: pending_case,
                },
            )
            .expect("frame schema 应保留 HandlePendingPayload slot");
        let emission = contract
            .outward_emission(pending_case)
            .expect("pending outward case 应保留 outward emission contract");

        assert_eq!(transport.frame_slot(), slot.slot_id());
        assert_eq!(transport.payload_tuple_ty(), slot.ty());
        assert_eq!(transport.payload_tuple_ty(), emission.payload_tuple_ty());
        assert!(
            contract
                .pending_payload_transport(LateLoweredHandlePendingCompletion::ContinueToExit)
                .is_none()
        );
    }

    #[test]
    fn refactor_handle_dispatch_contract_dump_exposes_published_completion_state() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_contract_dump.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("handle_contract:"));
        assert!(dump.contains("pending_completions:"));
        assert!(dump.contains("pending_payload_transports:"));
        assert!(dump.contains("state_regions:"));
        assert!(dump.contains("boundary_routings:"));
        assert!(dump.contains("case_routings:"));
        assert!(dump.contains("PropagateOutward("));
        assert!(dump.contains("HandlePendingPayload("));
        assert!(dump.contains("outward_emissions:"));
    }

    #[test]
    fn refactor_handle_arm_binding_contract_dump_exposes_payload_and_continuation_binders() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("payload_binders:"));
        assert!(dump.contains("continuation_binder:"));
        assert!(dump.contains("continuation_schema="));
    }

    #[test]
    fn refactor_impl_plan_lowering_keeps_no_outward_single_case_and_canonical_full_distinct() {
        let no_outward_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_no_outward.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        ));
        let no_outward = callable(&no_outward_output, "sample.main");
        assert_eq!(no_outward.impl_plan(), ImplPlan::NoOutward);
        assert_eq!(no_outward.call_abi_kind(), CallableAbiKind::Plain);
        assert!(no_outward.body_step_schema().is_none());
        assert!(no_outward.effect_step_abi().is_none());
        assert!(no_outward.plain_abi().is_some());

        let single_case_output =
            load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let single_case = callable(&single_case_output, "sample.leaf");
        let single_case_object = single_case_output
            .program()
            .continuation_object(single_case.continuation_object())
            .expect("single-case callable 应能回查 continuation object");
        assert!(matches!(single_case.impl_plan(), ImplPlan::SingleCase(_)));
        assert_eq!(
            single_case_object
                .methods()
                .iter()
                .filter(|method| {
                    method.reachability() == LateLoweredContinuationMethodReachability::Reachable
                })
                .count(),
            1
        );

        let canonical_output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let canonical = callable(&canonical_output, "sample.callValue");
        let canonical_boundary = site_boundary(canonical, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(canonical_lowering) = canonical_boundary
            .lowering()
            .expect("canonical-full boundary 应发布 lowering contract")
        else {
            panic!("canonical-full boundary 应物化成 Call lowering")
        };
        assert_eq!(canonical.impl_plan(), ImplPlan::CanonicalFull);
        assert_eq!(canonical_lowering.dispatch().outward_cases().len(), 2);
    }
}
