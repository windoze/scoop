//! Surface resume dispatch, owner trampolines, and wrapper projections.
//!
//! The surface resume layer exposes resume packings as user-callable methods.
//! This module materializes the per-method targets, the per-owner trampoline
//! candidates that need a published wrapper projection, and the validation
//! that ties wrapper projections back to handle binders.

use super::payload::same_completion_payload_source_ignoring_span;
use super::*;

pub(super) struct SurfaceResumeOwnerTrampolineCandidate {
    owner_version_key: LateLoweredBodyVersionKey,
    owner_continuation_object: ContinuationObjectId,
    resume_boundary_sites: BTreeSet<SiteId>,
    handle_binder_routes: BTreeSet<(SiteId, u32, crate::effect_facts::CaseTag)>,
    wrapper_projection: Option<LateLoweredSurfaceResumeWrapperProjection>,
    has_method_target: bool,
}

impl SurfaceResumeOwnerTrampolineCandidate {
    pub(super) fn new(
        owner_version_key: LateLoweredBodyVersionKey,
        owner_continuation_object: ContinuationObjectId,
    ) -> Self {
        Self {
            owner_version_key,
            owner_continuation_object,
            resume_boundary_sites: BTreeSet::new(),
            handle_binder_routes: BTreeSet::new(),
            wrapper_projection: None,
            has_method_target: false,
        }
    }

    pub(super) fn add_publication(
        &mut self,
        continuation_schema: ContinuationSchemaId,
        publication: &LateLoweredSurfaceResumeDispatchPublication,
    ) -> Result<(), LlvmEmitError> {
        match publication {
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_version_key,
                owner_continuation_object,
                site_id,
            } => {
                self.validate_owner(
                    continuation_schema,
                    owner_version_key,
                    *owner_continuation_object,
                )?;
                self.resume_boundary_sites.insert(*site_id);
            }
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_version_key,
                owner_continuation_object,
                site_id,
                arm_ordinal,
                handled_case,
            } => {
                self.validate_owner(
                    continuation_schema,
                    owner_version_key,
                    *owner_continuation_object,
                )?;
                self.handle_binder_routes
                    .insert((*site_id, *arm_ordinal, *handled_case));
            }
            LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { .. }
            | LateLoweredSurfaceResumeDispatchPublication::InternalMethod { .. } => {}
        }
        Ok(())
    }

    pub(super) fn set_wrapper_projection(
        &mut self,
        continuation_schema: ContinuationSchemaId,
        projection: LateLoweredSurfaceResumeWrapperProjection,
    ) -> Result<(), LlvmEmitError> {
        if let Some(existing) = &self.wrapper_projection {
            if !same_surface_resume_wrapper_projection_shape(existing, &projection) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} owner ko{} 的 owner-step -> wrapper-step projection contract 歧义：published={existing:?}，new={projection:?}",
                    continuation_schema.as_u32(),
                    self.owner_continuation_object.as_u32(),
                )));
            }
            return Ok(());
        }
        self.wrapper_projection = Some(projection);
        Ok(())
    }

    pub(super) fn validate_owner(
        &self,
        continuation_schema: ContinuationSchemaId,
        owner_version_key: &LateLoweredBodyVersionKey,
        owner_continuation_object: ContinuationObjectId,
    ) -> Result<(), LlvmEmitError> {
        if &self.owner_version_key != owner_version_key
            || self.owner_continuation_object != owner_continuation_object
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 漂移：candidate ko{} 与 publication ko{} 不一致",
                continuation_schema.as_u32(),
                self.owner_continuation_object.as_u32(),
                owner_continuation_object.as_u32(),
            )));
        }
        Ok(())
    }
}

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_surface_resume_dispatch_layouts(
        &mut self,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
        continuation_layouts: &BTreeMap<ContinuationObjectId, ContinuationObjectLayout<'ctx>>,
        resume_packing_layouts: &BTreeMap<ResumeInterfaceId, ResumeInterfaceLayout<'ctx>>,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
    ) -> Result<
        BTreeMap<ContinuationSchemaId, ContinuationSurfaceResumeDispatchLayout<'ctx>>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for entry in self.program.surface_resume_dispatch_inventory() {
            let continuation_schema = entry.continuation_schema();
            let surface_layout = surface_resume_layouts
                .get(&continuation_schema)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 continuation schema k{} 的 surface-resume layout，无法发布 owner dispatch contract",
                        continuation_schema.as_u32(),
                    ))
                })?;
            let method_targets = match entry.source_kind() {
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod => self
                    .materialize_surface_resume_method_targets(
                        entry,
                        surface_layout,
                        continuation_layouts,
                        resume_packing_layouts,
                    )?,
                _ => Vec::new(),
            };
            let target = match entry.source_kind() {
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable => {
                    ContinuationSurfaceResumeDispatchTarget::Unreachable
                }
                _ => {
                    let mut targets = self.materialize_surface_resume_owner_trampoline_layouts(
                        entry,
                        surface_layout,
                        callable_layouts,
                        frame_layouts,
                        &method_targets,
                    )?;
                    if targets.len() == 1 {
                        ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(Box::new(
                            targets.remove(0),
                        ))
                    } else {
                        ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets)
                    }
                }
            };
            if layouts
                .insert(
                    continuation_schema,
                    ContinuationSurfaceResumeDispatchLayout::new(
                        continuation_schema,
                        entry.source_kind(),
                        method_targets,
                        target,
                    ),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 重复发布",
                    continuation_schema.as_u32(),
                )));
            }
        }
        Ok(layouts)
    }

    pub(super) fn materialize_surface_resume_method_targets(
        &self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        surface_layout: &ContinuationSurfaceResumeLayout<'ctx>,
        continuation_layouts: &BTreeMap<ContinuationObjectId, ContinuationObjectLayout<'ctx>>,
        resume_packing_layouts: &BTreeMap<ResumeInterfaceId, ResumeInterfaceLayout<'ctx>>,
    ) -> Result<Vec<ContinuationSurfaceResumeMethodLookup>, LlvmEmitError> {
        let mut candidates = BTreeSet::new();
        for publication in entry.publications() {
            let LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                object_id,
                packing_interface_id,
                case_tag,
                reachability,
            } = publication
            else {
                continue;
            };
            if *reachability == LateLoweredContinuationMethodReachability::Reachable {
                candidates.insert((*object_id, *packing_interface_id, *case_tag));
            }
        }

        let render_candidates = || {
            if candidates.is_empty() {
                "<none>".to_string()
            } else {
                candidates
                    .iter()
                    .map(|(object_id, interface_id, case_tag)| {
                        format!(
                            "ko{} ri{}::c{}",
                            object_id.as_u32(),
                            interface_id.as_u32(),
                            case_tag.as_u32()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };

        candidates.iter().next().copied().ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 已发布为 ContinuationObjectMethod，但缺少 reachable internal method target",
                entry.continuation_schema().as_u32(),
            ))
        })?;
        let distinct_objects = candidates
            .iter()
            .map(|(object_id, _, _)| *object_id)
            .collect::<BTreeSet<_>>();
        if entry.wrapper_projections().is_empty() && distinct_objects.len() > 1 {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 歧义：多个 continuation object 共享同一 schema [{}]",
                entry.continuation_schema().as_u32(),
                render_candidates(),
            )));
        }

        let mut method_targets = Vec::with_capacity(candidates.len());
        for (object_id, interface_id, case_tag) in candidates {
            let continuation_layout = continuation_layouts.get(&object_id).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} 需要的 continuation object ko{} layout，无法发布 surface-resume owner dispatch contract",
                    entry.continuation_schema().as_u32(),
                    object_id.as_u32(),
                ))
            })?;
            let (binding_schema, expected_return_step_schema) =
                match entry.wrapper_projections().iter().find_map(|projection| {
                    let route = projection.underlying_route();
                    match route.publication() {
                        LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                            object_id: route_object,
                            packing_interface_id: route_interface,
                            case_tag: route_case,
                            reachability: LateLoweredContinuationMethodReachability::Reachable,
                        } if *route_object == object_id
                            && *route_interface == interface_id
                            && *route_case == case_tag =>
                        {
                            Some((route.continuation_schema(), projection.owner_step_schema()))
                        }
                        _ => None,
                    }
                }) {
                    Some(route) => route,
                    None => (
                        entry.continuation_schema(),
                        surface_layout.return_step_schema(),
                    ),
                };
            let packing_field_index = continuation_layout
                .field_index_for_packing(interface_id)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 internal method target ko{} ri{}::c{} 缺少 object-side packing field lookup",
                        entry.continuation_schema().as_u32(),
                        object_id.as_u32(),
                        interface_id.as_u32(),
                        case_tag.as_u32(),
                    ))
                })?;
            let bindings = continuation_layout
                .surface_resume_bindings(binding_schema)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 continuation object ko{} 缺少 object-side surface-resume binding k{}",
                        entry.continuation_schema().as_u32(),
                        object_id.as_u32(),
                        binding_schema.as_u32(),
                    ))
                })?;
            if !bindings.iter().any(|binding| {
                binding.case_tag() == case_tag
                    && binding.return_step_schema() == expected_return_step_schema
                    && binding.reachability()
                        == LateLoweredContinuationMethodReachability::Reachable
            }) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} 的 internal method target ko{} ri{}::c{} 缺少匹配的 reachable object-side surface-resume binding k{} -> s{}",
                    entry.continuation_schema().as_u32(),
                    object_id.as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                    binding_schema.as_u32(),
                    expected_return_step_schema.as_u32(),
                )));
            }

            let interface_layout = resume_packing_layouts.get(&interface_id).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} internal method target 需要的 resume packing ri{} layout",
                    entry.continuation_schema().as_u32(),
                    interface_id.as_u32(),
                ))
            })?;
            let method_layout = interface_layout.method(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} internal method target 需要的 resume method ri{}::c{} layout",
                    entry.continuation_schema().as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                ))
            })?;
            if method_layout.return_step_schema() != expected_return_step_schema
                || method_layout.param_count() != surface_layout.param_count()
                || method_layout.resume_payload_abi().is_elided()
                    != surface_layout.resume_payload_abi().is_elided()
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume method lookup contract 漂移：surface=(out_step_schema=s{}, param_count={}, payload_elided={})，method target ko{} ri{}::c{}=(out_step_schema=s{}, expected_out=s{}, param_count={}, payload_elided={})",
                    entry.continuation_schema().as_u32(),
                    surface_layout.return_step_schema().as_u32(),
                    surface_layout.param_count(),
                    surface_layout.resume_payload_abi().is_elided(),
                    object_id.as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                    method_layout.return_step_schema().as_u32(),
                    expected_return_step_schema.as_u32(),
                    method_layout.param_count(),
                    method_layout.resume_payload_abi().is_elided(),
                )));
            }

            method_targets.push(ContinuationSurfaceResumeMethodLookup::new(
                object_id,
                interface_id,
                packing_field_index,
                case_tag,
                method_layout.vtable_index(),
            ));
        }

        Ok(method_targets)
    }

    pub(super) fn materialize_surface_resume_owner_trampoline_layouts(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        surface_layout: &ContinuationSurfaceResumeLayout<'ctx>,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        method_targets: &[ContinuationSurfaceResumeMethodLookup],
    ) -> Result<Vec<ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>>, LlvmEmitError> {
        let mut candidates = Vec::<SurfaceResumeOwnerTrampolineCandidate>::new();

        for lookup in method_targets {
            let object = self
                .program
                .continuation_object(lookup.continuation_object())
                .expect("method target continuation object 应存在");
            let candidate = surface_resume_owner_candidate_mut(
                &mut candidates,
                object.owner_version_key(),
                lookup.continuation_object(),
            );
            candidate.has_method_target = true;
        }

        let projection_owners = entry
            .wrapper_projections()
            .iter()
            .filter_map(
                |projection| match projection.underlying_route().publication() {
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                        owner_version_key,
                        owner_continuation_object,
                        ..
                    }
                    | LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        owner_version_key,
                        owner_continuation_object,
                        ..
                    } => Some((owner_version_key.clone(), *owner_continuation_object)),
                    LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                        object_id,
                        ..
                    } => self
                        .program
                        .continuation_object(*object_id)
                        .map(|object| (object.owner_version_key().clone(), object.object_id())),
                    LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { .. } => None,
                },
            )
            .collect::<Vec<_>>();

        for projection in entry.wrapper_projections() {
            let Some((owner_version_key, owner_continuation_object)) =
                (match projection.underlying_route().publication() {
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                        owner_version_key,
                        owner_continuation_object,
                        ..
                    }
                    | LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        owner_version_key,
                        owner_continuation_object,
                        ..
                    } => Some((owner_version_key.clone(), *owner_continuation_object)),
                    LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                        object_id,
                        ..
                    } => self
                        .program
                        .continuation_object(*object_id)
                        .map(|object| (object.owner_version_key().clone(), object.object_id())),
                    LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { .. } => None,
                })
            else {
                continue;
            };
            let candidate = surface_resume_owner_candidate_mut(
                &mut candidates,
                &owner_version_key,
                owner_continuation_object,
            );
            if !matches!(
                projection.underlying_route().publication(),
                LateLoweredSurfaceResumeDispatchPublication::InternalMethod { .. }
            ) {
                candidate.add_publication(
                    entry.continuation_schema(),
                    projection.underlying_route().publication(),
                )?;
            }
            candidate.set_wrapper_projection(entry.continuation_schema(), projection.clone())?;
        }

        for publication in entry.publications() {
            let Some((published_owner, published_object)) =
                surface_resume_publication_owner_identity(publication)
            else {
                continue;
            };
            if !projection_owners.is_empty()
                && !projection_owners
                    .iter()
                    .any(|(owner, object)| owner == published_owner && *object == published_object)
            {
                continue;
            }
            let candidate = surface_resume_owner_candidate_mut(
                &mut candidates,
                published_owner,
                published_object,
            );
            candidate.add_publication(entry.continuation_schema(), publication)?;
        }

        if candidates.is_empty() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 已发布为 {:?}，但缺少 owner-specific surface-resume dispatch target",
                entry.continuation_schema().as_u32(),
                entry.source_kind(),
            )));
        }

        candidates
            .into_iter()
            .map(|candidate| {
                self.materialize_surface_resume_owner_trampoline_candidate(
                    entry,
                    surface_layout,
                    callable_layouts,
                    frame_layouts,
                    candidate,
                )
            })
            .collect()
    }

    pub(super) fn materialize_surface_resume_owner_trampoline_candidate(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        surface_layout: &ContinuationSurfaceResumeLayout<'ctx>,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        candidate: SurfaceResumeOwnerTrampolineCandidate,
    ) -> Result<ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>, LlvmEmitError> {
        let owner_version_key = candidate.owner_version_key;
        let owner_continuation_object = candidate.owner_continuation_object;
        if !candidate.has_method_target {
            match entry.source_kind() {
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                    if candidate.resume_boundary_sites.is_empty() =>
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 已发布为 ResumeBoundaryOnly，但 owner trampoline contract 缺少 resume boundary site",
                        entry.continuation_schema().as_u32(),
                    )));
                }
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                    if candidate.handle_binder_routes.is_empty() =>
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 已发布为 HandleContinuationBinderOnly，但 owner trampoline contract 缺少 handle binder route",
                        entry.continuation_schema().as_u32(),
                    )));
                }
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
                    if candidate.wrapper_projection.is_none()
                        && (candidate.resume_boundary_sites.is_empty()
                            || candidate.handle_binder_routes.is_empty()) =>
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 已发布为 OwnerTrampolineMixed，但 owner trampoline contract 未同时覆盖 resume boundary 与 handle binder route",
                        entry.continuation_schema().as_u32(),
                    )));
                }
                _ => {}
            }
        }

        let owner_callable = self
            .program
            .callable_by_version_key(&owner_version_key)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} owner trampoline 需要的 owner callable",
                    entry.continuation_schema().as_u32(),
                ))
            })?;
        if owner_callable.continuation_object() != owner_continuation_object {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 owner trampoline contract 漂移：owner callable `{}` 发布 continuation object ko{}，inventory 指向 ko{}",
                entry.continuation_schema().as_u32(),
                owner_callable.root_fqn(),
                owner_callable.continuation_object().as_u32(),
                owner_continuation_object.as_u32(),
            )));
        }
        if let Some(callable_layout) = callable_layouts.get(&owner_callable.step_schema()) {
            if callable_layout.continuation_object() != owner_continuation_object {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} 的 callable layout continuation object 漂移：callable `{}` -> ko{}，owner trampoline inventory -> ko{}",
                    entry.continuation_schema().as_u32(),
                    owner_callable.root_fqn(),
                    callable_layout.continuation_object().as_u32(),
                    owner_continuation_object.as_u32(),
                )));
            }
        } else if owner_callable.effect_step_abi().is_some() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 缺少 continuation schema k{} owner callable `{}` 的 callable layout，无法发布 effect-step owner trampoline contract",
                entry.continuation_schema().as_u32(),
                owner_callable.root_fqn(),
            )));
        }
        let wrapper_projection = self.validate_surface_resume_wrapper_projection(
            entry,
            owner_callable,
            frame_layouts,
            candidate.wrapper_projection.as_ref(),
        )?;

        let stable_owner_dispatch_key_text = canonical_record(
            "surface_resume_owner_dispatch",
            [
                stable_naming::callable_version_key_text(
                    self.codegen.stable_cone_key,
                    self.source_types,
                    self.codegen.stable_type_param_resolver(),
                    self.program,
                    &owner_version_key,
                    &format!(
                        "continuation schema {} owner trampoline `{}`",
                        entry.continuation_schema().as_u32(),
                        owner_callable.root_fqn()
                    ),
                )?,
                surface_layout.stable_continuation_key_text().to_string(),
            ],
        );
        let symbol_name = stable_naming::private_name_from_key_text(
            "surface_resume_owner_dispatch",
            &stable_owner_dispatch_key_text,
        );
        self.ensure_declared_compiler_private_helper_function(
            &symbol_name,
            surface_layout.llvm_ty(),
        );

        Ok(ContinuationSurfaceResumeOwnerTrampolineLayout::new(
            owner_version_key,
            owner_callable.root_fqn().to_string(),
            owner_callable.step_schema(),
            owner_continuation_object,
            stable_owner_dispatch_key_text,
            symbol_name,
            surface_layout.llvm_ty(),
            surface_layout.param_count(),
            candidate.resume_boundary_sites.into_iter().collect(),
            candidate
                .handle_binder_routes
                .into_iter()
                .map(|(site_id, arm_ordinal, handled_case)| {
                    ContinuationSurfaceResumeHandleBinderRoute::new(
                        site_id,
                        arm_ordinal,
                        handled_case,
                    )
                })
                .collect(),
            wrapper_projection,
        ))
    }

    pub(super) fn validate_surface_resume_wrapper_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        published_projection: Option<&LateLoweredSurfaceResumeWrapperProjection>,
    ) -> Result<Option<LateLoweredSurfaceResumeWrapperProjection>, LlvmEmitError> {
        let mut derived_candidates = Vec::<LateLoweredSurfaceResumeWrapperProjection>::new();

        for boundary in owner_callable.boundary_map().entries() {
            let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
                continue;
            };
            if lowering.facts().continuation_schema() != entry.continuation_schema() {
                continue;
            }
            let derived = self.derive_surface_resume_wrapper_projection(
                entry,
                owner_callable,
                frame_layouts,
                lowering,
            )?;
            let Some(derived) = derived else {
                continue;
            };
            if !derived_candidates
                .iter()
                .any(|candidate| same_surface_resume_wrapper_projection_shape(candidate, &derived))
            {
                derived_candidates.push(derived);
            }
        }

        if derived_candidates.len() > 1 {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 owner-step -> wrapper-step projection contract 歧义：不同 resume boundary 发布了多个 shared surface wrapper projection",
                entry.continuation_schema().as_u32(),
            )));
        }

        match (published_projection, derived_candidates.pop()) {
            (Some(published), Some(derived)) => {
                if !same_surface_resume_wrapper_projection_shape(published, &derived) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 owner-step -> wrapper-step projection contract 漂移：published={published:?}，derived={derived:?}",
                        entry.continuation_schema().as_u32(),
                    )));
                }
                Ok(Some(published.clone()))
            }
            (None, Some(derived)) => Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 已桥接到 underlying route k{}，但缺少 published owner-step -> wrapper-step projection contract：derived={derived:?}",
                entry.continuation_schema().as_u32(),
                derived.underlying_route().continuation_schema().as_u32(),
            ))),
            (Some(published), None) => {
                self.validate_surface_resume_wrapper_complete_projection(
                    entry,
                    owner_callable,
                    frame_layouts,
                    published.complete(),
                )?;
                Ok(Some(published.clone()))
            }
            (None, None) => Ok(None),
        }
    }

    pub(super) fn derive_surface_resume_wrapper_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
    ) -> Result<Option<LateLoweredSurfaceResumeWrapperProjection>, LlvmEmitError> {
        let underlying_route = lowering.operand_contract().underlying_continuation_route();
        let callable_owner_step = self
            .program
            .step_type(owner_callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 owner step schema s{}",
                    entry.continuation_schema().as_u32(),
                    owner_callable.step_schema().as_u32(),
                ))
            })?;
        let wrapper_step = self
            .program
            .step_type(lowering.facts().out_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 wrapper step schema s{}",
                    entry.continuation_schema().as_u32(),
                    lowering.facts().out_step_schema().as_u32(),
                ))
            })?;
        if underlying_route.continuation_schema() == entry.continuation_schema()
            && callable_owner_step.step_schema() == wrapper_step.step_schema()
        {
            return Ok(None);
        }
        let underlying_inventory = self
            .program
            .surface_resume_dispatch(underlying_route.continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 underlying route schema k{} inventory",
                    entry.continuation_schema().as_u32(),
                    underlying_route.continuation_schema().as_u32(),
                ))
            })?;
        let owner_step_schema =
            if underlying_route.continuation_schema() == entry.continuation_schema() {
                callable_owner_step.step_schema()
            } else {
                underlying_inventory.contract().out_step_schema()
            };
        let owner_step = self.program.step_type(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 continuation schema k{} wrapper projection underlying owner step schema s{}",
                entry.continuation_schema().as_u32(),
                owner_step_schema.as_u32(),
            ))
        })?;

        let outward_cases = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| {
                let wrapper_case = wrapper_step
                    .case(forwarding.input_case_tag())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection 缺少 wrapper step s{} case c{}",
                            entry.continuation_schema().as_u32(),
                            wrapper_step.step_schema().as_u32(),
                            forwarding.input_case_tag().as_u32(),
                        ))
                    })?;
                if wrapper_case.concrete_op_key() != forwarding.input_concrete_op_key() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection 输入 case 漂移：dispatch in c{} op={}，wrapper step s{} case op={}",
                        entry.continuation_schema().as_u32(),
                        forwarding.input_case_tag().as_u32(),
                        forwarding.input_concrete_op_key().instance_key().template.fqn,
                        wrapper_step.step_schema().as_u32(),
                        wrapper_case.concrete_op_key().instance_key().template.fqn,
                    )));
                }
                let owner_case = owner_step
                    .cases()
                    .iter()
                    .find(|case| case.concrete_op_key() == forwarding.input_concrete_op_key())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection 缺少 owner step s{} op={} case",
                            entry.continuation_schema().as_u32(),
                            owner_step.step_schema().as_u32(),
                            forwarding.input_concrete_op_key().instance_key().template.fqn,
                        ))
                    })?;
                Ok(LateLoweredSurfaceResumeWrapperCaseProjection::new(
                    owner_case.case_tag(),
                    owner_case.concrete_op_key().clone(),
                    owner_case.payload_tuple_ty(),
                    forwarding.input_case_tag(),
                    forwarding.input_concrete_op_key().clone(),
                    wrapper_case.payload_tuple_ty(),
                    wrapper_case.continuation_contract(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(LateLoweredSurfaceResumeWrapperProjection::new(
            underlying_route.clone(),
            owner_step.step_schema(),
            wrapper_step.step_schema(),
            self.derive_surface_resume_wrapper_complete_projection(
                entry,
                owner_callable,
                frame_layouts,
                underlying_route,
                owner_step.complete_ty(),
                lowering.dispatch().complete().answer_ty(),
            )?,
            outward_cases,
        )))
    }

    pub(super) fn derive_surface_resume_wrapper_complete_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        underlying_route: &crate::effect_lowered::ir::LateLoweredContinuationRoute,
        owner_answer_ty: TypeId,
        wrapper_answer_ty: TypeId,
    ) -> Result<LateLoweredSurfaceResumeWrapperCompleteProjection, LlvmEmitError> {
        let payload_source = if owner_answer_ty == wrapper_answer_ty {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::owner_complete(owner_answer_ty)
        } else {
            let source = self
                .wrapper_complete_payload_source_from_handle_binder(
                    entry,
                    owner_callable,
                    underlying_route,
                    wrapper_answer_ty,
                )?
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete projection 需要 t{} payload，但缺少 published wrapper payload source",
                        entry.continuation_schema().as_u32(),
                        wrapper_answer_ty.as_u32(),
                    ))
                })?;
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::wrapper_payload(source)
        };
        let projection = LateLoweredSurfaceResumeWrapperCompleteProjection::new(
            owner_answer_ty,
            wrapper_answer_ty,
            payload_source,
        );
        self.validate_surface_resume_wrapper_complete_projection(
            entry,
            owner_callable,
            frame_layouts,
            &projection,
        )?;
        Ok(projection)
    }

    pub(super) fn wrapper_complete_payload_source_from_handle_binder(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        underlying_route: &crate::effect_lowered::ir::LateLoweredContinuationRoute,
        wrapper_answer_ty: TypeId,
    ) -> Result<Option<LateLoweredCompletionPayloadSource>, LlvmEmitError> {
        let (site_id, arm_ordinal, handled_case) = match underlying_route.publication() {
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                site_id,
                arm_ordinal,
                handled_case,
                ..
            } => (site_id, arm_ordinal, handled_case),
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { site_id, .. } => {
                return owner_callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| {
                    let crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                        site_id: boundary_site,
                        kind: BoundarySiteKind::Resume,
                    } = boundary.source()
                    else {
                        return None;
                    };
                    if boundary_site != *site_id {
                        return None;
                    }
                    let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                    else {
                        return None;
                    };
                    (lowering.dispatch().complete().answer_ty() == wrapper_answer_ty).then(|| {
                        LateLoweredCompletionPayloadSource::operand(
                            LateLoweredOperandSource::new_local(
                                lowering.result_local(),
                                wrapper_answer_ty,
                                None,
                            ),
                        )
                    })
                })
                .map(Some)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete projection 找不到 resume boundary site{} 的 completion payload source",
                        entry.continuation_schema().as_u32(),
                        site_id.as_u32(),
                    ))
                });
            }
            _ => return Ok(None),
        };
        let source = owner_callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| {
                let LateLoweredStateTerminator::HandleDispatch {
                    site_id: state_site,
                    contract,
                    ..
                } = state.terminator()
                else {
                    return None;
                };
                if state_site != site_id {
                    return None;
                }
                contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == *arm_ordinal && arm.handled_case() == *handled_case
                    })
                    .map(|arm| arm.completion_payload_source().clone())
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete projection 找不到 handle binder site{} arm#{} case c{} 的 completion payload source",
                    entry.continuation_schema().as_u32(),
                    site_id.as_u32(),
                    arm_ordinal,
                    handled_case.as_u32(),
                ))
            })?;
        if source.source_ty() != wrapper_answer_ty {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload source type t{} 与 wrapper answer t{} 不一致",
                entry.continuation_schema().as_u32(),
                source.source_ty().as_u32(),
                wrapper_answer_ty.as_u32(),
            )));
        }
        Ok(Some(source))
    }

    pub(super) fn validate_surface_resume_wrapper_complete_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        complete: &LateLoweredSurfaceResumeWrapperCompleteProjection,
    ) -> Result<(), LlvmEmitError> {
        if complete.payload_source().source_ty() != complete.wrapper_answer_ty() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload source type t{} 与 wrapper answer t{} 不一致",
                entry.continuation_schema().as_u32(),
                complete.payload_source().source_ty().as_u32(),
                complete.wrapper_answer_ty().as_u32(),
            )));
        }
        match complete.payload_source() {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty } => {
                if *answer_ty != complete.owner_answer_ty()
                    || *answer_ty != complete.wrapper_answer_ty()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 owner-complete wrapper payload 漂移：owner=t{} wrapper=t{} source=t{}",
                        entry.continuation_schema().as_u32(),
                        complete.owner_answer_ty().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                        answer_ty.as_u32(),
                    )));
                }
                self.source_value_layout(*answer_ty)?;
            }
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                if source.source_ty() != complete.wrapper_answer_ty() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper payload source type t{} 与 wrapper answer t{} 不一致",
                        entry.continuation_schema().as_u32(),
                        source.source_ty().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                    )));
                }
                if matches!(source, LateLoweredCompletionPayloadSource::Unit { .. })
                    && !matches!(
                        self.source_types.kind(complete.wrapper_answer_ty()),
                        TypeKind::Value(ValueTypeKind::Unit)
                    )
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 对 non-Unit wrapper answer t{} 发布了 Unit wrapper complete payload source",
                        entry.continuation_schema().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                    )));
                }
                self.source_value_layout(source.source_ty())?;
                if let LateLoweredCompletionPayloadSource::Operand(source) = source
                    && let LateLoweredOperandValueSource::Local(local) = source.value()
                    && let Some(slot_id) =
                        Self::published_frame_slot_for_local(owner_callable.frame_schema(), *local)
                {
                    let slot = owner_callable
                        .frame_schema()
                        .slots()
                        .iter()
                        .find(|slot| slot.slot_id() == slot_id)
                        .expect("published_frame_slot_for_local returned existing slot");
                    if slot.ty() != source.source_ty() {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload home slot fs{} 类型 t{} 与 source type t{} 不一致",
                            entry.continuation_schema().as_u32(),
                            slot_id.as_u32(),
                            slot.ty().as_u32(),
                            source.source_ty().as_u32(),
                        )));
                    }
                    let frame_layout = frame_layouts.get(&owner_callable.step_schema()).ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 缺少 callable `{}` frame layout，无法校验 wrapper complete payload source",
                            owner_callable.root_fqn(),
                        ))
                    })?;
                    if frame_layout.field_index_for_slot(slot_id).is_none() {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload home slot fs{} 在 frame layout 中缺少 field",
                            entry.continuation_schema().as_u32(),
                            slot_id.as_u32(),
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn surface_resume_owner_candidate_mut<'a>(
    candidates: &'a mut Vec<SurfaceResumeOwnerTrampolineCandidate>,
    owner_version_key: &LateLoweredBodyVersionKey,
    owner_continuation_object: ContinuationObjectId,
) -> &'a mut SurfaceResumeOwnerTrampolineCandidate {
    if let Some(index) = candidates.iter().position(|candidate| {
        candidate.owner_version_key == *owner_version_key
            && candidate.owner_continuation_object == owner_continuation_object
    }) {
        return &mut candidates[index];
    }
    candidates.push(SurfaceResumeOwnerTrampolineCandidate::new(
        owner_version_key.clone(),
        owner_continuation_object,
    ));
    let index = candidates.len() - 1;
    &mut candidates[index]
}

pub(super) fn surface_resume_publication_owner_identity(
    publication: &LateLoweredSurfaceResumeDispatchPublication,
) -> Option<(&LateLoweredBodyVersionKey, ContinuationObjectId)> {
    match publication {
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_version_key,
            owner_continuation_object,
            ..
        }
        | LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_version_key,
            owner_continuation_object,
            ..
        } => Some((owner_version_key, *owner_continuation_object)),
        LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { .. }
        | LateLoweredSurfaceResumeDispatchPublication::InternalMethod { .. } => None,
    }
}

fn same_surface_resume_wrapper_projection_shape(
    left: &LateLoweredSurfaceResumeWrapperProjection,
    right: &LateLoweredSurfaceResumeWrapperProjection,
) -> bool {
    left == right
        || (same_surface_resume_projection_owner_identity(left, right)
            && left.owner_step_schema() == right.owner_step_schema()
            && left.wrapper_step_schema() == right.wrapper_step_schema()
            && same_surface_resume_wrapper_complete_shape(
                left.complete(),
                right.complete(),
                matches!(
                    (
                        left.underlying_route().publication(),
                        right.underlying_route().publication()
                    ),
                    (
                        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
                        _
                    ) | (
                        _,
                        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. }
                    )
                ),
            )
            && left.outward_cases() == right.outward_cases())
}

fn same_surface_resume_projection_owner_identity(
    left: &LateLoweredSurfaceResumeWrapperProjection,
    right: &LateLoweredSurfaceResumeWrapperProjection,
) -> bool {
    match (
        surface_resume_publication_owner_identity(left.underlying_route().publication()),
        surface_resume_publication_owner_identity(right.underlying_route().publication()),
    ) {
        (Some((left_owner, left_object)), Some((right_owner, right_object))) => {
            left.underlying_route().continuation_schema()
                == right.underlying_route().continuation_schema()
                && left_owner == right_owner
                && left_object == right_object
        }
        (None, None) => left.underlying_route() == right.underlying_route(),
        _ => false,
    }
}

fn same_surface_resume_wrapper_complete_shape(
    left: &LateLoweredSurfaceResumeWrapperCompleteProjection,
    right: &LateLoweredSurfaceResumeWrapperCompleteProjection,
    ignore_resume_boundary_local_identity: bool,
) -> bool {
    left.owner_answer_ty() == right.owner_answer_ty()
        && left.wrapper_answer_ty() == right.wrapper_answer_ty()
        && same_surface_resume_wrapper_complete_payload_source_shape(
            left.payload_source(),
            right.payload_source(),
            ignore_resume_boundary_local_identity,
        )
}

fn same_surface_resume_wrapper_complete_payload_source_shape(
    left: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    right: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    ignore_resume_boundary_local_identity: bool,
) -> bool {
    match (left, right) {
        (
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete {
                answer_ty: left_ty,
            },
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete {
                answer_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(left),
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(right),
        ) => {
            same_completion_payload_source_ignoring_span(left, right)
                || (ignore_resume_boundary_local_identity
                    && matches!(
                        (left, right),
                        (
                            LateLoweredCompletionPayloadSource::Operand(left_operand),
                            LateLoweredCompletionPayloadSource::Operand(right_operand)
                        ) if left_operand.source_ty() == right_operand.source_ty()
                            && matches!(left_operand.value(), LateLoweredOperandValueSource::Local(_))
                            && matches!(right_operand.value(), LateLoweredOperandValueSource::Local(_))
                    ))
                || (ignore_resume_boundary_local_identity
                    && matches!(
                        (left, right),
                        (
                            LateLoweredCompletionPayloadSource::Unit { complete_ty },
                            LateLoweredCompletionPayloadSource::Operand(operand)
                        )
                            | (
                                LateLoweredCompletionPayloadSource::Operand(operand),
                                LateLoweredCompletionPayloadSource::Unit { complete_ty }
                            ) if *complete_ty == operand.source_ty()
                                && matches!(operand.value(), LateLoweredOperandValueSource::Local(_))
                    ))
        }
        _ => false,
    }
}
