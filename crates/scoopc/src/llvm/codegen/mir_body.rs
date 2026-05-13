//! LLVM lowering for production-visible MIR callable bodies.
//!
//! Production emit lowers callable bodies from `MaterializedMirPassView` through this bridge when
//! their MIR shape is inside the currently supported lowering subset. Explicit pass rewrites enter
//! here strictly; raw materialized bodies outside this subset, declaration-only callables, and
//! non-generic bodies that have not been published into the pass view continue to use their
//! existing HIR-compatible boundary.

use std::collections::HashSet;

use inkwell::values::{BasicMetadataValueEnum, FunctionValue, PointerValue};

use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::*;

#[derive(Clone, Copy)]
pub(super) struct MirLocalSlot<'ctx> {
    pub(super) cg_ty: CgTy,
    pub(super) ptr: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
struct MirBodyCodegenCtx<'m, 'ctx> {
    body: &'m crate::mir::Body,
    mir_types: &'m TypeStore,
    slots: &'m [MirLocalSlot<'ctx>],
}

#[derive(Clone, Copy)]
struct MirInterpolatedSegment<'ctx> {
    ptr: PointerValue<'ctx>,
    len: inkwell::values::IntValue<'ctx>,
}

enum PlainDispatchTarget<'h> {
    Virtual {
        slot: u32,
        sig_fun: &'h hir::FunDecl,
    },
    Interface {
        interface_id: u64,
        slot: u32,
        sig_fun: &'h hir::FunDecl,
    },
}

impl<'h> PlainDispatchTarget<'h> {
    fn sig_fun(&self) -> &'h hir::FunDecl {
        match self {
            Self::Virtual { sig_fun, .. } | Self::Interface { sig_fun, .. } => sig_fun,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Virtual { .. } => "plain virtual",
            Self::Interface { .. } => "plain interface",
        }
    }
}

const MIR_CAPTURE_BOX_FQN: &str = "scoop.__CaptureBox";

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn mir_direct_call_base_fqn(fqn: &str) -> &str {
    let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn decompose_target_triple(triple: &str) -> (String, String, String, String) {
    let mut parts = triple.split('-');
    let arch = parts.next().unwrap_or("").to_string();
    let vendor = parts.next().unwrap_or("").to_string();
    let os = parts.next().unwrap_or("").to_string();
    let env = parts.next().unwrap_or("").to_string();
    (arch, vendor, os, env)
}

#[derive(Clone, Copy)]
struct MirMemberPlace<'ctx> {
    ptr: PointerValue<'ctx>,
    field_cg: CgTy,
    writable: bool,
    packed_alignment: Option<u32>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn has_materialized_instances_for_template(&self, fqn: &str) -> bool {
        self.materialized_pass_view().is_some_and(|pass_view| {
            pass_view.instances().any(|family| {
                family.key().template.fqn == fqn
                    && (!family.key().type_args.is_empty() || !family.key().eff_args.is_empty())
            })
        })
    }

    pub(in crate::llvm::codegen) fn materialized_mir_callable(
        &self,
        fqn: &str,
    ) -> Option<(&TypeStore, &crate::mir::FunDecl)> {
        let pass_view = self.materialized_pass_view()?;
        let fqn_is_generic_template_with_instances = pass_view.instances().any(|family| {
            family.key().template.fqn == fqn
                && (!family.key().type_args.is_empty() || !family.key().eff_args.is_empty())
        });
        if fqn_is_generic_template_with_instances {
            return None;
        }
        let mir_fun = pass_view
            .callable(fqn)
            .or_else(|| {
                pass_view
                    .materialized()
                    .file
                    .items
                    .iter()
                    .find_map(|item| match item {
                        crate::mir::Item::Fun(fun) if fun.fqn == fqn && fun.body.is_some() => {
                            Some(fun)
                        }
                        _ => None,
                    })
            })
            .or_else(|| {
                pass_view
                    .materialized()
                    .caller_side_pass_candidate_bodies()
                    .iter()
                    .find(|fun| fun.fqn == fqn && fun.body.is_some())
            })?;
        Some((&pass_view.materialized().types, mir_fun))
    }

    fn materialized_mir_closure_body_symbol(
        &self,
        callable_fqn: &str,
        at: crate::span::Span,
    ) -> Result<String, LlvmEmitError> {
        Ok(private_closure_body_fn_name(
            &self.stable_closure_key_for_materialized_callable(callable_fqn, at)?,
        ))
    }

    fn inferred_materialized_direct_call_fqn(
        &self,
        template_fqn: &str,
        args: &[crate::mir::CallArg],
        result_source_ty: TypeId,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Option<String> {
        let pass_view = self.materialized_pass_view()?;
        let materialized_types = &pass_view.materialized().types;
        let arg_cg_tys = args
            .iter()
            .map(|arg| {
                let source_ty = self.mir_operand_type_id(body, &arg.value)?;
                self.equivalent_codegen_type_id(mir_types, source_ty)
                    .and_then(|ty| self.cg_ty_of(ty))
                    .or_else(|| self.cg_ty_of_mir_type(mir_types, source_ty))
                    .or_else(|| self.cg_ty_of(source_ty))
            })
            .collect::<Option<Vec<_>>>()?;
        let result_cg = self
            .equivalent_codegen_type_id(mir_types, result_source_ty)
            .and_then(|ty| self.cg_ty_of(ty))
            .or_else(|| self.cg_ty_of_mir_type(mir_types, result_source_ty))
            .or_else(|| self.cg_ty_of(result_source_ty))?;
        let mut matched: Option<String> = None;
        for family in pass_view.instances() {
            if family.key().template.fqn != template_fqn {
                continue;
            }
            let Some(fun) = family.root_body() else {
                continue;
            };
            if fun.params.len() != arg_cg_tys.len() {
                continue;
            }
            let params_match =
                fun.params
                    .iter()
                    .zip(arg_cg_tys.iter().copied())
                    .all(|(param, arg_cg)| {
                        self.cg_ty_of_mir_type(materialized_types, param.ty)
                            .is_some_and(|param_cg| param_cg == arg_cg)
                    });
            if !params_match {
                continue;
            }
            if self.cg_ty_of_mir_type(materialized_types, fun.return_ty) != Some(result_cg) {
                continue;
            }
            let candidate = family.root_fqn().to_string();
            if let Some(found) = matched.as_ref() {
                if found != &candidate {
                    return None;
                }
                continue;
            }
            matched = Some(candidate);
        }
        matched
    }

    fn ensure_materialized_mir_closure_callable_defined(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let (mir_types, mir_fun) =
            self.materialized_mir_callable(fn_ptr)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure function",
                    at: span.into(),
                })?;
        if !mir_fun.name.starts_with("$lambda") {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure function",
                at: span.into(),
            });
        }
        let body_symbol = self.materialized_mir_closure_body_symbol(fn_ptr, mir_fun.span)?;
        if let Some(existing) = self.module.get_function(&body_symbol)
            && existing.count_basic_blocks() > 0
        {
            return Ok(existing);
        }

        let saved_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let mut child = self.fresh_child_codegen();
        child.current_source_id = child.materialized_mir_callable_source_id(fn_ptr, span)?;
        let llvm_fun = child.declare_materialized_mir_closure_fun(span, mir_fun, mir_types)?;
        if llvm_fun.count_basic_blocks() == 0 {
            child.codegen_materialized_mir_closure_fun(mir_fun, mir_types, llvm_fun)?;
        }
        self.builder.position_at_end(saved_block);
        Ok(llvm_fun)
    }

    pub(super) fn hir_fun_for_callable_fqn(&self, fqn: &str) -> Option<&'a hir::FunDecl> {
        if let Some(hir_fun) = self.fun_index.get(fqn).copied() {
            return Some(hir_fun);
        }
        if let Some(pass_view) = self.materialized_pass_view()
            && let Some(owner) = pass_view.owner_of_callable(fqn)
            && let Some(hir_fun) = self.fun_index.values().copied().find(|fun| {
                fun.fqn == owner.template.fqn
                    && fun.source_path == owner.template.source_path
                    && fun.span == owner.template.decl_span
            })
        {
            return Some(hir_fun);
        }
        let base = mir_direct_call_base_fqn(fqn);
        if base != fqn {
            if let Some(hir_fun) = self.fun_index.get(base).copied() {
                return Some(hir_fun);
            }
            if let Some(pass_view) = self.materialized_pass_view()
                && let Some(owner) = pass_view.owner_of_callable(base)
                && let Some(hir_fun) = self.fun_index.values().copied().find(|fun| {
                    fun.fqn == owner.template.fqn
                        && fun.source_path == owner.template.source_path
                        && fun.span == owner.template.decl_span
                })
            {
                return Some(hir_fun);
            }
        }
        None
    }

    pub(super) fn materialized_mir_callable_source_id(
        &self,
        fqn: &str,
        span: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        let mut owner_fqn = fqn;
        loop {
            if let Some(hir_fun) = self.hir_fun_for_callable_fqn(owner_fqn) {
                return self.source_id_for_path(hir_fun.source_path.as_path(), span);
            }
            let Some((parent, _)) = owner_fqn.rsplit_once(".$lambda") else {
                break;
            };
            owner_fqn = parent;
        }
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR callable source path",
            at: span.into(),
        })
    }

    pub(super) fn declare_materialized_mir_closure_fun(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let param_tys = mir_fun
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        self.declare_materialized_mir_closure_fun_with_signature(
            span,
            mir_fun,
            &param_tys,
            mir_fun.return_ty,
            mir_types,
        )
    }

    pub(super) fn declare_materialized_mir_closure_fun_with_signature(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        param_tys: &[TypeId],
        return_ty: TypeId,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let body_symbol = self.materialized_mir_closure_body_symbol(mir_fun.fqn.as_str(), span)?;
        if let Some(existing) = self.module.get_function(&body_symbol) {
            return Ok(existing);
        }
        if param_tys.len() != mir_fun.params.len() {
            return Err(frontend_error(format!(
                "refactor materialized closure `{}` 的 plain ABI 参数数量({}) 与 MIR 参数数量({}) 不一致",
                mir_fun.fqn,
                param_tys.len(),
                mir_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(mir_types, return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure return type",
                at: mir_fun.span.into(),
            },
        )?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        // 这里发布的是 plain callable ABI 的 closure body symbol；effect-step callable surface
        // 由 stage-owned direct/dynamic entry shell 单独承载，不应再为 plain entry 混入 hidden ABI。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(mir_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        for (param, param_ty) in mir_fun.params.iter().skip(1).zip(param_tys.iter().skip(1)) {
            let param_ty = self
                .equivalent_codegen_type_id(mir_types, *param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure param type",
                    at: param.span.into(),
                })?;
            llvm_param_tys.push(
                self.ordinary_param_abi(param.span, param_ty)?
                    .llvm_param_ty(),
            );
        }

        let fn_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(mir_fun.span, other)?
                .fn_type(&llvm_param_tys, false),
        };
        let llvm_fun =
            self.declare_compiler_private_helper_function(&body_symbol, fn_ty, Linkage::Internal);
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    pub(super) fn declare_materialized_mir_plain_fun_with_symbol(
        &mut self,
        llvm_name: &str,
        surface: LlvmFunctionDeclarationSurface,
        mir_fun: &crate::mir::FunDecl,
        param_tys: &[TypeId],
        return_ty: TypeId,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let llvm_name = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                self.exported_abi_symbol_for_materialized_fun(mir_fun, mir_types)?
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
            | LlvmFunctionDeclarationSurface::CompilerPrivateHelper => llvm_name.to_string(),
        };
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }
        if param_tys.len() != mir_fun.params.len() {
            return Err(frontend_error(format!(
                "refactor plain materialized callable `{}` 的 plain ABI 参数数量({}) 与 MIR 参数数量({}) 不一致",
                mir_fun.fqn,
                param_tys.len(),
                mir_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(mir_types, return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR plain return type",
                at: mir_fun.span.into(),
            },
        )?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(mir_fun.span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(mir_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for (param, param_ty) in mir_fun.params.iter().zip(param_tys.iter().copied()) {
            let param_ty = self.equivalent_codegen_type_id(mir_types, param_ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR plain param type",
                    at: param.span.into(),
                },
            )?;
            llvm_param_tys.push(
                self.ordinary_param_abi(param.span, param_ty)?
                    .llvm_param_ty(),
            );
        }

        let fn_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(mir_fun.span, other)?
                .fn_type(&llvm_param_tys, false),
        };
        let llvm_fun = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                self.declare_exported_abi_function(&llvm_name, fn_ty)
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport => {
                self.declare_runtime_or_native_import_function(&llvm_name, fn_ty)
            }
            LlvmFunctionDeclarationSurface::CompilerPrivateHelper => {
                self.declare_compiler_private_helper_function(&llvm_name, fn_ty, Linkage::Internal)
            }
        };
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    fn codegen_materialized_mir_closure_fun(
        mut self,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = mir_fun.body.as_ref() else {
            return Ok(());
        };
        body.validate_cfg()
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR cfg",
                at: mir_fun.span.into(),
            })?;
        self.function_cx.current_callable_fqn = Some(mir_fun.fqn.clone());

        let declared_return_cg = self.cg_ty_of_mir_type(mir_types, mir_fun.return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure return type",
                at: mir_fun.span.into(),
            },
        )?;
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(mir_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm function sret param",
                        at: mir_fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.clear_explicit_effect_hidden_abi_slots();

        let (return_bb, return_alloca) =
            self.setup_function_return_context(mir_fun.span, llvm_fun, declared_return_cg)?;
        let mut local_slots = self.create_mir_local_slots(body, mir_types)?;
        self.bind_mir_closure_params(
            mir_fun,
            mir_types,
            llvm_fun,
            u32::from(uses_hidden_sret),
            &mut local_slots,
        )?;
        let used_locals = collect_mir_local_uses(body);
        let llvm_blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                self.context
                    .append_basic_block(llvm_fun, &format!("mir.bb{idx}"))
            })
            .collect::<Vec<_>>();
        let start_bb = llvm_blocks
            .get(body.start.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR start block",
                at: mir_fun.span.into(),
            })?;
        self.builder.build_unconditional_branch(start_bb)?;

        for (idx, block) in body.blocks.iter().enumerate() {
            self.builder.position_at_end(llvm_blocks[idx]);
            for stmt in &block.stmts {
                self.codegen_mir_statement(stmt, body, mir_types, &local_slots, &used_locals)?;
            }
            self.codegen_mir_terminator(
                &block.terminator,
                body,
                mir_types,
                &local_slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(
            mir_fun.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    pub(super) fn bind_mir_closure_params(
        &mut self,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let env_param = mir_fun
            .params
            .first()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env param",
                at: mir_fun.span.into(),
            })?;
        if env_param.name != "$env" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env param",
                at: env_param.span.into(),
            });
        }
        let env_slot = slots
            .get(env_param.local.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env local",
                at: env_param.span.into(),
            })?;
        let env_init = self.codegen_mir_closure_env_param(
            env_param.span,
            &mir_fun.fqn,
            llvm_fun,
            param_offset,
            env_slot.cg_ty,
        )?;
        let _ = self.store_local_value(env_param.span, env_slot.ptr, env_slot.cg_ty, env_init)?;

        for (idx, param) in mir_fun.params.iter().enumerate().skip(1) {
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let param_ty = self.equivalent_codegen_type_id(mir_types, param.ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param type",
                    at: param.span.into(),
                },
            )?;
            let abi = self.ordinary_param_abi(param.span, param_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm param",
                        at: param.span.into(),
                    })?
                    .into_pointer_value();
                let loaded =
                    self.builder
                        .build_load(pointee_ty, param_ptr, "pass_mir_param_load")?;
                self.cg_value_from_loaded(param.span, slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing pass MIR llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    fn codegen_mir_closure_env_param(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        env_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match env_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Tuple(tuple_ty) => {
                let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys(env_cg).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env shape",
                        at: span.into(),
                    },
                )?;
                let env_arg = llvm_fun
                    .get_nth_param(param_index)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR closure env param",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let closure_key =
                    self.stable_closure_key_for_materialized_callable(fn_ptr, span)?;
                let env_ty =
                    self.mir_closure_env_object_type(span, &closure_key, &capture_field_cgs)?;
                let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
                let env_ptr = self.builder.build_pointer_cast(
                    env_arg,
                    env_ptr_ty,
                    "pass_mir_closure_env_ptr",
                )?;
                let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
                let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
                for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                    let field_gep = self.builder.build_struct_gep(
                        env_ty,
                        env_ptr,
                        (idx + 1) as u32,
                        "pass_mir_closure_env_field_gep",
                    )?;
                    let field_raw = self.builder.build_load(
                        self.llvm_basic_type_of(span, *field_cg)?,
                        field_gep,
                        "pass_mir_closure_env_field_load",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        field_raw,
                        idx as u32,
                        "pass_mir_closure_env_tuple_insert",
                    )?;
                }
                Ok(CgValue {
                    ty: env_cg,
                    value: Some(agg.as_basic_value_enum()),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env type",
                at: span.into(),
            }),
        }
    }

    pub(super) fn create_mir_local_slots(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Result<Vec<MirLocalSlot<'ctx>>, LlvmEmitError> {
        body.locals
            .iter()
            .enumerate()
            .map(|(idx, local)| {
                let local_id = crate::mir::LocalId::from_raw(idx as u32);
                let cg_ty = self.mir_local_storage_cg_ty(body, mir_types, local_id, local)?;
                let ptr = self.create_entry_alloca(
                    local.span,
                    local.name.as_deref().unwrap_or("mir_local"),
                    cg_ty,
                )?;
                Ok(MirLocalSlot { cg_ty, ptr })
            })
            .collect()
    }

    fn mir_local_storage_cg_ty(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local_id: crate::mir::LocalId,
        local: &crate::mir::LocalDecl,
    ) -> Result<CgTy, LlvmEmitError> {
        let local_cg = self.cg_ty_of_mir_type(mir_types, local.ty);
        let mut member_field_cg = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let crate::mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local_id {
                    continue;
                }
                let crate::mir::Rvalue::MemberAccess {
                    receiver, member, ..
                } = value
                else {
                    continue;
                };
                if !matches!(
                    member.resolved,
                    Some(crate::mir::MemberTarget::Value { .. })
                ) {
                    continue;
                }
                let Ok(field_cg) =
                    self.mir_member_field_cg_ty(stmt.span, body, mir_types, receiver, member)
                else {
                    continue;
                };
                if let Some(previous) = member_field_cg {
                    if !self.cg_ty_layout_equivalent(previous, field_cg) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR local member field type drift",
                            at: stmt.span.into(),
                        });
                    }
                } else {
                    member_field_cg = Some(field_cg);
                }
            }
        }
        if let Some(field_cg) = member_field_cg
            && (matches!(local.source, crate::mir::LocalSourceKind::CompilerTemporary)
                || local_cg.is_some_and(|local_cg| {
                    self.mir_type_contains_param(mir_types, local.ty)
                        || self.cg_ty_layout_equivalent(local_cg, field_cg)
                }))
        {
            return Ok(field_cg);
        }
        if let Some(assigned_cg) = self.mir_local_assignment_cg_ty(body, mir_types, local_id)
            && matches!(local.source, crate::mir::LocalSourceKind::CompilerTemporary)
            && (local_cg.is_none()
                || matches!(local_cg, Some(CgTy::Ref))
                || matches!(assigned_cg, CgTy::Enum(_))
                || self.mir_type_contains_param(mir_types, local.ty)
                || local_cg
                    .is_some_and(|local_cg| self.cg_ty_layout_equivalent(local_cg, assigned_cg)))
        {
            return Ok(assigned_cg);
        }
        local_cg.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR local type",
            at: local.span.into(),
        })
    }

    fn mir_local_assignment_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local_id: crate::mir::LocalId,
    ) -> Option<CgTy> {
        let mut inferred = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let crate::mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local_id {
                    continue;
                }
                let candidate = match value {
                    crate::mir::Rvalue::Use(operand) => {
                        self.mir_operand_cg_ty(body, mir_types, operand)?
                    }
                    crate::mir::Rvalue::Transport { value, transport } => {
                        self.mir_transport_result_cg_ty(body, mir_types, value, transport)?
                    }
                    crate::mir::Rvalue::Unary { operand, .. } => {
                        self.mir_operand_cg_ty(body, mir_types, operand)?
                    }
                    crate::mir::Rvalue::Binary { lhs, .. } => {
                        self.mir_operand_cg_ty(body, mir_types, lhs)?
                    }
                    crate::mir::Rvalue::TypeCheck { .. } => CgTy::Bool,
                    crate::mir::Rvalue::Cast { target_ty, .. } => {
                        self.cg_ty_of_mir_type(mir_types, *target_ty)?
                    }
                    crate::mir::Rvalue::Call { kind, .. } => {
                        self.mir_call_result_cg_ty(body, mir_types, kind)?
                    }
                    crate::mir::Rvalue::MemberAccess { member, .. } => {
                        self.mir_member_resolved_static_value_cg_ty(member)?
                    }
                    crate::mir::Rvalue::TupleGet { tuple, index } => {
                        self.mir_tuple_get_result_cg_ty(body, mir_types, tuple, *index)?
                    }
                    _ => continue,
                };
                match inferred {
                    Some(existing) if !self.cg_ty_layout_equivalent(existing, candidate) => {
                        return None;
                    }
                    Some(_) => {}
                    None => inferred = Some(candidate),
                }
            }
        }
        inferred
    }

    fn mir_call_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        kind: &crate::mir::CallKind,
    ) -> Option<CgTy> {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                if self.class_inits.contains_key(callee_fqn) {
                    return Some(CgTy::Ref);
                }
                if mir_direct_call_base_fqn(callee_fqn) == "scoop.core.size" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: true,
                    }));
                }
                let fun = self.hir_fun_for_callable_fqn(callee_fqn)?;
                self.cg_ty_of(fun.return_ty)
            }
            crate::mir::CallKind::Closure { callee, .. }
            | crate::mir::CallKind::FunValue { callee } => {
                let fun_ty = self
                    .mir_operand_funptr_function_type(body, mir_types, callee)
                    .or_else(|| self.mir_operand_function_type(body, mir_types, callee))?;
                self.cg_ty_of_mir_type(mir_types, fun_ty.return_ty)
            }
            crate::mir::CallKind::Resume { resume, .. } => {
                self.cg_ty_of_mir_type(mir_types, resume.answer_ty)
            }
            crate::mir::CallKind::Virtual { .. } | crate::mir::CallKind::Interface { .. } => None,
        }
    }

    fn cg_ty_layout_equivalent(&self, lhs: CgTy, rhs: CgTy) -> bool {
        if lhs == rhs {
            return true;
        }
        match (lhs, rhs) {
            (CgTy::Tuple(lhs), CgTy::Tuple(rhs))
            | (CgTy::Struct(lhs), CgTy::Struct(rhs))
            | (CgTy::Enum(lhs), CgTy::Enum(rhs)) => {
                self.types.display(lhs).to_string() == self.types.display(rhs).to_string()
            }
            _ => false,
        }
    }

    fn describe_cg_ty(&self, cg_ty: CgTy) -> String {
        match cg_ty {
            CgTy::Tuple(ty) | CgTy::Struct(ty) | CgTy::Enum(ty) => {
                format!("{cg_ty:?} {}", self.types.display(ty))
            }
            _ => format!("{cg_ty:?}"),
        }
    }

    fn mir_type_contains_param(&self, types: &TypeStore, ty: TypeId) -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match types.kind(id) {
                TypeKind::Param(_) => return true,
                TypeKind::StarProjection(star) => stack.push(star.read_ty),
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    stack.extend(nominal.args.iter().copied());
                    if let Some(eff) = &nominal.eff {
                        stack.extend(eff.terms.iter().copied());
                    }
                }
                TypeKind::Ref(RefTypeKind::Function(fun)) => {
                    if let Some(receiver) = fun.receiver {
                        stack.push(receiver);
                    }
                    stack.extend(fun.params.iter().copied());
                    stack.push(fun.return_ty);
                    stack.extend(fun.effects.terms.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Union(union)) => {
                    stack.extend(union.variants.iter().copied());
                }
                TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                    stack.extend(elements.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                | TypeKind::Value(
                    ValueTypeKind::Unit
                    | ValueTypeKind::Nothing
                    | ValueTypeKind::Bool
                    | ValueTypeKind::Char
                    | ValueTypeKind::Float64
                    | ValueTypeKind::Float32
                    | ValueTypeKind::Int
                    | ValueTypeKind::UInt
                    | ValueTypeKind::IntN(_)
                    | ValueTypeKind::UIntN(_),
                ) => {}
            }
        }
        false
    }

    pub(super) fn cg_ty_of_mir_type(&self, mir_types: &TypeStore, ty: TypeId) -> Option<CgTy> {
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::StarProjection(star) => self.cg_ty_of_mir_type(mir_types, star.read_ty),
            TypeKind::Value(ValueTypeKind::Nothing) => Some(CgTy::Never),
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Char) => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Float64) => Some(CgTy::Float64),
            TypeKind::Value(ValueTypeKind::Float32) => Some(CgTy::Float32),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Option(_)) => self
                .equivalent_codegen_type_id(mir_types, ty)
                .and_then(|codegen_ty| self.cg_ty_of(codegen_ty))
                .or_else(|| self.cg_ty_of(ty)),
            TypeKind::Value(ValueTypeKind::Tuple(_)) => self
                .equivalent_codegen_type_id(mir_types, ty)
                .and_then(|codegen_ty| self.cg_ty_of(codegen_ty))
                .or_else(|| self.cg_ty_of(ty)),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => self
                .builtin_nominal_cg_ty(&nominal.fqn)
                .or_else(|| {
                    self.equivalent_codegen_type_id(mir_types, ty)
                        .and_then(|codegen_ty| self.cg_ty_of(codegen_ty))
                })
                .or_else(|| self.cg_ty_of(ty)),
            TypeKind::Param(_) => None,
        }
    }

    pub(super) fn equivalent_codegen_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = source_types.display(source_ty).to_string();
        self.types
            .iter_ids()
            .find(|&candidate| self.types.display(candidate).to_string() == source_display)
            .or_else(|| {
                let display_matches = |source_arg: TypeId, candidate_arg: TypeId| {
                    source_types.display(source_arg).to_string()
                        == self.types.display(candidate_arg).to_string()
                };
                match source_types.kind(source_ty) {
                    TypeKind::Ref(RefTypeKind::Nominal(source_nominal)) => {
                        self.types.iter_ids().find(|candidate| {
                            matches!(
                                self.types.kind(*candidate),
                                TypeKind::Ref(RefTypeKind::Nominal(candidate_nominal))
                                    if candidate_nominal.fqn == source_nominal.fqn
                                        && candidate_nominal.args.len() == source_nominal.args.len()
                                        && source_nominal
                                            .args
                                            .iter()
                                            .zip(candidate_nominal.args.iter())
                                            .all(|(source_arg, candidate_arg)| {
                                                display_matches(*source_arg, *candidate_arg)
                                            })
                            )
                        })
                    }
                    TypeKind::Value(ValueTypeKind::Nominal(source_nominal)) => {
                        self.types.iter_ids().find(|candidate| {
                            matches!(
                                self.types.kind(*candidate),
                                TypeKind::Value(ValueTypeKind::Nominal(candidate_nominal))
                                    if candidate_nominal.fqn == source_nominal.fqn
                                        && candidate_nominal.args.len() == source_nominal.args.len()
                                        && source_nominal
                                            .args
                                            .iter()
                                            .zip(candidate_nominal.args.iter())
                                            .all(|(source_arg, candidate_arg)| {
                                                display_matches(*source_arg, *candidate_arg)
                                            })
                            )
                        })
                    }
                    TypeKind::Value(ValueTypeKind::Tuple(source_elems)) => {
                        self.types.iter_ids().find(|candidate| {
                            matches!(
                                self.types.kind(*candidate),
                                TypeKind::Value(ValueTypeKind::Tuple(candidate_elems))
                                    if candidate_elems.len() == source_elems.len()
                                        && source_elems
                                            .iter()
                                            .zip(candidate_elems.iter())
                                            .all(|(source_arg, candidate_arg)| {
                                                display_matches(*source_arg, *candidate_arg)
                                            })
                            )
                        })
                    }
                    TypeKind::Value(ValueTypeKind::Option(source_inner)) => {
                        self.types.iter_ids().find(|candidate| {
                            matches!(
                                self.types.kind(*candidate),
                                TypeKind::Value(ValueTypeKind::Option(candidate_inner))
                                    if display_matches(*source_inner, *candidate_inner)
                            )
                        })
                    }
                    _ => None,
                }
            })
    }

    fn runtime_type_descriptor_is_codegen_supported(
        &self,
        mir_types: &TypeStore,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
    ) -> bool {
        if !matches!(
            metadata.descriptor.kind,
            crate::mir::RuntimeTypeDescriptorKind::Any
                | crate::mir::RuntimeTypeDescriptorKind::String
                | crate::mir::RuntimeTypeDescriptorKind::Nominal { .. }
        ) {
            return false;
        }
        self.equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .and_then(|target_ty| self.cg_ty_of(target_ty))
            .is_some_and(|target_cg| matches!(target_cg, CgTy::Ref | CgTy::String))
    }

    fn runtime_pattern_type_descriptor_is_codegen_supported(
        &self,
        mir_types: &TypeStore,
        metadata: &crate::mir::RuntimePatternTypeTestMetadata,
    ) -> bool {
        if !matches!(
            metadata.descriptor.kind,
            crate::mir::RuntimeTypeDescriptorKind::Any
                | crate::mir::RuntimeTypeDescriptorKind::String
                | crate::mir::RuntimeTypeDescriptorKind::Nominal { .. }
        ) {
            return false;
        }
        self.equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .and_then(|target_ty| self.cg_ty_of(target_ty))
            .is_some_and(|target_cg| matches!(target_cg, CgTy::Ref | CgTy::String))
    }

    fn mir_tuple_get_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
    ) -> Option<CgTy> {
        let tuple_ty = self.mir_operand_type_id(body, tuple)?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            return None;
        };
        let element_ty = *elements.get(index)?;
        self.cg_ty_of_mir_type(mir_types, element_ty)
    }

    fn mir_member_resolved_top_level_value_fqn<'m>(
        &self,
        member: &'m crate::mir::MemberAccessMetadata,
    ) -> Option<&'m str> {
        let Some(crate::mir::MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
            return None;
        };
        (self.object_inits.contains_key(fqn)
            || self.lookup_object_property_by_fqn(fqn).is_some()
            || self.top_level_consts.contains_key(fqn)
            || self.top_level_immutable_values.contains_key(fqn)
            || self.top_level_vars.contains_key(fqn)
            || self.has_extern_global_contract(fqn)
            || self.mir_member_resolved_enum_unit_variant_fqn(fqn))
        .then_some(fqn.as_str())
    }

    fn mir_member_resolved_static_value_cg_ty(
        &self,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Option<CgTy> {
        let crate::mir::MemberTarget::Value { fqn } = member.resolved.as_ref()? else {
            return None;
        };
        if self.object_inits.contains_key(fqn) {
            return Some(CgTy::Ref);
        }
        if let Some((_object, prop)) = self.lookup_object_property_by_fqn(fqn) {
            return self.cg_ty_of(prop.ty);
        }
        if let Some(value) = self.top_level_consts.get(fqn) {
            return self.cg_ty_of(value.ty);
        }
        if let Some(value) = self.top_level_immutable_values.get(fqn) {
            return self.cg_ty_of(value.ty);
        }
        if let Some(value) = self.top_level_vars.get(fqn) {
            return self.cg_ty_of(value.ty);
        }
        if let Some(value) = self.materialized_extern_global_root(fqn) {
            return self.cg_ty_of(value.ty);
        }
        if let Some(value) = self.extern_globals.get(fqn) {
            return self.cg_ty_of(value.ty);
        }
        let (owner_fqn, variant_name) = fqn.rsplit_once('.')?;
        let layout = self.enum_layouts.get(owner_fqn)?;
        layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name && variant.fields.is_empty())?;
        self.types
            .iter_ids()
            .find(|id| {
                matches!(
                    self.types.kind(*id),
                    TypeKind::Value(ValueTypeKind::Nominal(nominal))
                        if nominal.fqn == owner_fqn && nominal.args.is_empty() && nominal.eff.is_none()
                )
            })
            .map(CgTy::Enum)
    }

    fn mir_member_resolved_enum_unit_variant_fqn(&self, fqn: &str) -> bool {
        let Some((owner_fqn, variant_name)) = fqn.rsplit_once('.') else {
            return false;
        };
        self.enum_layouts
            .get(owner_fqn)
            .and_then(|layout| {
                layout
                    .variants
                    .iter()
                    .find(|variant| variant.name == variant_name)
            })
            .is_some_and(|variant| variant.fields.is_empty())
    }

    fn mir_member_field_cg_ty(
        &mut self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Result<CgTy, LlvmEmitError> {
        let field_fqn = mir_member_value_fqn_for_codegen(span, member)?;
        let receiver_type_id =
            self.mir_member_receiver_codegen_type_id(span, body, mir_types, receiver, member)?;
        if let Some((_class, _field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
        {
            return Ok(field_cg);
        }

        let receiver_cg =
            self.cg_ty_of(receiver_type_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR member receiver type",
                    at: span.into(),
                })?;
        let CgTy::Struct(struct_ty) = receiver_cg else {
            return Err(frontend_error(format!(
                "pass MIR member field target `{field_fqn}` receiver_ty=t{} receiver_cg={}",
                receiver_type_id.as_u32(),
                self.describe_cg_ty(receiver_cg),
            )));
        };
        let (_field_idx, field_cg) = self.lookup_struct_field(struct_ty, field_fqn, span)?;
        Ok(field_cg)
    }

    fn mir_transport_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        value: &crate::mir::Operand,
        transport: &crate::mir::ValueTransportMetadata,
    ) -> Option<CgTy> {
        self.mir_operand_cg_ty(body, mir_types, value)?;
        let boxing = transport.boxing.as_ref()?;
        if !matches!(
            boxing.reason,
            crate::mir::MirBoxingReason::AnyErasure | crate::mir::MirBoxingReason::RefErasure
        ) || boxing.source_ty != transport.source_ty
        {
            return None;
        }
        if matches!(
            mir_types.kind(transport.source_ty),
            TypeKind::Value(ValueTypeKind::Nothing)
        ) {
            return Some(CgTy::Ref);
        }
        let source_ty = self.equivalent_codegen_type_id(mir_types, transport.source_ty)?;
        let source_cg = self.cg_ty_of(source_ty)?;
        match source_cg {
            CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Unit
            | CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Enum(_) => Some(CgTy::Ref),
            CgTy::Float64 | CgTy::Float32 | CgTy::Never => None,
        }
    }

    fn mir_enum_payload_schema_matches(
        &self,
        mir_types: &TypeStore,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
        args: &[crate::mir::CallArg],
        payload: &crate::mir::AggregateTransportMetadata,
    ) -> bool {
        if payload.kind != crate::mir::AggregateTransportKind::EnumPayload {
            return false;
        }
        let Some(payload_enum_ty) =
            self.equivalent_codegen_type_id(mir_types, payload.aggregate_ty)
        else {
            return false;
        };
        if payload_enum_ty != enum_ty
            || payload.fields.len() != args.len()
            || variant.fields.len() != args.len()
        {
            return false;
        }

        for (idx, ((field, arg), field_cg)) in payload
            .fields
            .iter()
            .zip(args)
            .zip(variant.fields.iter())
            .enumerate()
        {
            if field.index != idx || field.name.as_deref() != arg.name.as_deref() {
                return false;
            }
            if field.transport.source_ty != field.ty
                || field
                    .transport
                    .boxing
                    .as_ref()
                    .is_some_and(|boxing| boxing.source_ty != field.ty)
            {
                return false;
            }
            let Some(field_ty) = self.equivalent_codegen_type_id(mir_types, field.ty) else {
                return false;
            };
            let Some(expected_cg) = self.cg_ty_of(field_ty) else {
                return false;
            };
            if expected_cg != *field_cg {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    fn mir_member_receiver_codegen_type_id(
        &self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Result<TypeId, LlvmEmitError> {
        let receiver_source_ty = match receiver {
            crate::mir::Operand::Local(local) => body
                .locals
                .get(local.as_u32() as usize)
                .map(|local| local.ty)
                .unwrap_or(member.receiver_ty),
            crate::mir::Operand::Const(_) => member.receiver_ty,
        };
        self.equivalent_codegen_type_id(mir_types, receiver_source_ty)
            .or_else(|| self.equivalent_codegen_type_id(mir_types, member.receiver_ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member receiver type",
                at: span.into(),
            })
    }

    fn equivalent_runtime_ref_codegen_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = source_types.display(source_ty).to_string();
        self.types.iter_ids().find(|&candidate| {
            self.types.display(candidate).to_string() == source_display
                && matches!(self.types.kind(candidate), TypeKind::Ref(_))
        })
    }

    fn mir_class_ctor_layout_key(
        &self,
        class_fqn: &str,
        mir_types: &TypeStore,
        target_source_ty: Option<TypeId>,
    ) -> String {
        let Some(target_source_ty) = target_source_ty else {
            return class_fqn.to_string();
        };
        let Some(codegen_ty) = self.equivalent_codegen_type_id(mir_types, target_source_ty) else {
            return class_fqn.to_string();
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(codegen_ty) else {
            return class_fqn.to_string();
        };
        if nominal.fqn != class_fqn {
            return class_fqn.to_string();
        }
        self.nominal_layout_key(nominal)
    }

    pub(super) fn equivalent_codegen_effect_row(
        &self,
        source_types: &TypeStore,
        source_row: &crate::ty::EffectRow,
    ) -> Option<crate::ty::EffectRow> {
        let mut terms = Vec::with_capacity(source_row.terms.len());
        for term in &source_row.terms {
            terms.push(self.equivalent_codegen_type_id(source_types, *term)?);
        }
        Some(crate::ty::EffectRow::new(terms))
    }

    pub(super) fn equivalent_codegen_function_type(
        &self,
        source_types: &TypeStore,
        fun_ty: &crate::ty::FunctionType,
    ) -> Option<crate::ty::FunctionType> {
        let receiver = match fun_ty.receiver {
            Some(ty) => Some(self.equivalent_codegen_type_id(source_types, ty)?),
            None => None,
        };
        let mut params = Vec::with_capacity(fun_ty.params.len());
        for param in &fun_ty.params {
            params.push(self.equivalent_codegen_type_id(source_types, *param)?);
        }
        Some(crate::ty::FunctionType {
            receiver,
            params,
            return_ty: self.equivalent_codegen_type_id(source_types, fun_ty.return_ty)?,
            effects: self.equivalent_codegen_effect_row(source_types, &fun_ty.effects)?,
            effects_closed: fun_ty.effects_closed,
        })
    }

    pub(super) fn mir_local_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local: crate::mir::LocalId,
    ) -> Option<CgTy> {
        let local = body.locals.get(local.as_u32() as usize)?;
        self.cg_ty_of_mir_type(mir_types, local.ty)
    }

    pub(super) fn mir_operand_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<CgTy> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_cg_ty(body, mir_types, *local),
            crate::mir::Operand::Const(value) => self.mir_const_cg_ty(value),
        }
    }

    pub(super) fn mir_const_cg_ty(&self, value: &crate::mir::ConstValue) -> Option<CgTy> {
        match value {
            crate::mir::ConstValue::Bool(_) => Some(CgTy::Bool),
            crate::mir::ConstValue::Char => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            crate::mir::ConstValue::Unit => Some(CgTy::Unit),
            crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                self.cg_ty_of(self.builtins.int)
            }
            crate::mir::ConstValue::Float64 => Some(CgTy::Float64),
            crate::mir::ConstValue::Float32 => Some(CgTy::Float32),
            crate::mir::ConstValue::String => Some(CgTy::String),
        }
    }

    fn tuple_element_cg_ty(&self, tuple_ty: TypeId, index: usize) -> Option<CgTy> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return None;
        };
        let elem_ty = *elements.get(index)?;
        self.cg_ty_of(elem_ty)
    }

    pub(super) fn bind_mir_params(
        &mut self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let _hir_param = hir_fun
                .params
                .get(idx)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param arity",
                    at: param.span.into(),
                })?;
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let abi_ty = self.equivalent_codegen_type_id(mir_types, param.ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param type",
                    at: param.span.into(),
                },
            )?;
            let abi = self.ordinary_param_abi(param.span, abi_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm param",
                        at: param.span.into(),
                    })?
                    .into_pointer_value();
                let loaded =
                    self.builder
                        .build_load(pointee_ty, param_ptr, "pass_mir_param_load")?;
                self.cg_value_from_loaded(param.span, slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing pass MIR llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(super) fn bind_mir_params_without_hir(
        &mut self,
        mir_fun: &crate::mir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let init = if slot.cg_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing refactor plain MIR llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(super) fn codegen_mir_statement(
        &mut self,
        stmt: &crate::mir::Statement,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        used_locals: &HashSet<crate::mir::LocalId>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &stmt.kind {
            crate::mir::StatementKind::Nop => Ok(()),
            crate::mir::StatementKind::Assign { target, value } => {
                if !used_locals.contains(target)
                    && let crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) =
                        value
                    && self.fun_index.contains_key(fqn)
                {
                    return Ok(());
                }
                let slot = self.mir_local_slot(stmt.span, slots, *target)?;
                let target_source_ty = body
                    .locals
                    .get(target.as_u32() as usize)
                    .map(|local| local.ty);
                let value = self.codegen_mir_rvalue(
                    stmt.span,
                    value,
                    body,
                    mir_types,
                    slots,
                    slot.cg_ty,
                    target_source_ty,
                )?;
                let _ = self.store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)?;
                Ok(())
            }
            crate::mir::StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => self.codegen_mir_store_member(
                stmt.span,
                receiver,
                member,
                value,
                *value_ty,
                continuation_route,
                body,
                mir_types,
                slots,
            ),
            crate::mir::StatementKind::StoreTopLevelVar {
                fqn,
                value,
                value_ty,
            } => self.codegen_mir_store_top_level_var(stmt.span, fqn, value, *value_ty, slots),
            crate::mir::StatementKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR statement todo",
                at: stmt.span.into(),
            }),
        }
    }

    fn codegen_mir_terminator(
        &mut self,
        terminator: &crate::mir::Terminator,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        let mir_ctx = MirBodyCodegenCtx {
            body,
            mir_types,
            slots,
        };

        match &terminator.kind {
            crate::mir::TerminatorKind::Return { value } => {
                let value = match value {
                    Some(operand) => self.codegen_mir_operand_expected(
                        terminator.span,
                        operand,
                        slots,
                        Some(declared_return_cg),
                    )?,
                    None => self.default_value(terminator.span, declared_return_cg)?,
                };
                let value = self.coerce_value(terminator.span, value, declared_return_cg)?;
                self.finish_function_return_path(terminator.span, declared_return_cg, value)
            }
            crate::mir::TerminatorKind::Goto { target } => {
                let target_bb = llvm_blocks.get(target.as_u32() as usize).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR goto target",
                        at: terminator.span.into(),
                    },
                )?;
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let cond = self
                    .codegen_mir_operand(terminator.span, cond, slots)?
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR branch condition",
                        at: terminator.span.into(),
                    })?;
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR then target",
                        at: terminator.span.into(),
                    })?;
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR else target",
                        at: terminator.span.into(),
                    })?;
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::Unreachable => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            crate::mir::TerminatorKind::Perform {
                op_fqn,
                metadata,
                args,
                ..
            } => self.codegen_mir_perform_terminator(
                terminator.span,
                op_fqn,
                metadata,
                args,
                &terminator.unwind,
                mir_ctx,
            ),
            crate::mir::TerminatorKind::ResumeUnwind
            | crate::mir::TerminatorKind::Handle { .. }
            | crate::mir::TerminatorKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR terminator",
                at: terminator.span.into(),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Rvalue,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
        target_source_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            crate::mir::Rvalue::Transport { value, transport } => self.codegen_mir_value_transport(
                span, value, transport, body, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) => {
                if let Some(value) =
                    self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen_top_level_value_ref(span, fqn)
                }
            }
            crate::mir::Rvalue::Unary { op, operand } => {
                let operand = self.codegen_mir_operand(span, operand, slots)?;
                self.codegen_mir_unary(span, *op, operand)
            }
            crate::mir::Rvalue::Binary { lhs, op, rhs } => {
                let lhs = self.codegen_mir_operand(span, lhs, slots)?;
                let rhs = self.codegen_mir_operand(span, rhs, slots)?;
                self.codegen_mir_binary(span, *op, lhs, rhs)
            }
            crate::mir::Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => {
                self.codegen_mir_type_check(span, value, *op, *test_ty, metadata, mir_types, slots)
            }
            crate::mir::Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_mir_cast(
                span, value, *op, *target_ty, metadata, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::Call {
                kind,
                args,
                transport,
                ..
            } => self.codegen_mir_call(span, kind, args, transport, body, mir_types, slots),
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            crate::mir::Rvalue::MakeTuple { elements, .. } => {
                self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
            }
            crate::mir::Rvalue::SizeOf { value_ty } => {
                self.codegen_mir_size_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_mir_type_metadata_literal(span, metadata, mir_types)
            }
            crate::mir::Rvalue::StructLit { fields, .. } => {
                self.codegen_mir_make_struct(span, fields, target_cg, slots)
            }
            crate::mir::Rvalue::InterpolatedString { raw, parts } => {
                self.codegen_mir_interpolated_string(span, *raw, parts, body, mir_types, slots)
            }
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self.mir_operand_cg_ty(body, mir_types, env).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env type",
                        at: span.into(),
                    },
                )?;
                self.codegen_mir_make_closure(
                    span,
                    env,
                    fn_ptr,
                    env_contract,
                    mir_types,
                    env_cg,
                    target_cg,
                    slots,
                )
            }
            crate::mir::Rvalue::CaptureBoxNew { value, .. } => {
                self.codegen_mir_capture_box_new(span, value, body, mir_types, target_cg, slots)
            }
            crate::mir::Rvalue::CaptureBoxGet { box_operand, .. } => self
                .codegen_mir_capture_box_get(span, box_operand, body, mir_types, target_cg, slots),
            crate::mir::Rvalue::CaptureBoxSet {
                box_operand, value, ..
            } => self.codegen_mir_capture_box_set(span, box_operand, value, body, mir_types, slots),
            crate::mir::Rvalue::PerformResult { effect_ty, .. } => {
                let _ = self.codegen_mir_effect_instance_key(span, mir_types, *effect_ty)?;
                self.default_value(span, target_cg)
            }
            crate::mir::Rvalue::MemberAccess {
                receiver, member, ..
            } => self.codegen_mir_member_access(
                span,
                receiver,
                member,
                MirBodyCodegenCtx {
                    body,
                    mir_types,
                    slots,
                },
                target_cg,
            ),
            crate::mir::Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.codegen_mir_enum_variant_ctor_call(
                span,
                *enum_ty,
                variant_name,
                args,
                payload,
                body,
                mir_types,
                slots,
            ),
            crate::mir::Rvalue::ClassCtor {
                class_fqn,
                ctor,
                args,
                ..
            } => {
                let class_layout_key =
                    self.mir_class_ctor_layout_key(class_fqn, mir_types, target_source_ty);
                self.codegen_mir_refactor_class_ctor_call(
                    span,
                    &class_layout_key,
                    ctor,
                    args,
                    slots,
                )
            }
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR rvalue",
                at: span.into(),
            }),
        }
    }

    pub(super) fn codegen_mir_effect_neutral_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Rvalue,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            crate::mir::Rvalue::Transport { value, transport } => self.codegen_mir_value_transport(
                span, value, transport, body, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) => {
                if let Some(value) =
                    self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen_top_level_value_ref(span, fqn)
                }
            }
            crate::mir::Rvalue::Unary { op, operand } => {
                let operand = self.codegen_mir_operand(span, operand, slots)?;
                self.codegen_mir_unary(span, *op, operand)
            }
            crate::mir::Rvalue::Binary { lhs, op, rhs } => {
                let lhs = self.codegen_mir_operand(span, lhs, slots)?;
                let rhs = self.codegen_mir_operand(span, rhs, slots)?;
                self.codegen_mir_binary(span, *op, lhs, rhs)
            }
            crate::mir::Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => {
                self.codegen_mir_type_check(span, value, *op, *test_ty, metadata, mir_types, slots)
            }
            crate::mir::Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_mir_cast(
                span, value, *op, *target_ty, metadata, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            crate::mir::Rvalue::MakeTuple { elements, .. } => {
                self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
            }
            crate::mir::Rvalue::SizeOf { value_ty } => {
                self.codegen_mir_size_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_mir_type_metadata_literal(span, metadata, mir_types)
            }
            crate::mir::Rvalue::StructLit { fields, .. } => {
                self.codegen_mir_make_struct(span, fields, target_cg, slots)
            }
            crate::mir::Rvalue::InterpolatedString { raw, parts } => {
                self.codegen_mir_interpolated_string(span, *raw, parts, body, mir_types, slots)
            }
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::CaptureBoxNew { value, .. } => {
                self.codegen_mir_capture_box_new(span, value, body, mir_types, target_cg, slots)
            }
            crate::mir::Rvalue::CaptureBoxGet { box_operand, .. } => self
                .codegen_mir_capture_box_get(span, box_operand, body, mir_types, target_cg, slots),
            crate::mir::Rvalue::CaptureBoxSet {
                box_operand, value, ..
            } => self.codegen_mir_capture_box_set(span, box_operand, value, body, mir_types, slots),
            crate::mir::Rvalue::MemberAccess {
                receiver, member, ..
            } => self.codegen_mir_member_access(
                span,
                receiver,
                member,
                MirBodyCodegenCtx {
                    body,
                    mir_types,
                    slots,
                },
                target_cg,
            ),
            crate::mir::Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.codegen_mir_enum_variant_ctor_call(
                span,
                *enum_ty,
                variant_name,
                args,
                payload,
                body,
                mir_types,
                slots,
            ),
            crate::mir::Rvalue::Call { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor value primitive call requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::MakeClosure { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor value primitive closure carrier requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::ClassCtor { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor value primitive class construction requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::PerformResult { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor value primitive boundary payload requires published contract",
                at: span.into(),
            }),
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor value primitive rvalue",
                at: span.into(),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_value_transport(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        transport: &crate::mir::ValueTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(boxing) = transport.boxing.as_ref() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value erasure transport missing boxing intent",
                at: span.into(),
            });
        };
        if !matches!(
            boxing.reason,
            crate::mir::MirBoxingReason::AnyErasure | crate::mir::MirBoxingReason::RefErasure
        ) || boxing.source_ty != transport.source_ty
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value erasure transport boxing intent",
                at: span.into(),
            });
        }
        let source_ty = self
            .equivalent_codegen_type_id(mir_types, transport.source_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "value erasure transport source type",
                at: span.into(),
            })?;
        let source_cg = self
            .cg_ty_of(source_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "value erasure transport source codegen type",
                at: span.into(),
            })?;

        if matches!(
            mir_types.kind(transport.source_ty),
            TypeKind::Value(ValueTypeKind::Nothing)
        ) {
            return self.default_value(span, target_cg);
        }

        if target_cg == CgTy::String {
            return self.codegen_mir_transport_to_string(
                span,
                value,
                transport.source_ty,
                source_cg,
                mir_types,
                slots,
            );
        }
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value erasure transport target type",
                at: span.into(),
            });
        }
        let body_fqn = self.current_codegen_body_fqn();
        let _descriptor = self.get_or_create_value_composite_transport_descriptor_global(
            &body_fqn, span, mir_types, transport,
        )?;

        match source_cg {
            CgTy::Tuple(_) | CgTy::Struct(_) => self.codegen_mir_composite_value_box(
                span, value, source_ty, source_cg, body, mir_types, slots,
            ),
            CgTy::Enum(_) if transport.kind == crate::mir::MirTransportKind::EnumPayload => self
                .codegen_mir_composite_value_box(
                    span, value, source_ty, source_cg, body, mir_types, slots,
                ),
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref | CgTy::Enum(_) => {
                let source =
                    self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
                let source = self.coerce_value(span, source, source_cg)?;
                self.coerce_value(span, source, CgTy::Ref)
            }
            CgTy::Float64 | CgTy::Float32 | CgTy::Never => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "value erasure transport source kind",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_mir_transport_to_string(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        source_ty: TypeId,
        source_cg: CgTy,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source = self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
        let source = self.coerce_value(span, source, source_cg)?;
        match source_cg {
            CgTy::String => self.coerce_value(span, source, CgTy::String),
            CgTy::Bool => {
                let Some(BasicValueEnum::IntValue(raw)) = source.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR transport Bool.toString value",
                        at: span.into(),
                    });
                };
                let widened = self.builder.build_int_z_extend(
                    raw,
                    self.context.i64_type(),
                    "mir_transport_bool_to_string_arg",
                )?;
                let runtime = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[widened.into()],
                    "mir_transport_bool_to_string",
                )?;
                self.string_value_from_runtime_call(span, call, "MIR transport Bool.toString")
            }
            CgTy::Int(from_ty)
                if matches!(
                    mir_types.kind(source_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) =>
            {
                let Some(BasicValueEnum::IntValue(raw)) = source.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR transport Char.toString value",
                        at: span.into(),
                    });
                };
                let codepoint = self.cast_int(
                    raw,
                    from_ty,
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                )?;
                let str_ptr = self.codegen_char_to_string_value(span, codepoint)?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Int(from_ty) => {
                let Some(BasicValueEnum::IntValue(raw)) = source.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR transport Int.toString value",
                        at: span.into(),
                    });
                };
                let widened = self.cast_int(
                    raw,
                    from_ty,
                    IntTy {
                        bits: 64,
                        signed: from_ty.signed,
                    },
                )?;
                let runtime = self.declare_runtime_int_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[widened.into()],
                    "mir_transport_int_to_string",
                )?;
                self.string_value_from_runtime_call(span, call, "MIR transport Int.toString")
            }
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_to_string_value(span, span, source),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR transport to String source type",
                at: span.into(),
            }),
        }
    }

    fn string_value_from_runtime_call(
        &self,
        span: crate::span::Span,
        call: inkwell::values::CallSiteValue<'ctx>,
        kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        };
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_composite_value_box(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        source_ty: TypeId,
        source_cg: CgTy,
        _body: &crate::mir::Body,
        _mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source = self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
        let source = self.coerce_value(span, source, source_cg)?;
        let deferred_source =
            self.defer_gc_sensitive_cg_value(span, "mir_value_box_source", source)?;

        let box_obj_ty = self.mir_value_box_object_type(span, source_ty, source_cg)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_value_box_type_desc_global(span, source_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "mir_value_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            "rt_alloc_mir_value_box",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed value box return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed value box return type",
                at: span.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "mir_value_box_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "mir_value_box_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "mir_value_box_obj_reload",
            &deferred_obj,
        )?;
        let payload_gep =
            self.builder
                .build_struct_gep(box_obj_ty, obj_ptr, 1, "mir_value_box_payload_gep")?;
        let payload = self.materialize_deferred_cg_value(
            span,
            "mir_value_box_source_reload",
            deferred_source,
        )?;
        let _ = self.store_local_value(span, payload_gep, source_cg, payload)?;
        let obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "mir_value_box_return",
            &deferred_obj,
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn current_codegen_body_fqn(&self) -> String {
        self.function_cx
            .current_callable_fqn
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    fn codegen_mir_type_metadata_literal(
        &mut self,
        span: crate::span::Span,
        metadata: &crate::mir::TypeMetadataLiteral,
        mir_types: &TypeStore,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match metadata.kind {
            crate::mir::TypeMetadataLiteralKind::TypeNameString => {
                let type_name = metadata
                    .source_fqn
                    .clone()
                    .unwrap_or_else(|| mir_types.display(metadata.source_ty).to_string());
                self.codegen_string_literal_from_text(span, &type_name)
            }
        }
    }

    pub(super) fn codegen_platform_literal(
        &mut self,
        span: crate::span::Span,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic result type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic nominal Platform type",
                at: span.into(),
            });
        };
        if nominal.fqn != "scoop.core.Platform" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic Platform target",
                at: span.into(),
            });
        }

        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic Platform layout",
                    at: span.into(),
                })?;
        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let (arch, vendor, os, env) = decompose_target_triple(&self.host.triple);
        let field_values = [
            ("triple", self.host.triple.as_str()),
            ("arch", arch.as_str()),
            ("vendor", vendor.as_str()),
            ("os", os.as_str()),
            ("env", env.as_str()),
        ];

        let mut deferred_fields: Vec<(u32, String, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());
        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let (_, text) = field_values
                .iter()
                .find(|(name, _)| *name == layout_field.name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic Platform field",
                    at: span.into(),
                })?;
            let field_cg =
                self.cg_ty_of_layout_field(span, layout_field.ty, layout_field.ty_fqn.as_deref())?;
            let value = self.codegen_string_literal_from_text(span, text)?;
            let value = if value.ty != field_cg {
                self.coerce_value(span, value, field_cg)?
            } else {
                value
            };
            let deferred = self.defer_gc_sensitive_cg_value(
                span,
                &format!("get_platform_field_{idx}"),
                value,
            )?;
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, layout_field.name.clone(), deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, deferred)) in deferred_fields.into_iter().enumerate() {
            let materialized = self.materialize_deferred_cg_value(
                span,
                &format!("get_platform_field_reload_{idx}"),
                deferred,
            )?;
            let raw = materialized
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic field value",
                    at: span.into(),
                })?;
            agg = self.builder.build_insert_value(
                agg,
                raw,
                llvm_idx,
                &format!("get_platform_insert_{field_name}"),
            )?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_type_check(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if metadata.target_ty != test_ty || metadata.descriptor.ty != test_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime type-check metadata",
                at: span.into(),
            });
        }
        let is_ok =
            self.codegen_mir_runtime_type_test_is_ok(span, value, metadata, mir_types, slots)?;
        let out = match op {
            ast::TypeCheckOp::Is => is_ok,
            ast::TypeCheckOp::NotIs => self.builder.build_not(is_ok, "mir_typecheck_not")?,
        };
        Ok(CgValue::bool(out))
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_cast(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        op: ast::CastOp,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if metadata.test.target_ty != target_ty || metadata.test.descriptor.ty != target_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime cast metadata",
                at: span.into(),
            });
        }
        match op {
            ast::CastOp::As => self.codegen_mir_cast_as(
                span, value, target_ty, metadata, mir_types, slots, target_cg,
            ),
            ast::CastOp::AsQ => self.codegen_mir_cast_asq(
                span, value, target_ty, metadata, mir_types, slots, target_cg,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_cast_as(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let crate::mir::RuntimeCastFailure::Raise { error_fqn, .. } = &metadata.failure else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as` cast failure contract",
                at: span.into(),
            });
        };
        if error_fqn != "scoop.core.RuntimeError.ClassCastFailed" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as` cast runtime error contract",
                at: span.into(),
            });
        }
        let crate::mir::RuntimeCastResult::Target { ty } = &metadata.result else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as` cast result contract",
                at: span.into(),
            });
        };
        if *ty != target_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as` cast target contract",
                at: span.into(),
            });
        }

        let target_codegen_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as` target codegen type",
                at: span.into(),
            })?;
        let expected_cg =
            self.cg_ty_of(target_codegen_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "MIR `as` target type",
                    at: span.into(),
                })?;
        let result_cg = if target_cg == CgTy::Never {
            expected_cg
        } else {
            target_cg
        };
        if expected_cg != result_cg || !matches!(result_cg, CgTy::Ref | CgTy::String) {
            return Err(frontend_error(format!(
                "MIR `as` target runtime type mismatch: target_ty={}, expected_cg={expected_cg:?}, result_cg={target_cg:?}",
                mir_types.display(target_ty)
            )));
        }

        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        if metadata.test.static_fold == crate::mir::RuntimeTypeStaticFold::AlwaysTrue {
            let target_ptr_ty = self.runtime_cast_target_ptr_type(span, result_cg)?;
            let casted_ptr =
                self.builder
                    .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_cast_verified_ptr")?;
            return Ok(CgValue {
                ty: result_cg,
                value: Some(casted_ptr.into()),
            });
        }
        let is_ok = self.codegen_mir_runtime_type_test_is_ok(
            span,
            value,
            &metadata.test,
            mir_types,
            slots,
        )?;
        self.codegen_checked_runtime_ref_cast(span, obj_ptr, target_codegen_ty, result_cg, is_ok)
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_cast_asq(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !matches!(metadata.failure, crate::mir::RuntimeCastFailure::ReturnNone) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` cast failure contract",
                at: span.into(),
            });
        }
        let crate::mir::RuntimeCastResult::Option { option_ty, some_ty } = &metadata.result else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` cast result contract",
                at: span.into(),
            });
        };
        if *some_ty != target_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` cast target contract",
                at: span.into(),
            });
        }

        let target_codegen_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` target codegen type",
                at: span.into(),
            })?;
        let target_value_cg =
            self.cg_ty_of(target_codegen_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "MIR `as?` target type",
                    at: span.into(),
                })?;
        if !matches!(target_value_cg, CgTy::Ref | CgTy::String) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` target runtime type",
                at: span.into(),
            });
        }
        let option_codegen_ty = self
            .equivalent_codegen_type_id(mir_types, *option_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` option codegen type",
                at: span.into(),
            })?;
        if target_cg != CgTy::Enum(option_codegen_ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR `as?` result type",
                at: span.into(),
            });
        }

        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        let is_ok = self.codegen_mir_runtime_type_test_is_ok(
            span,
            value,
            &metadata.test,
            mir_types,
            slots,
        )?;
        self.codegen_checked_runtime_ref_cast_option(
            span,
            obj_ptr,
            target_codegen_ty,
            target_value_cg,
            option_codegen_ty,
            is_ok,
        )
    }

    fn codegen_mir_runtime_type_test_is_ok(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        match metadata.static_fold {
            crate::mir::RuntimeTypeStaticFold::AlwaysTrue => {
                return Ok(self.context.bool_type().const_int(1, false));
            }
            crate::mir::RuntimeTypeStaticFold::AlwaysFalse => {
                return Ok(self.context.bool_type().const_int(0, false));
            }
            crate::mir::RuntimeTypeStaticFold::Dynamic => {}
        }

        if !self.runtime_type_descriptor_is_codegen_supported(mir_types, metadata) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime type descriptor",
                at: span.into(),
            });
        }
        let target_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime type target",
                at: span.into(),
            })?;
        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)
    }

    fn codegen_mir_runtime_ref_operand(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(PointerValue<'ctx>, CgValue<'ctx>), LlvmEmitError> {
        let value = self.codegen_mir_operand(span, value, slots)?;
        let value = match value.ty {
            CgTy::Ref => value,
            CgTy::String => self.coerce_value(span, value, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "MIR runtime type operand",
                    at: span.into(),
                });
            }
        };
        let Some(BasicValueEnum::PointerValue(obj_ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime type operand value",
                at: span.into(),
            });
        };
        Ok((obj_ptr, value))
    }

    fn runtime_cast_target_ptr_type(
        &self,
        span: crate::span::Span,
        target_cg: CgTy,
    ) -> Result<inkwell::types::PointerType<'ctx>, LlvmEmitError> {
        match target_cg {
            CgTy::Ref => Ok(self.llvm_gc_i8_ptr_type()),
            CgTy::String => Ok(self.llvm_scoop_string_ptr_type()),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR runtime cast target type",
                at: span.into(),
            }),
        }
    }

    fn codegen_checked_runtime_ref_cast(
        &mut self,
        span: crate::span::Span,
        obj_ptr: PointerValue<'ctx>,
        _target_ty: TypeId,
        target_cg: CgTy,
        is_ok: inkwell::values::IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_ptr_ty = self.runtime_cast_target_ptr_type(span, target_cg)?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "mir_cast_ok");
        let fail_bb = self.context.append_basic_block(func, "mir_cast_fail");
        let merge_bb = self.context.append_basic_block(func, "mir_cast_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_cast_ptr")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(fail_bb);
        self.emit_raise_runtime_error_variant(span, "ClassCastFailed")?;
        let fail_incoming = if self.ordinary_effect_propagation_enabled() {
            self.emit_ordinary_non_resuming_effect_exit(span, "mir_cast_raise_effect")?;
            self.builder.build_unreachable()?;
            None
        } else {
            let dead_bb =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "builder has no insert block",
                        at: span.into(),
                    })?;
            let default_ptr = target_ptr_ty.const_null();
            self.builder.build_unconditional_branch(merge_bb)?;
            Some((default_ptr, dead_bb))
        };

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(target_ptr_ty, "mir_cast_value")?;
        if let Some((default_ptr, dead_bb)) = fail_incoming {
            phi.add_incoming(&[(&casted_ptr, ok_bb), (&default_ptr, dead_bb)]);
        } else {
            phi.add_incoming(&[(&casted_ptr, ok_bb)]);
        }
        Ok(CgValue {
            ty: target_cg,
            value: Some(phi.as_basic_value().into_pointer_value().into()),
        })
    }

    fn codegen_checked_runtime_ref_cast_option(
        &mut self,
        span: crate::span::Span,
        obj_ptr: PointerValue<'ctx>,
        _target_ty: TypeId,
        target_cg: CgTy,
        option_ty: TypeId,
        is_ok: inkwell::values::IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_ptr_ty = self.runtime_cast_target_ptr_type(span, target_cg)?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "mir_asq_ok");
        let fail_bb = self.context.append_basic_block(func, "mir_asq_fail");
        let merge_bb = self.context.append_basic_block(func, "mir_asq_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        self.builder.position_at_end(ok_bb);
        let casted_ptr =
            self.builder
                .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_asq_cast_ptr")?;
        let casted = CgValue {
            ty: target_cg,
            value: Some(casted_ptr.into()),
        };
        let payload = self.coerce_enum_payload(span, casted, target_cg)?;
        let some_v = self.build_enum_value(span, option_ty, 0, payload)?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "MIR `as?` Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(fail_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "MIR `as?` None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let llvm_option_ty = self.llvm_enum_value_type(span, option_ty)?;
        let phi = self.builder.build_phi(llvm_option_ty, "mir_asq_value")?;
        phi.add_incoming(&[(&some_raw, ok_bb), (&none_raw, fail_bb)]);
        Ok(CgValue {
            ty: CgTy::Enum(option_ty),
            value: Some(phi.as_basic_value()),
        })
    }

    fn codegen_mir_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[crate::mir::InterpolatedStringPart],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let scoop_str_ty = self.llvm_scoop_string_type();
        let mut segments = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            let segment = match part {
                crate::mir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.current_source_slice(*text_span)?;
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid MIR interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;
                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "mir_fstr_text_ptr",
                    )?;
                    MirInterpolatedSegment {
                        ptr,
                        len: i64_ty.const_int(bytes.len() as u64, false),
                    }
                }
                crate::mir::InterpolatedStringPart::Expr {
                    span: expr_span,
                    value,
                    ty,
                } => {
                    let source_ty = self.mir_operand_type_id(body, value).unwrap_or(*ty);
                    let value_cg = match value {
                        crate::mir::Operand::Local(local) => {
                            self.mir_local_slot(*expr_span, slots, *local)?.cg_ty
                        }
                        crate::mir::Operand::Const(_) => self
                            .cg_ty_of_mir_type(mir_types, source_ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "MIR interpolated string expr type",
                                at: (*expr_span).into(),
                            })?,
                    };
                    let v = self.codegen_mir_operand_expected(
                        *expr_span,
                        value,
                        slots,
                        Some(value_cg),
                    )?;
                    let v = self.coerce_value(*expr_span, v, value_cg)?;
                    self.codegen_mir_interpolated_expr_segment(*expr_span, source_ty, v, mir_types)?
                }
            };
            total_len = self
                .builder
                .build_int_add(total_len, segment.len, "mir_fstr_total_len")?;
            segments.push(segment);
        }

        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "mir_fstr_total_is_zero",
        )?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let malloc_bb = self.context.append_basic_block(func, "mir_fstr_malloc");
        let done_bb = self.context.append_basic_block(func, "mir_fstr_done");
        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[total_len.into()], "mir_fstr_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };

        let mut cursor = i64_ty.const_zero();
        for (idx, seg) in segments.iter().enumerate() {
            let dst = unsafe {
                self.builder.build_in_bounds_gep(
                    i8_ty,
                    buf,
                    &[cursor],
                    &format!("mir_fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self
                .builder
                .build_int_add(cursor, seg.len, "mir_fstr_cursor")?;
        }
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "mir_fstr_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let buf_ptr = buf_phi.as_basic_value().into_pointer_value();

        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);
        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "mir_fstr_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_mir_fstr",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "mir_fstr_obj_ptr")?;
        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 1, "mir_fstr_len_gep")?;
        let data_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 2, "mir_fstr_data_gep")?;
        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    pub(super) fn codegen_mir_unresolved_name_with_source_ty(
        &mut self,
        span: crate::span::Span,
        name: &str,
        source_types: &TypeStore,
        source_ty: TypeId,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_cg = self.cg_ty_of_mir_type(source_types, source_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "MIR unresolved name source type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_unresolved_ident(span, name, Some(source_cg))?;
        self.coerce_value(span, value, target_cg)
    }

    fn codegen_mir_interpolated_expr_segment(
        &mut self,
        span: crate::span::Span,
        source_ty: TypeId,
        v: CgValue<'ctx>,
        mir_types: &TypeStore,
    ) -> Result<MirInterpolatedSegment<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        match v.ty {
            CgTy::String => {
                let coerced = self.coerce_value(span, v, CgTy::String)?;
                let Some(BasicValueEnum::PointerValue(str_obj_ptr)) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation expr value",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Bool => {
                let Some(BasicValueEnum::IntValue(bool_val)) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool expr value",
                        at: span.into(),
                    });
                };
                let bool_as_i64 =
                    self.builder
                        .build_int_z_extend(bool_val, i64_ty, "mir_fstr_bool_zext")?;
                let rt_bool = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_bool,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string_for_mir_fstr",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool return type",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Float64 | CgTy::Float32 => {
                let str_v = self.codegen_float_to_string_value(span, span, v)?;
                let Some(BasicValueEnum::PointerValue(str_obj_ptr)) = str_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation float return type",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Int(from_ty)
                if matches!(
                    mir_types.kind(source_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) =>
            {
                let Some(BasicValueEnum::IntValue(codepoint)) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation char expr value",
                        at: span.into(),
                    });
                };
                let codepoint = self.cast_int(
                    codepoint,
                    from_ty,
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                )?;
                let str_obj_ptr = self.codegen_char_to_string_value(span, codepoint)?;
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Int(from_ty) => {
                if from_ty.bits > 64 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "integer width for MIR string interpolation",
                        at: span.into(),
                    });
                }
                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "MIR integer interpolation expr value",
                    at: span.into(),
                })?;
                let to_ty = IntTy {
                    bits: 64,
                    signed: from_ty.signed,
                };
                let int64 = self.cast_int(raw_int, from_ty, to_ty)?;
                let cap = i64_ty.const_int(64, false);
                let buf = self.builder.build_array_alloca(
                    self.context.i8_type(),
                    cap,
                    "mir_fstr_int_buf",
                )?;
                let fmt_name = if from_ty.signed {
                    "scoop_format_i64"
                } else {
                    "scoop_format_u64"
                };
                let fmt_fun = self.declare_runtime_format_int(fmt_name);
                let call_site = self.builder.build_call(
                    fmt_fun,
                    &[int64.into(), buf.into(), cap.into()],
                    "mir_fstr_fmt_int",
                )?;
                let len = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation int length",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(MirInterpolatedSegment { ptr: buf, len })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR string interpolation expr type",
                at: span.into(),
            }),
        }
    }

    fn codegen_mir_member_access(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        mir_ctx: MirBodyCodegenCtx<'_, 'ctx>,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(fqn) = self.mir_member_resolved_top_level_value_fqn(member) {
            let value = if self.lookup_object_property_by_fqn(fqn).is_some() {
                self.codegen_object_property_access(span, fqn)?
            } else if let Some(value) =
                self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
            {
                value
            } else {
                self.codegen_top_level_value_ref(span, fqn)?
            };
            return self.coerce_value(span, value, target_cg);
        }
        let place = self.codegen_mir_member_place(span, receiver, member, mir_ctx, false)?;
        let same_layout = self.cg_ty_layout_equivalent(place.field_cg, target_cg);
        if !same_layout {
            return Err(frontend_error(format!(
                "pass MIR member access result type drift: field={} target={}",
                self.describe_cg_ty(place.field_cg),
                self.describe_cg_ty(target_cg),
            )));
        }
        if place.field_cg == CgTy::Unit {
            return self.coerce_value(span, CgValue::unit(), target_cg);
        }
        let llvm_ty = self.llvm_basic_type_of(span, place.field_cg)?;
        let loaded = self
            .builder
            .build_load(llvm_ty, place.ptr, "pass_mir_member_load")?;
        if let Some(alignment) = place.packed_alignment
            && let Some(inst) = loaded.as_instruction_value()
        {
            inst.set_alignment(alignment)?;
        }
        let value = self.cg_value_from_loaded(span, place.field_cg, loaded)?;
        if place.field_cg != target_cg {
            return Ok(CgValue {
                ty: target_cg,
                value: value.value,
            });
        }
        self.coerce_value(span, value, target_cg)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[crate::mir::CallArg],
        payload: &crate::mir::AggregateTransportMetadata,
        _body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let enum_ty = self.equivalent_codegen_type_id(mir_types, enum_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR enum ctor type",
                at: span.into(),
            },
        )?;
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR unknown enum variant",
                at: span.into(),
            })?
            .clone();
        if variant.fields.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR enum variant ctor arity",
                at: span.into(),
            });
        }
        if !self.mir_enum_payload_schema_matches(mir_types, enum_ty, &variant, args, payload) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR enum payload schema",
                at: span.into(),
            });
        }
        let mut field_values = Vec::with_capacity(args.len());
        for (idx, (field_cg, arg)) in variant.fields.iter().copied().zip(args).enumerate() {
            if arg.name.is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR named enum ctor arg",
                    at: span.into(),
                });
            }
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(field_cg))?;
            let coerced = self.coerce_value(arg.span, value, field_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_enum_ctor_field_{idx}"),
                coerced,
            )?;
            field_values.push((arg.span, field_cg, deferred));
        }
        self.build_enum_variant_value_from_field_values(span, enum_ty, variant_name, &field_values)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_store_member(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        value: &crate::mir::Operand,
        value_ty: TypeId,
        continuation_route: &crate::mir::StoredContinuationRoutePublication,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        mir_store_member_continuation_route_is_lowerable(span, body, continuation_route)?;

        let mir_ctx = MirBodyCodegenCtx {
            body,
            mir_types,
            slots,
        };
        let place = self.codegen_mir_member_place(span, receiver, member, mir_ctx, true)?;
        if !place.writable {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member store target not writable",
                at: span.into(),
            });
        }
        let _value_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member store value type",
                at: span.into(),
            },
        )?;
        let _operand_cg = self.mir_operand_cg_ty(body, mir_types, value).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member store operand type",
                at: span.into(),
            },
        )?;

        let value = self.codegen_mir_operand_expected(span, value, slots, Some(place.field_cg))?;
        let stored = self.coerce_value(span, value, place.field_cg)?;
        let _ = self.store_local_value(span, place.ptr, place.field_cg, stored)?;
        Ok(())
    }

    pub(super) fn codegen_mir_store_top_level_var(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        value: &crate::mir::Operand,
        _value_ty: TypeId,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        if let Some(global) = self.materialized_extern_global_root(fqn).cloned() {
            if !global.mutable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR extern global store target immutable",
                    at: span.into(),
                });
            }
            let target_cg = self
                .cg_ty_of(global.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR extern global store target type",
                    at: span.into(),
                })?;
            let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
            let stored = self.coerce_value(span, raw, target_cg)?;
            let global = self.declare_mir_extern_global(&global)?;
            let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
            return Ok(());
        }

        if let Some(global) = self.extern_globals.get(fqn).cloned() {
            if !global.mutable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR extern global store target immutable",
                    at: span.into(),
                });
            }
            let target_cg = self
                .cg_ty_of(global.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR extern global store target type",
                    at: span.into(),
                })?;
            let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
            let stored = self.coerce_value(span, raw, target_cg)?;
            let global = self.declare_extern_global(&global)?;
            let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
            return Ok(());
        }

        let var = self
            .top_level_vars
            .get(fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR top-level var store target",
                at: span.into(),
            })?;
        let target_cg = self
            .cg_ty_of(var.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR top-level var store target type",
                at: span.into(),
            })?;
        let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
        let stored = self.coerce_value(span, raw, target_cg)?;
        let global = self.declare_top_level_var_global(var)?;
        let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
        Ok(())
    }

    fn codegen_mir_member_place(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        mir_ctx: MirBodyCodegenCtx<'_, 'ctx>,
        require_writable: bool,
    ) -> Result<MirMemberPlace<'ctx>, LlvmEmitError> {
        let field_fqn = mir_member_value_fqn_for_codegen(span, member)?;
        let receiver_type_id = self.mir_member_receiver_codegen_type_id(
            span,
            mir_ctx.body,
            mir_ctx.mir_types,
            receiver,
            member,
        )?;
        if let Some((class, field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
        {
            let receiver_cg = self
                .mir_operand_cg_ty(mir_ctx.body, mir_ctx.mir_types, receiver)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR class member receiver operand type",
                    at: span.into(),
                })?;
            if receiver_cg == CgTy::Ref {
                let field = class.fields.get(field_idx as usize).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR class member field index",
                        at: span.into(),
                    },
                )?;
                if require_writable && !field.mutable {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR immutable class member store",
                        at: span.into(),
                    });
                }
                let receiver_value = self.codegen_mir_operand_expected(
                    span,
                    receiver,
                    mir_ctx.slots,
                    Some(CgTy::Ref),
                )?;
                let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
                let Some(raw) = receiver_value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR class member receiver value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR class member receiver type",
                        at: span.into(),
                    });
                };
                let ptr = self.codegen_class_field_ptr(span, &class, obj_ptr, field_idx)?;
                return Ok(MirMemberPlace {
                    ptr,
                    field_cg,
                    writable: field.mutable,
                    packed_alignment: None,
                });
            }
        }

        let receiver_cg =
            self.cg_ty_of(receiver_type_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR member receiver type",
                    at: span.into(),
                })?;
        let CgTy::Struct(struct_ty) = receiver_cg else {
            return Err(frontend_error(format!(
                "pass MIR member field target `{field_fqn}` receiver_ty=t{} receiver_cg={}",
                receiver_type_id.as_u32(),
                self.describe_cg_ty(receiver_cg),
            )));
        };
        let (field_idx, field_cg) = self.lookup_struct_field(struct_ty, field_fqn, span)?;
        let crate::mir::Operand::Local(local) = receiver else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member store receiver place",
                at: span.into(),
            });
        };
        let slot = self.mir_local_slot(span, mir_ctx.slots, *local)?;
        if slot.cg_ty != CgTy::Struct(struct_ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR member receiver type drift",
                at: span.into(),
            });
        }
        let local_ptr = self.local_ptr_for_use(
            span,
            CgLocal {
                hir_ty: None,
                call_may_suspend: false,
                ty: slot.cg_ty,
                ptr: slot.ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
            "pass_mir_member_base",
        )?;
        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let ptr = self.builder.build_struct_gep(
            llvm_struct_ty,
            local_ptr,
            field_idx,
            "pass_mir_member_gep",
        )?;
        let packed_alignment = if let Some(pack_n) = self
            .struct_clayout(struct_ty)
            .and_then(|layout| layout.packed)
        {
            if require_writable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR packed struct member store",
                    at: span.into(),
                });
            }
            let field_ty = self.llvm_basic_type_of(span, field_cg)?;
            let natural = self.target_data.get_abi_alignment(&field_ty);
            Some(std::cmp::min(natural, pack_n))
        } else {
            None
        };
        Ok(MirMemberPlace {
            ptr,
            field_cg,
            writable: matches!(receiver, crate::mir::Operand::Local(_)),
            packed_alignment,
        })
    }

    fn codegen_mir_effect_instance_key(
        &self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        effect_ty: TypeId,
    ) -> Result<u32, LlvmEmitError> {
        let effect_ty = self
            .equivalent_codegen_type_id(mir_types, effect_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR perform effect type",
                at: span.into(),
            })?;
        self.effect_instance_key(effect_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR perform effect instance key",
                at: span.into(),
            })
    }

    pub(super) fn codegen_mir_operand(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_operand_expected(span, operand, slots, None)
    }

    pub(super) fn codegen_mir_operand_expected(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match operand {
            crate::mir::Operand::Local(local) => {
                let slot = self.mir_local_slot(span, slots, *local)?;
                self.load_mir_local(span, slot)
            }
            crate::mir::Operand::Const(value) => self.codegen_mir_const(span, value, expected),
        }
    }

    pub(super) fn codegen_mir_sysroot_gc_handle_new(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew arg contract",
                at: span.into(),
            });
        }
        let Some(CgTy::Struct(handle_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew call without expected handle type",
                at: span.into(),
            });
        };
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", span)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew raw field type",
                at: span.into(),
            });
        };

        let arg = &args[0];
        let obj_v =
            self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(CgTy::Ref))?;
        let obj_ref = self.coerce_value(arg.span, obj_v, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(obj_ptr)) = obj_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew arg value",
                at: arg.span.into(),
            });
        };

        let rt_handle_new = self.declare_runtime_gc_handle_new();
        let call =
            self.builder
                .build_call(rt_handle_new, &[obj_ptr.into()], "mir_gc_handle_new")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(handle_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleNew return type",
                at: span.into(),
            });
        };
        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            handle_i64,
            self.context.i64_type().const_zero(),
            "mir_gc_handle_new_ok",
        )?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        let handle_word = self.cast_int(
            handle_i64,
            IntTy {
                bits: 64,
                signed: false,
            },
            field_int_ty,
        )?;
        let llvm_struct_ty = self.llvm_struct_type(span, handle_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        agg = self.builder.build_insert_value(
            agg,
            handle_word.as_basic_value_enum(),
            field_idx,
            "mir_gc_handle_raw",
        )?;
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(handle_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_mir_sysroot_gc_handle_get(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet arg contract",
                at: span.into(),
            });
        }
        if expected.is_some_and(|ty| ty != CgTy::Ref) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet target type",
                at: span.into(),
            });
        }

        let arg = &args[0];
        let handle_v = self.codegen_mir_operand(arg.span, &arg.value, slots)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet arg type",
                at: arg.span.into(),
            });
        };
        let Some(BasicValueEnum::StructValue(struct_v)) = handle_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet arg value",
                at: arg.span.into(),
            });
        };
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", arg.span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "mir_gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet raw field type",
                at: arg.span.into(),
            });
        };
        let Some(BasicValueEnum::IntValue(handle_word)) = field_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet raw value",
                at: arg.span.into(),
            });
        };
        let handle_i64 = self.cast_int(
            handle_word,
            field_int_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        let rt_handle_get = self.declare_runtime_gc_handle_get();
        let call =
            self.builder
                .build_call(rt_handle_get, &[handle_i64.into()], "mir_gc_handle_get")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleGet return type",
                at: span.into(),
            });
        };

        let obj_is_null = self
            .builder
            .build_is_null(obj_ptr, "mir_gc_handle_get_is_null")?;
        let ok_cond = self
            .builder
            .build_not(obj_is_null, "mir_gc_handle_get_ok")?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_ptr.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_mir_sysroot_gc_handle_drop(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let handle_v = self.codegen_mir_operand(arg.span, &arg.value, slots)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop arg type",
                at: arg.span.into(),
            });
        };
        let Some(BasicValueEnum::StructValue(struct_v)) = handle_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop arg value",
                at: arg.span.into(),
            });
        };
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", arg.span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "mir_gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop raw field type",
                at: arg.span.into(),
            });
        };
        let Some(BasicValueEnum::IntValue(handle_word)) = field_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop raw value",
                at: arg.span.into(),
            });
        };
        let handle_i64 = self.cast_int(
            handle_word,
            field_int_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        let rt_handle_drop = self.declare_runtime_gc_handle_drop();
        let call =
            self.builder
                .build_call(rt_handle_drop, &[handle_i64.into()], "mir_gc_handle_drop")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR GC.handleDrop return type",
                at: span.into(),
            });
        };
        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "mir_gc_handle_drop_ok",
        )?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    fn codegen_mir_const(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::ConstValue,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::ConstValue::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            crate::mir::ConstValue::Char => {
                let text = self.current_source_slice(span)?;
                let value =
                    crate::syntax::char_literal::parse_char_literal(text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR char literal",
                            at: span.into(),
                        }
                    })?;
                Ok(CgValue::int(
                    self.context.i32_type().const_int(value as u64, false),
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                ))
            }
            crate::mir::ConstValue::Unit => Ok(CgValue::unit()),
            crate::mir::ConstValue::Int => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR int builtin type",
                            at: span.into(),
                        });
                    }
                };
                let bits = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(bits, false),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::SynthInt(value) => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR synthesized int builtin type",
                            at: span.into(),
                        });
                    }
                };
                Ok(CgValue::int(
                    self.int_type(int_ty)
                        .const_int(*value as u64, int_ty.signed),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::Float64 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f64_type().const_float(parsed.value),
                    CgTy::Float64,
                ))
            }
            crate::mir::ConstValue::Float32 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f32_type().const_float(parsed.value),
                    CgTy::Float32,
                ))
            }
            crate::mir::ConstValue::String => self.codegen_string_literal(span),
        }
    }

    fn codegen_mir_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: &crate::mir::Operand,
        pattern: &crate::mir::Pattern,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let subject = self.codegen_mir_operand(span, subject, slots)?;
        let cond = self.codegen_mir_pattern_match_value(span, mir_types, subject, pattern)?;
        Ok(CgValue::bool(cond))
    }

    fn codegen_mir_pattern_match_value(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        pattern: &crate::mir::Pattern,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pattern {
            crate::mir::Pattern::Else
            | crate::mir::Pattern::Wildcard
            | crate::mir::Pattern::Rest
            | crate::mir::Pattern::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            crate::mir::Pattern::Or { pats } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for pat in pats {
                    let pat_cond =
                        self.codegen_mir_pattern_match_value(span, mir_types, subject, pat)?;
                    cond = self
                        .builder
                        .build_or(cond, pat_cond, "pass_mir_pattern_or")?;
                }
                Ok(cond)
            }
            crate::mir::Pattern::Is { ty, metadata } => {
                self.codegen_mir_is_pattern_match(span, mir_types, subject, *ty, metadata)
            }
            crate::mir::Pattern::Tuple { elements } => {
                self.codegen_mir_tuple_pattern_match(span, mir_types, subject, elements)
            }
            crate::mir::Pattern::Variant { name, args } => {
                self.codegen_mir_variant_pattern_match(span, mir_types, subject, name, args)
            }
            crate::mir::Pattern::IntLit { raw } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern int subject",
                        at: span.into(),
                    })?;
                let expected = self.int_literal_bits_from_text_for_ty(span, raw, int_ty)?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(expected, false),
                    "pass_mir_pattern_int_eq",
                )?)
            }
            crate::mir::Pattern::CharLit { value: expected } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern char subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(*expected as u64, false),
                    "pass_mir_pattern_char_eq",
                )?)
            }
            crate::mir::Pattern::StringLit { value } => {
                let CgTy::String = subject.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string subject",
                        at: span.into(),
                    });
                };
                let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string value",
                        at: span.into(),
                    });
                };
                let deferred_subject =
                    self.defer_gc_ref_pointer(span, "pass_mir_pattern_str_subject", subject_ptr)?;
                let expected = self.codegen_string_literal_from_text(span, value)?;
                let Some(BasicValueEnum::PointerValue(expected_ptr)) = expected.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string literal",
                        at: span.into(),
                    });
                };
                let subject_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "pass_mir_pattern_str_subject_reload",
                    &deferred_subject,
                )?;
                let fn_val = self.declare_runtime_string_equals();
                let call = self.builder.build_call(
                    fn_val,
                    &[subject_ptr.into(), expected_ptr.into()],
                    "pass_mir_pattern_str_eq",
                )?;
                let raw_result = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return type",
                        at: span.into(),
                    });
                };
                Ok(self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "pass_mir_pattern_str_eq_bool",
                )?)
            }
            crate::mir::Pattern::BoolLit { value: expected } => {
                let value = subject
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern bool subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.context.bool_type().const_int(*expected as u64, false),
                    "pass_mir_pattern_bool_eq",
                )?)
            }
        }
    }

    fn codegen_mir_is_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimePatternTypeTestMetadata,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if metadata.target_ty != target_ty || metadata.descriptor.ty != target_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is metadata",
                at: span.into(),
            });
        }
        let metadata_subject_ty = self
            .cg_ty_of_mir_type(mir_types, metadata.subject_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject metadata",
                at: span.into(),
            })?;
        if !self.cg_ty_layout_equivalent(metadata_subject_ty, subject.ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject type drift",
                at: span.into(),
            });
        }
        match metadata.static_fold {
            crate::mir::RuntimeTypeStaticFold::AlwaysTrue => {
                return Ok(self.context.bool_type().const_int(1, false));
            }
            crate::mir::RuntimeTypeStaticFold::AlwaysFalse => {
                return Ok(self.context.bool_type().const_int(0, false));
            }
            crate::mir::RuntimeTypeStaticFold::Dynamic => {}
        }
        if !self.runtime_pattern_type_descriptor_is_codegen_supported(mir_types, metadata) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is runtime type descriptor",
                at: span.into(),
            });
        }
        let target_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        if !matches!(target_cg, CgTy::Ref | CgTy::String) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target runtime type",
                at: span.into(),
            });
        }

        let subject = match subject.ty {
            CgTy::Ref => subject,
            CgTy::String => self.coerce_value(span, subject, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR pattern is subject type",
                    at: span.into(),
                });
            }
        };
        let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject value",
                at: span.into(),
            });
        };
        self.codegen_ref_is_instance_of(span, subject_ptr, target_ty)
    }

    fn codegen_mir_tuple_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        elements: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern tuple type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject value",
                at: span.into(),
            });
        };
        let tuple_v = raw.into_struct_value();
        let (prefix_pats, has_rest) = match elements.last() {
            Some(crate::mir::Pattern::Rest) => {
                (&elements[..elements.len().saturating_sub(1)], true)
            }
            _ => (elements, false),
        };
        let pat_arity = prefix_pats.len();
        if (!has_rest && pat_arity != tuple_elems.len())
            || (has_rest && pat_arity > tuple_elems.len())
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern arity",
                at: span.into(),
            });
        }

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            let elem_ty = self.tuple_element_cg_ty(tuple_ty, idx).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple pattern element type",
                    at: span.into(),
                },
            )?;
            let elem_value = self.extract_mir_tuple_element_value(span, tuple_v, idx, elem_ty)?;
            let elem_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, elem_value, pat)?;
            cond = self
                .builder
                .build_and(cond, elem_cond, "pass_mir_tuple_pattern_and")?;
        }
        Ok(cond)
    }

    fn codegen_mir_variant_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        variant_name: &str,
        args: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Enum(enum_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject value",
                at: span.into(),
            });
        };
        let (repr, variant) = {
            let layout = self.cg_enum_layout(span, enum_ty)?;
            let repr = layout.repr;
            let variant = layout
                .variants
                .iter()
                .find(|variant| variant.name == variant_name)
                .cloned()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR unknown enum variant",
                    at: span.into(),
                })?;
            (repr, variant)
        };
        let (prefix_pats, has_rest) = match args.last() {
            Some(crate::mir::Pattern::Rest) => (&args[..args.len().saturating_sub(1)], true),
            _ => (args, false),
        };
        let expected_arity = variant.fields.len();
        let found_arity = prefix_pats.len();
        if (!has_rest && expected_arity != found_arity)
            || (has_rest && found_arity > expected_arity)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern arity",
                at: span.into(),
            });
        }

        let tag = self.extract_mir_enum_tag_value(span, enum_ty, repr, raw)?;
        let expected = tag.get_type().const_int(variant.tag, false);
        let tag_eq = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            expected,
            "pass_mir_variant_tag_eq",
        )?;
        if !prefix_pats
            .iter()
            .any(Self::mir_pattern_needs_payload_match)
        {
            return Ok(tag_eq);
        }

        let subject_ptr = self.create_entry_alloca(span, "pass_mir_variant_subject", subject.ty)?;
        let _ = self.store_local_value(span, subject_ptr, subject.ty, subject)?;
        let current_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = current_bb
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let payload_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_payload");
        let merge_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_merge");
        self.builder
            .build_conditional_branch(tag_eq, payload_bb, merge_bb)?;

        self.builder.position_at_end(payload_bb);
        let mut payload_cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            if !Self::mir_pattern_needs_payload_match(pat) {
                continue;
            }
            let extracted = self.extract_matched_when_variant_field_value(
                enum_ty,
                repr,
                &variant,
                idx,
                span,
                subject_ptr,
            )?;
            let field_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, extracted, pat)?;
            payload_cond =
                self.builder
                    .build_and(payload_cond, field_cond, "pass_mir_variant_payload_and")?;
        }

        let payload_tail =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR variant payload tail",
                    at: span.into(),
                })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "pass_mir_variant_match")?;
        let no_match = self.context.bool_type().const_int(0, false);
        phi.add_incoming(&[(&no_match, current_bb), (&payload_cond, payload_tail)]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_mir_pattern_extract(
        &mut self,
        span: crate::span::Span,
        subject: &crate::mir::Operand,
        path: &[crate::mir::PatternBindingStep],
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut current = self.codegen_mir_operand(span, subject, slots)?;
        for step in path {
            current = match step {
                crate::mir::PatternBindingStep::TupleIndex(index) => {
                    let CgTy::Tuple(tuple_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject type",
                            at: span.into(),
                        });
                    };
                    let Some(raw) = current.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject value",
                            at: span.into(),
                        });
                    };
                    let elem_ty = self.tuple_element_cg_ty(tuple_ty, *index).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple field type",
                            at: span.into(),
                        },
                    )?;
                    self.extract_mir_tuple_element_value(
                        span,
                        raw.into_struct_value(),
                        *index,
                        elem_ty,
                    )?
                }
                crate::mir::PatternBindingStep::VariantField {
                    variant,
                    field_index,
                } => {
                    let CgTy::Enum(enum_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract variant subject type",
                            at: span.into(),
                        });
                    };
                    let layout = self.cg_enum_layout(span, enum_ty)?;
                    let variant = layout
                        .variants
                        .iter()
                        .find(|item| item.name == *variant)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract unknown enum variant",
                            at: span.into(),
                        })?;
                    let subject_ptr =
                        self.create_entry_alloca(span, "pass_mir_extract_subject", current.ty)?;
                    let _ = self.store_local_value(span, subject_ptr, current.ty, current)?;
                    self.extract_matched_when_variant_field_value(
                        enum_ty,
                        layout.repr,
                        &variant,
                        *field_index,
                        span,
                        subject_ptr,
                    )?
                }
            };
        }
        self.coerce_value(span, current, target_cg)
    }

    fn extract_mir_tuple_element_value(
        &mut self,
        span: crate::span::Span,
        tuple_v: inkwell::values::StructValue<'ctx>,
        index: usize,
        elem_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let raw = self
            .builder
            .build_extract_value(tuple_v, index as u32, "pass_mir_tuple_elem")?;
        self.cg_value_from_loaded(span, elem_ty, raw)
    }

    fn extract_mir_enum_tag_value(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        repr: CgEnumRepr,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match repr {
            CgEnumRepr::TaggedUnion => Ok(self
                .builder
                .build_extract_value(raw.into_struct_value(), 0, "pass_mir_when_tag")?
                .into_int_value()),
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = raw.into_pointer_value();
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "pass MIR niche pointer none_value",
                                at: span.into(),
                            });
                        }
                        self.builder.build_is_null(ptr, "pass_mir_option_is_none")?
                    }
                    NicheStorage::U8 => {
                        let value = raw.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            value,
                            expected,
                            "pass_mir_option_is_none",
                        )?
                    }
                };
                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                Ok(self
                    .builder
                    .build_select(is_none, none_tag, some_tag, "pass_mir_option_tag")?
                    .into_int_value())
            }
            CgEnumRepr::ValueOnly { .. } => {
                let _ = enum_ty;
                Ok(raw.into_int_value())
            }
        }
    }

    fn mir_pattern_needs_payload_match(pattern: &crate::mir::Pattern) -> bool {
        !matches!(
            pattern,
            crate::mir::Pattern::Else
                | crate::mir::Pattern::Wildcard
                | crate::mir::Pattern::Rest
                | crate::mir::Pattern::Bind { .. }
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_call(
        &mut self,
        span: crate::span::Span,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        transport: &crate::mir::CallTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                if self.class_inits.contains_key(callee_fqn) {
                    return self.codegen_mir_class_ctor_call(span, callee_fqn, args, slots);
                }
                self.codegen_mir_direct_call(
                    span, callee_fqn, args, body, mir_types, transport, slots,
                )
            }
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure callee type",
                        at: span.into(),
                    })?;
                self.codegen_mir_closure_call(span, callee, fn_ptr, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunValue { callee } => {
                if let Some(fun_ty) = self.mir_operand_funptr_function_type(body, mir_types, callee)
                {
                    return self.codegen_mir_funptr_value_call(
                        span,
                        callee,
                        args,
                        &fun_ty,
                        !fun_ty.effects.is_pure(),
                        (body, mir_types, slots),
                    );
                }
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR function-value callee type",
                        at: span.into(),
                    })?;
                self.codegen_mir_fun_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call kind",
                at: span.into(),
            }),
        }
    }

    fn published_callable_uses_effect_step_surface(&self, callable_fqn: &str) -> bool {
        self.published_late_lowered_program()
            .and_then(|program| program.callable(callable_fqn))
            .is_some_and(|callable| callable.effect_step_abi().is_some())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_perform_terminator(
        &mut self,
        _span: crate::span::Span,
        op_fqn: &str,
        metadata: &crate::mir::PerformMetadata,
        _args: &[crate::mir::PerformArg],
        _unwind: &crate::mir::UnwindAction,
        _mir_ctx: MirBodyCodegenCtx<'_, 'ctx>,
    ) -> Result<(), LlvmEmitError> {
        Err(LlvmEmitError::Frontend {
            message: format!(
                "direct MIR perform terminator `{op_fqn}`（payload_tuple_ty={:?}）应先经 published late-lowered boundary lowering，而不是命中 plain/materialized MIR body codegen",
                metadata.payload_tuple_ty,
            ),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_direct_call_with_policy(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        transport: &crate::mir::CallTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_plain_surface: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut concrete_fqn = self.concrete_top_level_fun_call_fqn(span, fqn)?;
        if self.materialized_mir_callable(&concrete_fqn).is_none()
            && let Some(inferred_fqn) = self.inferred_materialized_direct_call_fqn(
                &concrete_fqn,
                args,
                transport.result.source_ty,
                body,
                mir_types,
            )
        {
            concrete_fqn = inferred_fqn;
        }
        let is_extern = self.extern_funs.contains_key(&concrete_fqn);
        let dispatch_fqn = mir_direct_call_base_fqn(&concrete_fqn);
        let uses_effect_step_surface = !is_extern
            && (self.published_callable_uses_effect_step_surface(&concrete_fqn)
                || self.published_callable_uses_effect_step_surface(dispatch_fqn));
        if require_plain_surface && uses_effect_step_surface {
            return Err(frontend_error(format!(
                "refactor plain direct call `{}` 仍要求 effect-step callable surface；应走 published boundary/dynamic adapter，而不是 ordinary direct call",
                concrete_fqn,
            )));
        }
        let uses_explicit_effect_hidden_abi = !require_plain_surface && uses_effect_step_surface;
        let materialized_sig = self
            .materialized_mir_callable(&concrete_fqn)
            .map(|(mir_types, fun)| (fun.clone(), mir_types as *const TypeStore));
        let hir_sig_fun = self
            .fun_index
            .get(&concrete_fqn)
            .copied()
            .or_else(|| self.hir_fun_for_callable_fqn(&concrete_fqn));
        if hir_sig_fun.is_none() && materialized_sig.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR direct callee type",
                at: span.into(),
            });
        }
        let map_foreign_signature_ty_to_codegen =
            |cg: &Self, ty: TypeId| cg.equivalent_codegen_type_id(mir_types, ty).unwrap_or(ty);
        let (param_names, param_tys, return_ty_for_codegen) =
            if let Some((fun, materialized_types)) = materialized_sig.as_ref() {
                // SAFETY: `materialized_types` points into the materialized pass view owned by the
                // compilation-unit codegen context and outlives this call.
                let materialized_types = unsafe { &**materialized_types };
                let param_names = fun
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let fallback_param_tys =
                    fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
                let fallback_return_ty = fun.return_ty;
                let needs_published_sig = fallback_param_tys
                    .iter()
                    .any(|&ty| self.cg_ty_of_mir_type(materialized_types, ty).is_none())
                    || self
                        .cg_ty_of_mir_type(materialized_types, fallback_return_ty)
                        .is_none();
                let published_sig = if needs_published_sig {
                    self.published_callable_signature(&concrete_fqn)
                        .or_else(|| {
                            (dispatch_fqn != concrete_fqn)
                                .then(|| self.published_callable_signature(dispatch_fqn))
                                .flatten()
                        })
                } else {
                    None
                };
                let (param_tys, return_ty, from_foreign_store) =
                    if let Some((param_tys, return_ty)) = published_sig {
                        (param_tys, return_ty, true)
                    } else {
                        (fallback_param_tys, fallback_return_ty, false)
                    };
                let (param_tys, return_ty) = if from_foreign_store {
                    (
                        param_tys
                            .into_iter()
                            .map(|ty| map_foreign_signature_ty_to_codegen(self, ty))
                            .collect::<Vec<_>>(),
                        map_foreign_signature_ty_to_codegen(self, return_ty),
                    )
                } else {
                    (param_tys, return_ty)
                };
                (param_names, param_tys, return_ty)
            } else {
                let fun = hir_sig_fun.expect("validated above");
                let param_names = fun
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let arg_to_param = map_mir_call_args_to_params(&fun.params, args).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR call arg binding",
                        at: span.into(),
                    },
                )?;
                let mut fallback_param_tys =
                    fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
                for (arg_idx, arg) in args.iter().enumerate() {
                    let param_idx = arg_to_param[arg_idx];
                    if self.cg_ty_of(fallback_param_tys[param_idx]).is_some() {
                        continue;
                    }
                    if let Some(source_ty) = self.mir_operand_type_id(body, &arg.value)
                        && let Some(codegen_ty) =
                            self.equivalent_codegen_type_id(mir_types, source_ty)
                    {
                        fallback_param_tys[param_idx] = codegen_ty;
                    }
                }
                let fallback_return_ty = fun.return_ty;
                let needs_published_sig = fallback_param_tys
                    .iter()
                    .any(|&ty| self.cg_ty_of(ty).is_none())
                    || self.cg_ty_of(fallback_return_ty).is_none();
                let published_sig = if needs_published_sig {
                    self.published_callable_signature(&concrete_fqn)
                        .or_else(|| {
                            (dispatch_fqn != concrete_fqn)
                                .then(|| self.published_callable_signature(dispatch_fqn))
                                .flatten()
                        })
                } else {
                    None
                };
                let (param_tys, return_ty, from_foreign_store) =
                    if let Some((param_tys, return_ty)) = published_sig {
                        (param_tys, return_ty, true)
                    } else {
                        (fallback_param_tys, fallback_return_ty, false)
                    };
                let (param_tys, return_ty) = if from_foreign_store {
                    (
                        param_tys
                            .into_iter()
                            .map(|ty| map_foreign_signature_ty_to_codegen(self, ty))
                            .collect::<Vec<_>>(),
                        map_foreign_signature_ty_to_codegen(self, return_ty),
                    )
                } else {
                    (param_tys, return_ty)
                };
                let param_tys = param_tys
                    .into_iter()
                    .map(|ty| {
                        if self.cg_ty_of(ty).is_some() {
                            ty
                        } else {
                            map_foreign_signature_ty_to_codegen(self, ty)
                        }
                    })
                    .collect::<Vec<_>>();
                let return_ty = if self.cg_ty_of(return_ty).is_some() {
                    return_ty
                } else {
                    map_foreign_signature_ty_to_codegen(self, return_ty)
                };
                (param_names, param_tys, return_ty)
            };
        if param_names.len() != param_tys.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR direct call signature arity mismatch",
                at: span.into(),
            });
        }
        if args.len() != param_tys.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR direct call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg = if let Some((fun, materialized_types)) = materialized_sig.as_ref() {
            // SAFETY: `materialized_types` points into the materialized pass view owned by the
            // compilation-unit codegen context and outlives this call.
            let materialized_types = unsafe { &**materialized_types };
            self.cg_ty_of_mir_type(materialized_types, fun.return_ty)
                .or_else(|| self.cg_ty_of(return_ty_for_codegen))
                .or_else(|| {
                    self.cg_ty_of_mir_type(mir_types, transport.result.source_ty)
                        .or_else(|| {
                            self.equivalent_codegen_type_id(mir_types, transport.result.source_ty)
                                .and_then(|ty| self.cg_ty_of(ty))
                        })
                })
        } else {
            self.cg_ty_of(return_ty_for_codegen).or_else(|| {
                self.cg_ty_of_mir_type(mir_types, transport.result.source_ty)
                    .or_else(|| {
                        self.equivalent_codegen_type_id(mir_types, transport.result.source_ty)
                            .and_then(|ty| self.cg_ty_of(ty))
                    })
            })
        }
        .ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR direct call return type",
            at: span.into(),
        })?;
        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(span, ret_cg)?
        };
        let evaluated_args = if let Some((fun, mir_types)) = materialized_sig.as_ref() {
            // SAFETY: `mir_types` points into the materialized pass view owned by the
            // compilation-unit codegen context and outlives this call.
            let mir_types = unsafe { &**mir_types };
            self.codegen_bound_materialized_mir_call_args(
                span, fun, mir_types, args, slots, is_extern,
            )?
        } else {
            self.codegen_bound_mir_call_args_from_signature(
                span,
                &param_names,
                &param_tys,
                args,
                slots,
                is_extern,
                self.types,
            )?
        };

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_direct_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            let slot = self.alloc_effect_outcome_slot(span, "pass_mir_direct_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let llvm_name = self
            .extern_funs
            .get(&concrete_fqn)
            .map(|extern_fun| extern_fun.symbol.as_str())
            .unwrap_or(concrete_fqn.as_str());
        let llvm_fun = match self.module.get_function(llvm_name) {
            Some(function) => function,
            None => {
                if let Some((fun, mir_types)) = materialized_sig.as_ref() {
                    // SAFETY: `mir_types` points into the materialized pass view owned by the
                    // compilation-unit codegen context and outlives this call.
                    let mir_types = unsafe { &**mir_types };
                    let param_tys = fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
                    let declaration_surface = if is_extern {
                        LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
                    } else {
                        LlvmFunctionDeclarationSurface::ExportedAbi
                    };
                    self.declare_materialized_mir_plain_fun_with_symbol(
                        llvm_name,
                        declaration_surface,
                        fun,
                        &param_tys,
                        fun.return_ty,
                        mir_types,
                    )?
                } else {
                    self.declare_top_level_fun_with_signature_override(
                        hir_sig_fun.expect("validated above"),
                        llvm_name,
                        &param_tys,
                        return_ty_for_codegen,
                    )?
                }
            }
        };
        let call_site_result = if is_extern {
            self.emit_extern_native_call(span, &concrete_fqn, llvm_fun, &llvm_args)
        } else {
            self.with_conservative_gc_local_root_spills(span, |cg| {
                let call_site =
                    cg.builder
                        .build_call(llvm_fun, &llvm_args, "pass_mir_direct_call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(&concrete_fqn));
                Ok(call_site)
            })
        };
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_direct_call_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_direct_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_direct_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_direct_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "pass_mir_direct_call_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR direct call deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    fn codegen_mir_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        class_layout_key: &str,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let site = self
            .ctor_call_sites
            .get(&self.current_call_site(span)?)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR class ctor call site",
                at: span.into(),
            })?;
        self.codegen_mir_refactor_class_ctor_call(
            span,
            class_layout_key,
            &crate::mir::ClassCtorCallMetadata {
                selected_ctor_span: site.ctor_span,
                ordered_param_count: args.len(),
            },
            args,
            slots,
        )
    }

    fn codegen_mir_closure_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        fn_ptr: &str,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let call_may_suspend = self
            .published_late_lowered_program()
            .and_then(|program| program.callable(fn_ptr))
            .map(|callable| callable.effect_step_abi().is_some())
            .unwrap_or(!fun_ty.effects.is_pure());
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure callee value",
                at: span.into(),
            });
        };
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            call_may_suspend,
            args,
            slots,
        )
    }

    fn codegen_mir_fun_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR function-value callee value",
                at: span.into(),
            });
        };
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            !fun_ty.effects.is_pure(),
            args,
            slots,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_funptr_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        call_may_suspend: bool,
        mir_ctx: (&crate::mir::Body, &TypeStore, &[MirLocalSlot<'ctx>]),
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (_body, mir_types, slots) = mir_ctx;
        let callee_value = self.codegen_mir_operand(span, callee, slots)?;
        let (funptr_addr, funptr_int_ty) =
            callee_value
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR FunPtr callee value",
                    at: span.into(),
                })?;
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr call return type",
                at: span.into(),
            })?;
        // `FunPtr<F>` follows the target's native function-pointer ABI instead of the
        // ordinary managed-call sret policy.
        let hidden_sret_result_ty = None;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            expected_arity
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.llvm_param_ty(span, receiver_ty)?);
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(span, *ty)?);
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let fun_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let casted_addr = if funptr_int_ty.bits == self.host.word_bit_width() {
            funptr_addr
        } else {
            self.cast_int(
                funptr_addr,
                funptr_int_ty,
                IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                },
            )?
        };
        let typed_fn_ptr =
            self.builder
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "pass_mir_funptr_typed")?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_funptr_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if call_may_suspend {
            let slot = self.alloc_effect_outcome_slot(span, "pass_mir_funptr_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let evaluated_args =
            self.codegen_mir_funptr_value_args(span, fun_ty, args, mir_types, slots)?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "pass_mir_call_funptr",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(0);
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_funptr_call_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_funptr_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_funptr_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_funptr_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "pass_mir_funptr_call_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR FunPtr deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        fun_ty: &crate::ty::FunctionType,
        call_may_suspend: bool,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR function-value call arity mismatch",
                at: span.into(),
            });
        }
        let deferred_closure =
            self.defer_gc_ref_pointer(span, "pass_mir_function_value_closure", closure_obj_i8)?;

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR function-value call return type",
                at: span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;

        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            1 + expected_arity
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        llvm_param_tys.push(gc_i8_ptr_ty.into());
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.ordinary_param_abi(span, receiver_ty)?.llvm_param_ty());
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.ordinary_param_abi(span, *ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
            (None, CgTy::String) => self
                .llvm_scoop_string_ptr_type()
                .fn_type(&llvm_param_tys, false),
            (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                "aggregate MIR function-value returns should have been lowered through hidden sret"
            ),
        };

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if call_may_suspend {
            let slot = self.alloc_effect_outcome_slot(span, "pass_mir_closure_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let evaluated_args = self.codegen_mir_callable_value_args(span, fun_ty, args, slots)?;

        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_call_obj_reload",
            &deferred_closure,
        )?;
        let closure_ptr = self.builder.build_pointer_cast(
            closure_obj_i8,
            closure_ptr_ty,
            "pass_mir_closure_obj_ptr",
        )?;
        let env_ptr_gep = self.builder.build_struct_gep(
            closure_ty,
            closure_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "pass_mir_closure_fn_gep")?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "pass_mir_closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "pass_mir_closure_fn")?
            .into_pointer_value();
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "pass_mir_closure_fn_typed",
        )?;
        llvm_args.push(env_ptr.into());
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "pass_mir_call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_closure_call_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_closure_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_closure_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_closure_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "pass_mir_closure_call_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR function-value deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    pub(super) fn codegen_mir_refactor_plain_dynamic_call(
        &mut self,
        span: crate::span::Span,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_plain_dynamic_call_with_policy(
            span, kind, args, body, mir_types, slots, true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_plain_dynamic_call_with_policy(
        &mut self,
        span: crate::span::Span,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        allow_effect_typed_dispatch_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain closure callee type",
                        at: span.into(),
                    })?;
                if !fun_ty.effects.is_pure() {
                    if self.plain_callable_carrier_fallback_allowed(
                        CallableCarrierKind::ClosureObject,
                        fn_ptr,
                    ) {
                        return self.codegen_mir_plain_function_value_call(
                            span, callee, args, &fun_ty, slots,
                        );
                    }
                    let Some(fun) = self.hir_fun_for_callable_fqn(fn_ptr) else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "refactor plain closure call effect-typed surface requires adapter",
                            at: span.into(),
                        });
                    };
                    if self.known_fun_body_may_outward_effect(fn_ptr, fun.ty) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "refactor plain closure call target may outward-effect",
                            at: span.into(),
                        });
                    }
                }
                self.codegen_mir_plain_function_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunValue { callee } => {
                if let Some(fun_ty) = self.mir_operand_funptr_function_type(body, mir_types, callee)
                {
                    if !fun_ty.effects.is_pure() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "refactor plain FunPtr call effect-typed surface requires adapter",
                            at: span.into(),
                        });
                    }
                    return self.codegen_mir_funptr_value_call(
                        span,
                        callee,
                        args,
                        &fun_ty,
                        false,
                        (body, mir_types, slots),
                    );
                }
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain function-value callee type",
                        at: span.into(),
                    })?;
                if !fun_ty.effects.is_pure() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain function-value call effect-typed surface requires adapter",
                        at: span.into(),
                    });
                }
                self.codegen_mir_plain_function_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::Virtual { receiver, dispatch } => {
                let target = self.resolve_plain_virtual_dispatch_target(dispatch, args.len())?;
                self.codegen_mir_plain_dispatch_call(
                    span,
                    receiver,
                    args,
                    slots,
                    target,
                    allow_effect_typed_dispatch_signature,
                )
            }
            crate::mir::CallKind::Interface { receiver, dispatch } => {
                let target = self.resolve_plain_interface_dispatch_target(dispatch, args.len())?;
                self.codegen_mir_plain_dispatch_call(
                    span,
                    receiver,
                    args,
                    slots,
                    target,
                    allow_effect_typed_dispatch_signature,
                )
            }
            crate::mir::CallKind::Direct { .. } | crate::mir::CallKind::Resume { .. } => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor plain dynamic call kind",
                    at: span.into(),
                })
            }
        }
    }

    pub(super) fn codegen_mir_plain_function_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain function-value callee value",
                at: span.into(),
            });
        };
        self.codegen_mir_plain_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            args,
            fun_ty,
            slots,
        )
    }

    pub(super) fn codegen_mir_plain_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let deferred_callee = self.defer_gc_ref_pointer(
            span,
            "refactor_plain_function_value_callee",
            closure_obj_i8,
        )?;
        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "refactor_plain_function_value_callee_reload",
            &deferred_callee,
        )?;
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            false,
            args,
            slots,
        )
    }

    fn resolve_plain_virtual_dispatch_target(
        &self,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget<'a>, LlvmEmitError> {
        let slots = self.class_vtables.get(&dispatch.owner_fqn).ok_or_else(|| {
            frontend_error(format!(
                "refactor plain virtual call 缺少 `{}` 的 class vtable metadata",
                dispatch.owner_fqn,
            ))
        })?;
        let mut candidates = slots.iter().filter(|slot| {
            slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
        });
        let slot = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor plain virtual call 缺少 `{}`.`{}`/{} 的 vtable slot",
                dispatch.owner_fqn, dispatch.member_name, explicit_arg_count,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "refactor plain virtual call `{}`.`{}`/{} 的 vtable slot 多义",
                dispatch.owner_fqn, dispatch.member_name, explicit_arg_count,
            )));
        }
        let sig_fun = self
            .fun_index
            .get(slot.impl_member_fqn.as_str())
            .copied()
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor plain virtual call 缺少 target `{}` 的 signature",
                    slot.impl_member_fqn,
                ))
            })?;
        Ok(PlainDispatchTarget::Virtual {
            slot: slot.slot,
            sig_fun,
        })
    }

    fn resolve_plain_interface_dispatch_target(
        &self,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget<'a>, LlvmEmitError> {
        let iface = self.interfaces.get(&dispatch.owner_fqn).ok_or_else(|| {
            frontend_error(format!(
                "refactor plain interface call 缺少 `{}` 的 interface metadata",
                dispatch.owner_fqn,
            ))
        })?;
        let mut slots = iface.method_slots.iter().filter(|slot| {
            slot.member_fqn == dispatch.member_fqn && slot.params_len == explicit_arg_count as u32
        });
        let slot = slots.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor plain interface call 缺少 `{}` 的 selected itable slot",
                dispatch.member_fqn,
            ))
        })?;
        if slots.next().is_some() {
            return Err(frontend_error(format!(
                "refactor plain interface call `{}` 的 selected itable slot 多义",
                dispatch.member_fqn,
            )));
        }

        let sig_fun = self
            .fun_index
            .get(dispatch.member_fqn.as_str())
            .copied()
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor plain interface call 缺少 `{}` 的 selected signature",
                    dispatch.member_fqn,
                ))
            })?;
        Ok(PlainDispatchTarget::Interface {
            interface_id: iface.interface_id,
            slot: slot.slot,
            sig_fun,
        })
    }

    fn codegen_mir_plain_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        target: PlainDispatchTarget<'a>,
        allow_effect_typed_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sig_fun = target.sig_fun();
        if sig_fun.params.len() != args.len() + 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain dispatch arity mismatch",
                at: span.into(),
            });
        }
        if !allow_effect_typed_signature
            && self.known_fun_body_may_outward_effect(&sig_fun.fqn, sig_fun.ty)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain dispatch target may outward-effect",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor plain dispatch return type",
                    at: span.into(),
                })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(sig_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for param in &sig_fun.params {
            llvm_param_tys.push(self.ordinary_param_abi(span, param.ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let mut all_args = Vec::with_capacity(args.len() + 1);
        all_args.push(crate::mir::CallArg {
            span,
            name: None,
            value: receiver.clone(),
        });
        all_args.extend(args.iter().cloned());
        let evaluated_args =
            self.codegen_bound_mir_call_args(span, sig_fun, &all_args, slots, false)?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain dispatch receiver value",
                at: span.into(),
            })?;
        let deferred_receiver = self.defer_gc_ref_pointer(
            span,
            &format!("{}_receiver", target.label().replace(' ', "_")),
            receiver_ptr,
        )?;
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(evaluated_args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if let Some(result_ty) = hidden_sret_result_ty {
            let slot = self.create_entry_alloca(span, "refactor_plain_dispatch_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "refactor_plain_dispatch_receiver_reload",
            &deferred_receiver,
        )?;
        let fn_i8 = match target {
            PlainDispatchTarget::Virtual { slot, .. } => {
                self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, slot)?
            }
            PlainDispatchTarget::Interface {
                interface_id, slot, ..
            } => {
                self.load_interface_itable_slot_fn_ptr_i8(span, receiver_ptr, interface_id, slot)?
            }
        };
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "refactor_plain_dispatch_fn_typed",
        )?;
        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "refactor_plain_dispatch_call",
            )?;
            if let Some((_, result_ty)) = sret_result_slot {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(&sig_fun.fqn));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some((result_ptr, _)) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "refactor_plain_dispatch_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(
                span,
                ret_cg,
                call_site,
                "refactor_plain_dispatch_direct_result",
            )?
        } else {
            None
        };
        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => Ok(if let Some((result_ptr, _)) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(
                    span,
                    ret_cg,
                    result_ptr,
                    "refactor_plain_dispatch_sret",
                )?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "refactor_plain_dispatch_direct_result_reload",
                    deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain dispatch deferred return value",
                        at: span.into(),
                    })?,
                )?
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        transport: &crate::mir::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span, fqn, args, transport, body, mir_types, slots, false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_refactor_plain_direct_call(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        transport: &crate::mir::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span, fqn, args, transport, body, mir_types, slots, true,
        )
    }
    fn selected_mir_class_ctor_from_contract<'b>(
        &self,
        span: crate::span::Span,
        class: &'b hir::ClassInit,
        ctor: &crate::mir::ClassCtorCallMetadata,
        args: &[crate::mir::CallArg],
        kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor>, LlvmEmitError> {
        if args.iter().any(|arg| arg.name.is_some()) || args.len() != ctor.ordered_param_count {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        let selected_ctor = match ctor.selected_ctor_span {
            Some(selected_span) => Some(
                class
                    .ctors
                    .iter()
                    .find(|candidate| candidate.span == selected_span)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor class ctor selected ctor contract",
                        at: span.into(),
                    })?,
            ),
            None if class.ctors.is_empty() => None,
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor class ctor selected ctor contract",
                    at: span.into(),
                });
            }
        };

        let param_count = selected_ctor.map_or(0, |ctor| ctor.params.len());
        if param_count != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        Ok(selected_ctor)
    }

    fn codegen_mir_refactor_class_ctor_ordered_args(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        ctor_params: &[hir::ClassCtorParam],
        kind: &'static str,
    ) -> Result<Vec<CgValue<'ctx>>, LlvmEmitError> {
        if ctor_params.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        let mut evaluated_args = Vec::with_capacity(args.len());
        for (idx, (param, arg)) in ctor_params.iter().zip(args).enumerate() {
            if arg.name.is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: arg.span.into(),
                });
            }
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor class ctor param type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(param_cg))?;
            let value = self.coerce_value(arg.span, value, param_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("refactor_class_ctor_ordered_arg_{idx}"),
                value,
            )?;
            evaluated_args.push(self.materialize_deferred_cg_value(
                arg.span,
                &format!("refactor_class_ctor_ordered_arg_reload_{idx}"),
                deferred,
            )?);
        }

        Ok(evaluated_args)
    }

    pub(super) fn codegen_mir_refactor_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        class_layout_key: &str,
        ctor: &crate::mir::ClassCtorCallMetadata,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let class = self.class_init_layout(span, class_layout_key)?;
        let selected_ctor = self.selected_mir_class_ctor_from_contract(
            span,
            &class,
            ctor,
            args,
            "refactor class ctor selected/ordered args contract",
        )?;
        let ctor_params: &[hir::ClassCtorParam] = match selected_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };

        let obj_ty = self.llvm_class_object_type(span, &class)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let type_desc = self.get_or_create_class_type_desc_global(span, class_layout_key)?;
        let type_desc_i8 = self.builder.build_pointer_cast(
            type_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "refactor_class_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[type_desc_i8.into(), size_v.into()],
            "rt_alloc_refactor_class",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let typed_obj =
            self.builder
                .build_pointer_cast(obj_ptr, obj_ptr_ty, "refactor_class_obj_ptr")?;
        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "refactor_class_payload_gep")?;
        let payload_ty = self.llvm_class_payload_type(span, &class)?;
        let payload_size_bytes = self.target_data.get_store_size(&payload_ty);
        if payload_size_bytes > 0 {
            let payload_i8 = self
                .builder
                .build_bit_cast(
                    payload_ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "refactor_class_payload_i8",
                )?
                .into_pointer_value();
            let size_ty = self.llvm_ptr_sized_int_type(None);
            let size_v = size_ty.const_int(payload_size_bytes, false);
            let zero = self.context.i8_type().const_int(0, false);
            let _ = self.builder.build_memset(payload_i8, 1, zero, size_v)?;
        }

        let deferred_obj = self.defer_gc_sensitive_cg_value(
            span,
            "refactor_class_ctor_obj_root",
            CgValue {
                ty: CgTy::Ref,
                value: Some(obj_ptr.into()),
            },
        )?;

        let evaluated_args = self.codegen_mir_refactor_class_ctor_ordered_args(
            span,
            args,
            slots,
            ctor_params,
            "refactor class ctor ordered arg eval",
        )?;

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "refactor_class_ctor_obj_before_invoke",
            &deferred_obj,
        )?;

        self.codegen_class_ctor_invoke(
            span,
            span,
            &class,
            selected_ctor,
            evaluated_args.as_slice(),
            current_obj,
        )?;
        self.emit_ordinary_call_effect_propagation_check(span, "refactor_class_ctor_call_effect")?;

        if !self.ordinary_effect_propagation_enabled()
            && let Some(outcome_ptr) = self.function_cx.current_effect_outcome_ptr
        {
            let current_fn = self
                .builder
                .get_insert_block()
                .and_then(|bb| bb.get_parent())
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor class ctor current function",
                    at: span.into(),
                })?;
            let active_bb = self
                .context
                .append_basic_block(current_fn, "refactor_class_ctor_active");
            let inactive_bb = self
                .context
                .append_basic_block(current_fn, "refactor_class_ctor_inactive");
            let merge_bb = self
                .context
                .append_basic_block(current_fn, "refactor_class_ctor_merge");
            let is_propagating = self.effect_outcome_is_propagating(
                span,
                outcome_ptr,
                "refactor_class_ctor_effect",
            )?;
            self.builder
                .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

            self.builder.position_at_end(active_bb);
            self.clear_deferred_cg_value_root_homes(
                span,
                "refactor_class_ctor_obj_active_drop",
                &deferred_obj,
            )?;
            let active_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor class ctor active block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(inactive_bb);
            let current_obj = self.reload_deferred_gc_ref_without_clearing(
                span,
                "refactor_class_ctor_obj_return",
                &deferred_obj,
            )?;
            let inactive_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor class ctor inactive block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(merge_bb);
            let result_phi = self
                .builder
                .build_phi(self.llvm_gc_i8_ptr_type(), "refactor_class_ctor_result")?;
            result_phi.add_incoming(&[
                (&self.llvm_gc_i8_ptr_type().const_null(), active_bb_end),
                (&current_obj, inactive_bb_end),
            ]);
            return Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(result_phi.as_basic_value()),
            });
        }

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "refactor_class_ctor_obj_return",
            &deferred_obj,
        )?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(current_obj.into()),
        })
    }

    fn codegen_bound_mir_call_args(
        &mut self,
        span: crate::span::Span,
        sig_fun: &hir::FunDecl,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        is_extern: bool,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let param_tys = sig_fun
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        self.codegen_bound_mir_call_args_from_signature(
            span,
            &param_names,
            &param_tys,
            args,
            slots,
            is_extern,
            self.types,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_bound_mir_call_args_from_signature(
        &mut self,
        span: crate::span::Span,
        param_names: &[String],
        param_tys: &[TypeId],
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        is_extern: bool,
        source_types: &TypeStore,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let arg_to_param = map_mir_call_args_to_param_names(param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let param_ty = param_tys[param_idx];
            let target_cg = self
                .cg_ty_of_mir_type(source_types, param_ty)
                .or_else(|| {
                    self.equivalent_codegen_type_id(source_types, param_ty)
                        .and_then(|ty| self.cg_ty_of(ty))
                })
                .or_else(|| self.cg_ty_of(param_ty))
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_call_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg binding",
                    at: span.into(),
                })?;
                let param_ty = param_tys[param_idx];
                let abi_ty = self
                    .equivalent_codegen_type_id(source_types, param_ty)
                    .unwrap_or(param_ty);
                let param_abi = if is_extern {
                    None
                } else {
                    Some(self.ordinary_param_abi(span, abi_ty)?)
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(arg_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn codegen_bound_materialized_mir_call_args(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        is_extern: bool,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let arg_to_param = map_mir_call_args_to_mir_params(&mir_fun.params, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; mir_fun.params.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let param = &mir_fun.params[param_idx];
            let target_cg = self.cg_ty_of_mir_type(mir_types, param.ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg type",
                    at: arg.span.into(),
                },
            )?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_call_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg binding",
                    at: span.into(),
                })?;
                let param = &mir_fun.params[param_idx];
                let abi_ty = self.equivalent_codegen_type_id(mir_types, param.ty).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR plain param type",
                        at: param.span.into(),
                    },
                )?;
                let param_abi = if is_extern {
                    None
                } else {
                    Some(self.ordinary_param_abi(param.span, abi_ty)?)
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(arg_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn mir_local_type_id(
        &self,
        body: &crate::mir::Body,
        local: crate::mir::LocalId,
    ) -> Option<TypeId> {
        body.locals
            .get(local.as_u32() as usize)
            .map(|local| local.ty)
    }

    fn mir_operand_type_id(
        &self,
        body: &crate::mir::Body,
        operand: &crate::mir::Operand,
    ) -> Option<TypeId> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_type_id(body, *local),
            crate::mir::Operand::Const(value) => Some(match value {
                crate::mir::ConstValue::Bool(_) => self.builtins.bool_,
                crate::mir::ConstValue::Char => self.builtins.char_,
                crate::mir::ConstValue::Unit => self.builtins.unit,
                crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                    self.builtins.int
                }
                crate::mir::ConstValue::Float64 => self.builtins.float64,
                crate::mir::ConstValue::Float32 => self.builtins.float32,
                crate::mir::ConstValue::String => self.builtins.string,
            }),
        }
    }

    fn mir_operand_function_type(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                self.equivalent_codegen_function_type(mir_types, fun_ty)
            }
            _ => None,
        }
    }

    fn mir_operand_funptr_function_type(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = mir_types.kind(ty) else {
            return None;
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" || nominal.args.len() != 1 {
            return None;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = mir_types.kind(nominal.args[0]) else {
            return None;
        };
        self.equivalent_codegen_function_type(mir_types, fun_ty)
    }

    fn mir_closure_env_capture_element_cg_tys(&self, env_cg: CgTy) -> Option<Vec<CgTy>> {
        match env_cg {
            CgTy::Unit => Some(Vec::new()),
            CgTy::Tuple(tuple_ty) => {
                let tuple_types = self.codegen_type_store_for_type_id(tuple_ty)?;
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) = tuple_types.kind(tuple_ty)
                else {
                    return None;
                };
                let elements = elements.clone();
                let mut out = Vec::with_capacity(elements.len());
                for elem_ty in elements {
                    let cg = if std::ptr::eq(tuple_types, self.types) {
                        self.cg_ty_of(elem_ty)
                    } else {
                        self.cg_ty_of_mir_type(tuple_types, elem_ty)
                    }?;
                    if !Self::mir_closure_env_capture_cg_is_supported(cg) {
                        return None;
                    }
                    out.push(cg);
                }
                Some(out)
            }
            _ => None,
        }
    }

    fn mir_closure_env_capture_cg_is_supported(cg_ty: CgTy) -> bool {
        matches!(
            cg_ty,
            CgTy::Unit
                | CgTy::Bool
                | CgTy::Float64
                | CgTy::Float32
                | CgTy::Int(_)
                | CgTy::String
                | CgTy::Ref
                | CgTy::Tuple(_)
                | CgTy::Struct(_)
                | CgTy::Enum(_)
        )
    }

    fn mir_closure_env_capture_element_cg_tys_from_contract(
        &mut self,
        span: crate::span::Span,
        body_fqn: &str,
        mir_types: &TypeStore,
        env_cg: CgTy,
        contract: &crate::mir::ClosureEnvTransportMetadata,
    ) -> Result<Vec<CgTy>, LlvmEmitError> {
        let contract_env_cg = self.cg_ty_of_mir_type(mir_types, contract.env_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env contract codegen type",
                at: span.into(),
            },
        )?;
        if contract_env_cg != env_cg {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env contract mismatch",
                at: span.into(),
            });
        }

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys(env_cg).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env shape",
                at: span.into(),
            },
        )?;
        if capture_field_cgs.len() != contract.captures.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env capture schema arity",
                at: span.into(),
            });
        }

        let env_transport = crate::mir::ValueTransportMetadata {
            source_ty: contract.env_ty,
            kind: crate::mir::MirTransportKind::ClosureEnv,
            requirements: self
                .composite_transport_requirements_for_type(mir_types, contract.env_ty),
            boxing: None,
        };
        self.get_or_create_value_composite_transport_descriptor_global(
            body_fqn,
            span,
            mir_types,
            &env_transport,
        )?;

        let env_element_tys = match mir_types.kind(contract.env_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => &[][..],
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.as_slice(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env contract shape",
                    at: span.into(),
                });
            }
        };

        for (index, capture) in contract.captures.iter().enumerate() {
            let env_element_ty =
                env_element_tys
                    .get(index)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env capture schema element",
                        at: capture.decl_span.into(),
                    })?;
            if mir_types.display(capture.transport.source_ty).to_string()
                != mir_types.display(env_element_ty).to_string()
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env capture schema type",
                    at: capture.decl_span.into(),
                });
            }
            if capture.mutable
                && (capture.transport.kind != crate::mir::MirTransportKind::CaptureBox
                    || self
                        .mir_capture_box_inner_type_id(mir_types, capture.transport.source_ty)
                        .is_none())
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure mutable capture box contract",
                    at: capture.decl_span.into(),
                });
            }
            self.get_or_create_value_composite_transport_descriptor_global(
                body_fqn,
                capture.decl_span,
                mir_types,
                &capture.transport,
            )?;
        }

        Ok(capture_field_cgs)
    }

    fn mir_capture_box_inner_type_id(
        &self,
        mir_types: &TypeStore,
        box_ty: TypeId,
    ) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = mir_types.kind(box_ty) else {
            return None;
        };
        if nominal.fqn != MIR_CAPTURE_BOX_FQN || nominal.args.len() != 1 || nominal.eff.is_some() {
            return None;
        }
        self.equivalent_codegen_type_id(mir_types, nominal.args[0])
    }

    fn codegen_mir_make_tuple(
        &mut self,
        span: crate::span::Span,
        _body: &crate::mir::Body,
        mir_types: &TypeStore,
        elements: &[crate::mir::Operand],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple target type",
                at: span.into(),
            });
        };
        let (element_tys, use_primary_types) = {
            let tuple_types = self
                .codegen_type_store_for_type_id(tuple_ty)
                .unwrap_or(mir_types);
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = tuple_types.kind(tuple_ty)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple type",
                    at: span.into(),
                });
            };
            (element_tys.clone(), std::ptr::eq(tuple_types, self.types))
        };
        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (operand, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = if use_primary_types {
                self.cg_ty_of(*elem_ty)
            } else {
                self.cg_ty_of_mir_type(mir_types, *elem_ty)
            }
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple element type",
                at: span.into(),
            })?;
            let value = self.codegen_mir_operand_expected(span, operand, slots, Some(elem_cg))?;
            let coerced = self.coerce_value(span, value, elem_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                span,
                &format!("pass_mir_tuple_elem_{idx}"),
                coerced,
            )?;
            deferred_elements.push((idx, span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("pass_mir_tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR tuple element value",
                        at: elem_span.into(),
                    })?,
            };
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, "pass_mir_tuple_insert")?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_mir_size_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR sizeOf arg type",
                at: span.into(),
            },
        )?;
        let llvm_ty = self.llvm_basic_type_of(span, arg_cg)?;
        let bytes = self.store_size_bytes_of_basic_type(llvm_ty);
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(bytes, false);
        Ok(CgValue::int(raw, value_word))
    }

    fn codegen_mir_make_struct(
        &mut self,
        span: crate::span::Span,
        fields: &[crate::mir::StructLitField],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal target type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal type",
                at: span.into(),
            });
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR struct literal layout",
                    at: span.into(),
                })?;
        if layout.fields.len() != fields.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal field count",
                at: span.into(),
            });
        }

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut deferred_fields: Vec<(u32, String, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());

        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let mut matches = fields
                .iter()
                .filter(|field| field.name == layout_field.name);
            let Some(init) = matches.next() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR struct literal missing field",
                    at: span.into(),
                });
            };
            if matches.next().is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR struct literal duplicate field",
                    at: init.span.into(),
                });
            }

            let field_cg = self.cg_ty_of_layout_field(
                init.span,
                layout_field.ty,
                layout_field.ty_fqn.as_deref(),
            )?;
            let value =
                self.codegen_mir_operand_expected(init.span, &init.value, slots, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if value.ty != field_cg {
                self.coerce_value(init.span, value, field_cg)?
            } else {
                value
            };
            let deferred = self.defer_gc_sensitive_cg_value(
                init.span,
                &format!("pass_mir_struct_field_{idx}"),
                coerced,
            )?;
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, layout_field.name.clone(), init.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, field_span, deferred)) in
            deferred_fields.into_iter().enumerate()
        {
            let materialized = self.materialize_deferred_cg_value(
                field_span,
                &format!("pass_mir_struct_field_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR struct literal field value",
                        at: field_span.into(),
                    })?,
            };
            agg = self.builder.build_insert_value(
                agg,
                raw,
                llvm_idx,
                &format!("pass_mir_struct_insert_{field_name}"),
            )?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_mir_tuple_get(
        &mut self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let tuple_ty =
            self.mir_operand_type_id(body, tuple)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple operand type",
                    at: span.into(),
                })?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand type",
                at: span.into(),
            });
        };
        let elem_ty = *elements
            .get(index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple index",
                at: span.into(),
            })?;
        let elem_cg = self.cg_ty_of_mir_type(mir_types, elem_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple element type",
                at: span.into(),
            },
        )?;
        let tuple_cg = self.mir_operand_cg_ty(body, mir_types, tuple).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand cg type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_mir_operand_expected(span, tuple, slots, Some(tuple_cg))?;
        let tuple_v = value
            .value
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand value",
                at: span.into(),
            })?
            .into_struct_value();
        self.extract_mir_tuple_element_value(span, tuple_v, index, elem_cg)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_make_closure(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            mir_types,
            env_cg,
            target_cg,
            slots,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_mir_make_closure_with_target_fn_ptr(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            mir_types,
            env_cg,
            target_cg,
            slots,
            Some(target_fn_ptr),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mir_make_closure_impl(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure target type",
                at: span.into(),
            });
        }

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys_from_contract(
            span,
            fn_ptr,
            mir_types,
            env_cg,
            env_contract,
        )?;

        let deferred_env = if capture_field_cgs.is_empty() {
            None
        } else {
            let value = self.codegen_mir_operand_expected(span, env, slots, Some(env_cg))?;
            let coerced = self.coerce_value(span, value, env_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "pass_mir_closure_env", coerced)?)
        };

        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let closure_desc = self.get_or_create_closure_object_type_desc_global(span)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "pass_mir_closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_pass_mir_closure",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "pass_mir_closure_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "pass_mir_closure_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_init",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        let env_i8 = if capture_field_cgs.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
            let closure_key = self.stable_closure_key_for_materialized_callable(fn_ptr, span)?;
            let env_ty =
                self.mir_closure_env_object_type(span, &closure_key, &capture_field_cgs)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);
            let env_size_v = self.context.i64_type().const_int(env_size_bytes, false);
            let env_desc =
                self.get_or_create_mir_closure_env_type_desc_global(span, &closure_key, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "pass_mir_closure_env_desc_i8",
            )?;
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[env_desc_i8.into(), env_size_v.into()],
                "rt_alloc_pass_mir_closure_env",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type",
                    at: span.into(),
                });
            };

            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr =
                self.builder
                    .build_pointer_cast(env_i8, env_ptr_ty, "pass_mir_closure_env_ptr")?;
            let deferred_env_obj =
                self.defer_gc_ref_pointer(span, "pass_mir_closure_env_root", env_ptr)?;
            let env_value = self.materialize_deferred_cg_value(
                span,
                "pass_mir_closure_env_reload",
                deferred_env.expect("non-empty env must have been deferred"),
            )?;
            let tuple_v = env_value
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env value",
                    at: span.into(),
                })?
                .into_struct_value();
            for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                let env_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "pass_mir_closure_env_field_reload",
                    &deferred_env_obj,
                )?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    "pass_mir_closure_env_field_gep",
                )?;
                let field_value =
                    self.extract_mir_tuple_element_value(span, tuple_v, idx, *field_cg)?;
                let _ = self.store_local_value(span, field_gep, *field_cg, field_value)?;
            }
            env_i8
        };
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_env",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

        let use_plain_fallback = self
            .plain_callable_carrier_fallback_allowed(CallableCarrierKind::ClosureObject, fn_ptr);
        let fallback_target = if target_fn_ptr.is_some() {
            self.llvm_i8_ptr_type().const_null()
        } else if self.callable_carrier_contract_enabled() && !use_plain_fallback {
            // Refactor callable carriers publish their own dynamic entry shell; do
            // not define a fallback lambda body just to obtain a fallback pointer.
            self.llvm_i8_ptr_type().const_null()
        } else if let Some(plain_entry) = self
            .module
            .get_function(&self.materialized_mir_closure_body_symbol(fn_ptr, span)?)
        {
            plain_entry.as_global_value().as_pointer_value()
        } else {
            self.ensure_materialized_mir_closure_callable_defined(span, fn_ptr)?
                .as_global_value()
                .as_pointer_value()
        };
        let fn_ptr = match target_fn_ptr {
            Some(ptr) => ptr,
            None => self.callable_carrier_target_fn_ptr(
                CallableCarrierKind::ClosureObject,
                fn_ptr,
                fallback_target,
            )?,
        };
        let fn_i8 = self
            .builder
            .build_pointer_cast(fn_ptr, i8_ptr_ty, "pass_mir_closure_fn_i8")?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_fn",
            &deferred_obj,
        )?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 2, "pass_mir_closure_fn_gep")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_return",
            &deferred_obj,
        )?;
        let obj_i8 =
            self.builder
                .build_pointer_cast(obj_ptr, gc_i8_ptr_ty, "pass_mir_closure_obj_i8")?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn codegen_mir_capture_box_new(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box target type",
                at: span.into(),
            });
        }

        let value_ty = self
            .mir_operand_type_id(body, value)
            .and_then(|ty| self.equivalent_codegen_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;

        let deferred_value = if value_cg == CgTy::Unit {
            None
        } else {
            let value = self.codegen_mir_operand_expected(span, value, slots, Some(value_cg))?;
            let coerced = self.coerce_value(span, value, value_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "pass_mir_capture_box_value", coerced)?)
        };

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_capture_box_type_desc_global(span, value_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "pass_mir_capture_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            "rt_alloc_pass_mir_capture_box",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "pass_mir_capture_box_obj_ptr")?;
        let deferred_obj =
            self.defer_gc_ref_pointer(span, "pass_mir_capture_box_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_capture_box_obj_reload",
            &deferred_obj,
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_field_gep",
        )?;
        let stored_value = deferred_value
            .map(|value| {
                self.materialize_deferred_cg_value(span, "pass_mir_capture_box_reload", value)
            })
            .transpose()?
            .unwrap_or_else(CgValue::unit);
        let _ = self.store_local_value(span, field_gep, value_cg, stored_value)?;
        let obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_capture_box_return",
            &deferred_obj,
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn codegen_mir_capture_box_get(
        &mut self,
        span: crate::span::Span,
        box_operand: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_ty = self
            .mir_operand_type_id(body, box_operand)
            .and_then(|ty| self.mir_capture_box_inner_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let box_value =
            self.codegen_mir_operand_expected(span, box_operand, slots, Some(CgTy::Ref))?;
        let box_value = self.coerce_value(span, box_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(box_obj_i8)) = box_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand value",
                at: span.into(),
            });
        };

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self.builder.build_pointer_cast(
            box_obj_i8,
            obj_ptr_ty,
            "pass_mir_capture_box_get_obj_ptr",
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_get_field_gep",
        )?;
        let loaded = if value_cg == CgTy::Unit {
            CgValue::unit()
        } else {
            let llvm_value_ty = self.llvm_basic_type_of(span, value_cg)?;
            let raw =
                self.builder
                    .build_load(llvm_value_ty, field_gep, "pass_mir_capture_box_get")?;
            self.cg_value_from_loaded(span, value_cg, raw)?
        };
        self.coerce_value(span, loaded, target_cg)
    }

    fn codegen_mir_capture_box_set(
        &mut self,
        span: crate::span::Span,
        box_operand: &crate::mir::Operand,
        value: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_ty = self
            .mir_operand_type_id(body, box_operand)
            .and_then(|ty| self.mir_capture_box_inner_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let box_value =
            self.codegen_mir_operand_expected(span, box_operand, slots, Some(CgTy::Ref))?;
        let box_value = self.coerce_value(span, box_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(box_obj_i8)) = box_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand value",
                at: span.into(),
            });
        };
        let value = self.codegen_mir_operand_expected(span, value, slots, Some(value_cg))?;
        let value = self.coerce_value(span, value, value_cg)?;

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self.builder.build_pointer_cast(
            box_obj_i8,
            obj_ptr_ty,
            "pass_mir_capture_box_set_obj_ptr",
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_set_field_gep",
        )?;
        let _ = self.store_local_value(span, field_gep, value_cg, value)?;
        Ok(CgValue::unit())
    }

    pub(super) fn codegen_mir_funptr_invoke_call(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some((receiver_arg, call_args)) = args.split_first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke arity mismatch",
                at: span.into(),
            });
        };
        if receiver_arg.name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke receiver binding",
                at: receiver_arg.span.into(),
            });
        }
        let fun_ty = self
            .mir_operand_funptr_function_type(body, mir_types, &receiver_arg.value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke receiver type",
                at: receiver_arg.span.into(),
            })?;
        self.codegen_mir_funptr_value_call(
            span,
            &receiver_arg.value,
            call_args,
            &fun_ty,
            !fun_ty.effects.is_pure(),
            (body, mir_types, slots),
        )
    }
    fn codegen_mir_callable_value_args(
        &mut self,
        span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        let arg_to_param = map_mir_call_args_to_param_names(&param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let target_cg =
                self.cg_ty_of(param_tys[param_idx])
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure call arg type",
                        at: arg.span.into(),
                    })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_closure_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure call arg binding",
                    at: span.into(),
                })?;
                let param_ty = param_tys[param_idx];
                let param_abi = self.ordinary_param_abi(span, param_ty)?;
                if param_abi.pointee_ty().is_some() {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let value = self.as_llvm_arg_value(arg_span, param_abi.cg_ty(), materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn codegen_mir_funptr_value_args(
        &mut self,
        span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[crate::mir::CallArg],
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        let arg_to_param = map_mir_call_args_to_param_names(&param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let target_cg = self
                .cg_ty_of_mir_type(mir_types, param_tys[param_idx])
                .or_else(|| self.cg_ty_of(param_tys[param_idx]))
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR FunPtr call arg type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_funptr_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR FunPtr call arg binding",
                    at: span.into(),
                })?;
                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_funptr_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let value = self.as_llvm_arg_value(arg_span, materialized.ty, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn mir_closure_env_object_type(
        &mut self,
        at: crate::span::Span,
        closure_key: &StableClosureKey,
        field_cgs: &[CgTy],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = private_closure_env_type_name(closure_key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + field_cgs.len());
        fields.push(self.llvm_gc_object_header_type().into());
        for cg in field_cgs {
            fields.push(self.llvm_basic_type_of(at, *cg)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    fn mir_capture_box_object_type(
        &mut self,
        at: crate::span::Span,
        value_ty: TypeId,
        value_cg: CgTy,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(value_ty, "MIR capture box LLVM type")?,
        );
        let name = PrivateSymbolMangler.type_name("MirCaptureBox", "mir_capture_box_type", &key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let box_ty = self.context.opaque_struct_type(&name);
        let fields = [
            self.llvm_gc_object_header_type().into(),
            self.llvm_basic_type_of(at, value_cg)?,
        ];
        box_ty.set_body(&fields, false);
        Ok(box_ty)
    }

    pub(in crate::llvm::codegen) fn mir_value_box_object_type(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        source_cg: CgTy,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(source_ty, "MIR value box LLVM type")?,
        );
        let name = PrivateSymbolMangler.type_name("MirValueBox", "mir_value_box_type", &key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let box_ty = self.context.opaque_struct_type(&name);
        let fields = [
            self.llvm_gc_object_header_type().into(),
            self.llvm_basic_type_of(at, source_cg)?,
        ];
        box_ty.set_body(&fields, false);
        Ok(box_ty)
    }

    fn get_or_create_mir_closure_env_type_desc_global(
        &mut self,
        at: crate::span::Span,
        closure_key: &StableClosureKey,
        env_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = private_closure_env_type_desc_name(closure_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&env_ty, 1).unwrap_or(0);
        let canonical_name = closure_key.env_canonical_name();
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: env_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn get_or_create_mir_capture_box_type_desc_global(
        &mut self,
        at: crate::span::Span,
        value_ty: TypeId,
        box_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let types = self
            .codegen_type_store_for_type_id(value_ty)
            .ok_or_else(|| {
                frontend_error(
                    "MIR capture box type descriptor 缺少 codegen type store".to_string(),
                )
            })?;
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(value_ty, "MIR capture box type descriptor")?,
        );
        let global_name = PrivateSymbolMangler.mangle("mir_capture_box_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&box_ty, 1).unwrap_or(0);
        let value_name = sanitize_llvm_ident(&types.display(value_ty).to_string());
        let canonical_name = format!("__scoop_type_desc_mir_capture_box__{value_name}");
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: box_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(in crate::llvm::codegen) fn get_or_create_mir_value_box_type_desc_global(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        box_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let types = self
            .codegen_type_store_for_type_id(source_ty)
            .ok_or_else(|| {
                frontend_error("MIR value box type descriptor 缺少 codegen type store".to_string())
            })?;
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(source_ty, "MIR value box type descriptor")?,
        );
        let global_name = PrivateSymbolMangler.mangle("mir_value_box_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&box_ty, 1).unwrap_or(0);
        let canonical_name = format!("scoop.runtime.ValueBox<{}>", types.display(source_ty));
        let itable = self
            .get_or_create_mir_value_box_itable_global(at, source_ty)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: box_ty,
            trace_start_offset_bytes,
            parent: None,
            itable,
            vtable: None,
        })
    }

    fn get_or_create_mir_value_box_itable_global(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(source_ty) else {
            return Ok(None);
        };
        if !self
            .struct_layouts
            .contains_key(&self.nominal_layout_key(nominal))
        {
            return Ok(None);
        }
        let entries = self.mir_value_box_itable_entries(&nominal.fqn)?;
        if entries.is_empty() {
            return Ok(None);
        }
        let owner_key = CanonicalTextKey::new(canonical_record(
            "mir_value_box_itable_owner",
            [self.canonical_type_key_text_for_codegen(source_ty, "MIR value box itable owner")?],
        ));
        self.get_or_create_itable_global_from_entries(at, &owner_key, &entries)
    }

    fn mir_value_box_itable_entries(
        &self,
        source_fqn: &str,
    ) -> Result<Vec<crate::itable::ClassItableEntry>, LlvmEmitError> {
        let mut interfaces = Vec::new();
        let mut visiting = HashSet::new();
        self.collect_mir_value_box_interfaces(source_fqn, &mut interfaces, &mut visiting);
        interfaces
            .into_iter()
            .map(|interface_fqn| {
                let iface = self.interfaces.get(&interface_fqn).ok_or_else(|| {
                    frontend_error(format!(
                        "value box interface `{interface_fqn}` missing interface metadata"
                    ))
                })?;
                let mut method_impl_fqns = Vec::with_capacity(iface.method_slots.len());
                for slot in &iface.method_slots {
                    let impl_fqn = format!("{source_fqn}.{}", slot.name);
                    if self.fun_index.contains_key(impl_fqn.as_str()) {
                        method_impl_fqns.push(impl_fqn);
                    } else if slot.has_body {
                        method_impl_fqns.push(slot.member_fqn.clone());
                    } else {
                        return Err(frontend_error(format!(
                            "value box `{source_fqn}` missing implementation for interface method `{}`",
                            slot.member_fqn
                        )));
                    }
                }
                let interface_type_name = iface.fqn.clone();
                let interface_type_id = stable_rtti_type_id(&interface_type_name);
                Ok(crate::itable::ClassItableEntry {
                    interface_fqn: iface.fqn.clone(),
                    interface_id: iface.interface_id,
                    interface_type_name: interface_type_name.clone(),
                    interface_type_id,
                    runtime_match_type_names: vec![interface_type_name],
                    runtime_match_type_ids: vec![interface_type_id],
                    method_impl_fqns,
                })
            })
            .collect()
    }

    fn collect_mir_value_box_interfaces(
        &self,
        fqn: &str,
        out: &mut Vec<String>,
        visiting: &mut HashSet<String>,
    ) {
        if !visiting.insert(fqn.to_string()) {
            return;
        }
        if let Some(supertypes) = self.direct_supertypes.get(fqn) {
            for super_fqn in supertypes {
                if self.interfaces.contains_key(super_fqn) && !out.contains(super_fqn) {
                    out.push(super_fqn.clone());
                }
                self.collect_mir_value_box_interfaces(super_fqn, out, visiting);
            }
        }
        visiting.remove(fqn);
    }

    fn codegen_mir_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        operand: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let value = operand
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::bool(
                    self.builder.build_not(value, "pass_mir_not")?,
                ))
            }
            ast::UnaryOp::Neg => {
                if let Some((value, int_ty)) = operand.as_int() {
                    return Ok(CgValue::int(
                        self.builder.build_int_neg(value, "pass_mir_neg")?,
                        int_ty,
                    ));
                }
                if let Some((value, float_ty)) = operand.as_float() {
                    return Ok(CgValue::float(
                        self.builder.build_float_neg(value, "pass_mir_fneg")?,
                        float_ty,
                    ));
                }
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR numeric unary",
                    at: span.into(),
                })
            }
            ast::UnaryOp::BitNot => {
                let (value, int_ty) =
                    operand.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::int(
                    self.builder.build_not(value, "pass_mir_bitnot")?,
                    int_ty,
                ))
            }
        }
    }

    fn codegen_mir_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: CgValue<'ctx>,
        rhs: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_int(), rhs.as_int()) {
            let target_ty = self.pass_mir_binary_int_target_ty(op, l_ty, r_ty);
            let l = self.cast_int(l, l_ty, target_ty)?;
            let r = self.cast_int(r, r_ty, target_ty)?;
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::int(
                        self.builder.build_int_add(l, r, "pass_mir_iadd")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::int(
                        self.builder.build_int_sub(l, r, "pass_mir_isub")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::int(
                        self.builder.build_int_mul(l, r, "pass_mir_imul")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div if target_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_div(l, r, "pass_mir_sdiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_div(l, r, "pass_mir_udiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem if target_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_rem(l, r, "pass_mir_srem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_rem(l, r, "pass_mir_urem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shl => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder.build_left_shift(l, r, "pass_mir_shl")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shr if target_ty.signed => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, true, "pass_mir_ashr")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shr => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, false, "pass_mir_lshr")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitAnd => {
                    return Ok(CgValue::int(
                        self.builder.build_and(l, r, "pass_mir_iand")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitXor => {
                    return Ok(CgValue::int(
                        self.builder.build_xor(l, r, "pass_mir_ixor")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitOr => {
                    return Ok(CgValue::int(
                        self.builder.build_or(l, r, "pass_mir_ior")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Lt => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Lt),
                    l,
                    r,
                    "pass_mir_ilt",
                )?,
                ast::BinaryOp::Le => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Le),
                    l,
                    r,
                    "pass_mir_ile",
                )?,
                ast::BinaryOp::Gt => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Gt),
                    l,
                    r,
                    "pass_mir_igt",
                )?,
                ast::BinaryOp::Ge => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Ge),
                    l,
                    r,
                    "pass_mir_ige",
                )?,
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_ieq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_ine")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some(l), Some(r)) = (lhs.as_bool(), rhs.as_bool()) {
            let value = match op {
                ast::BinaryOp::LogAnd | ast::BinaryOp::BitAnd => {
                    self.builder.build_and(l, r, "pass_mir_band")?
                }
                ast::BinaryOp::LogOr | ast::BinaryOp::BitOr => {
                    self.builder.build_or(l, r, "pass_mir_bor")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_beq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_bne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_float(), rhs.as_float()) {
            let target_ty = if l_ty == CgTy::Float64 || r_ty == CgTy::Float64 {
                CgTy::Float64
            } else {
                CgTy::Float32
            };
            let l = self.cast_float(l, l_ty, target_ty)?;
            let r = self.cast_float(r, r_ty, target_ty)?;
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::float(
                        self.builder.build_float_add(l, r, "pass_mir_fadd")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::float(
                        self.builder.build_float_sub(l, r, "pass_mir_fsub")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::float(
                        self.builder.build_float_mul(l, r, "pass_mir_fmul")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::float(
                        self.builder.build_float_div(l, r, "pass_mir_fdiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::float(
                        self.builder.build_float_rem(l, r, "pass_mir_frem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Lt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLT, l, r, "pass_mir_flt")?
                }
                ast::BinaryOp::Le => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLE, l, r, "pass_mir_fle")?
                }
                ast::BinaryOp::Gt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGT, l, r, "pass_mir_fgt")?
                }
                ast::BinaryOp::Ge => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGE, l, r, "pass_mir_fge")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_float_compare(FloatPredicate::OEQ, l, r, "pass_mir_feq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_float_compare(FloatPredicate::UNE, l, r, "pass_mir_fne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR float binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if matches!(op, ast::BinaryOp::Eq | ast::BinaryOp::Ne)
            && lhs.ty == CgTy::String
            && rhs.ty == CgTy::String
        {
            let Some(BasicValueEnum::PointerValue(lhs_ptr)) = lhs.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality lhs",
                    at: span.into(),
                });
            };
            let Some(BasicValueEnum::PointerValue(rhs_ptr)) = rhs.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality rhs",
                    at: span.into(),
                });
            };
            let runtime = self.declare_runtime_string_equals();
            let call = self.builder.build_call(
                runtime,
                &[lhs_ptr.into(), rhs_ptr.into()],
                "pass_mir_string_eq",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR string equality return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(eq_i64) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality return type",
                    at: span.into(),
                });
            };
            let mut is_eq = self.builder.build_int_compare(
                IntPredicate::NE,
                eq_i64,
                self.context.i64_type().const_zero(),
                "pass_mir_string_eq_bool",
            )?;
            if op == ast::BinaryOp::Ne {
                is_eq = self.builder.build_not(is_eq, "pass_mir_string_ne_bool")?;
            }
            return Ok(CgValue::bool(is_eq));
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR binary operands",
            at: span.into(),
        })
    }

    fn pass_mir_binary_int_target_ty(&self, op: ast::BinaryOp, lhs: IntTy, rhs: IntTy) -> IntTy {
        if matches!(op, ast::BinaryOp::Shl | ast::BinaryOp::Shr) {
            return lhs;
        }
        let word_bits = self.host.word_bit_width();
        if lhs.bits == word_bits && rhs.bits != word_bits {
            rhs
        } else {
            lhs
        }
    }

    pub(super) fn mir_local_slot(
        &self,
        span: crate::span::Span,
        slots: &[MirLocalSlot<'ctx>],
        local: crate::mir::LocalId,
    ) -> Result<MirLocalSlot<'ctx>, LlvmEmitError> {
        slots
            .get(local.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR local",
                at: span.into(),
            })
    }

    pub(super) fn load_mir_local(
        &mut self,
        span: crate::span::Span,
        slot: MirLocalSlot<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match slot.cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let local_ptr = self.local_ptr_for_use(
                    span,
                    CgLocal {
                        hir_ty: None,
                        call_may_suspend: false,
                        ty: slot.cg_ty,
                        ptr: slot.ptr,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                    "pass_mir_load_slot",
                )?;
                let llvm_ty = self.llvm_basic_type_of(span, slot.cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "pass_mir_load")?;
                self.cg_value_from_loaded(span, slot.cg_ty, loaded)
            }
        }
    }
}

fn mir_member_value_fqn_for_codegen(
    span: crate::span::Span,
    member: &crate::mir::MemberAccessMetadata,
) -> Result<&str, LlvmEmitError> {
    match member.resolved.as_ref() {
        Some(crate::mir::MemberTarget::Value { fqn }) => Ok(fqn.as_str()),
        Some(_) => Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR member target is not value",
            at: span.into(),
        }),
        None => Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR member target unresolved",
            at: span.into(),
        }),
    }
}

fn mir_store_member_continuation_route_is_lowerable(
    span: crate::span::Span,
    body: &crate::mir::Body,
    continuation_route: &crate::mir::StoredContinuationRoutePublication,
) -> Result<(), LlvmEmitError> {
    match continuation_route {
        crate::mir::StoredContinuationRoutePublication::Ambiguous => {
            Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR ambiguous member continuation route",
                at: span.into(),
            })
        }
        crate::mir::StoredContinuationRoutePublication::None => Ok(()),
        crate::mir::StoredContinuationRoutePublication::Unique(route) => {
            let Some(local) = body.locals.get(route.source_local.as_u32() as usize) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR member continuation route missing source local",
                    at: span.into(),
                });
            };
            if local.ty != route.source_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR member continuation route source type drift",
                    at: span.into(),
                });
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
enum IntCompareKind {
    Lt,
    Le,
    Gt,
    Ge,
}

fn int_predicate(ty: IntTy, kind: IntCompareKind) -> IntPredicate {
    match (ty.signed, kind) {
        (true, IntCompareKind::Lt) => IntPredicate::SLT,
        (true, IntCompareKind::Le) => IntPredicate::SLE,
        (true, IntCompareKind::Gt) => IntPredicate::SGT,
        (true, IntCompareKind::Ge) => IntPredicate::SGE,
        (false, IntCompareKind::Lt) => IntPredicate::ULT,
        (false, IntCompareKind::Le) => IntPredicate::ULE,
        (false, IntCompareKind::Gt) => IntPredicate::UGT,
        (false, IntCompareKind::Ge) => IntPredicate::UGE,
    }
}

fn map_mir_call_args_to_params(
    params: &[hir::Param],
    args: &[crate::mir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == params.len()).then_some(out)
}

fn map_mir_call_args_to_mir_params(
    params: &[crate::mir::Param],
    args: &[crate::mir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == params.len()).then_some(out)
}

fn map_mir_call_args_to_param_names(
    param_names: &[String],
    args: &[crate::mir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; param_names.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => param_names
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= param_names.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == param_names.len()).then_some(out)
}

pub(super) fn collect_mir_local_uses(body: &crate::mir::Body) -> HashSet<crate::mir::LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            match &stmt.kind {
                crate::mir::StatementKind::Assign { value, .. } => {
                    collect_mir_rvalue_uses(value, &mut out);
                }
                crate::mir::StatementKind::StoreMember {
                    receiver, value, ..
                } => {
                    collect_mir_operand_use(receiver, &mut out);
                    collect_mir_operand_use(value, &mut out);
                }
                crate::mir::StatementKind::StoreTopLevelVar { value, .. } => {
                    collect_mir_operand_use(value, &mut out);
                }
                crate::mir::StatementKind::Nop | crate::mir::StatementKind::Todo(_) => {}
            }
        }
        collect_mir_terminator_uses(&block.terminator.kind, &mut out);
    }
    out
}

fn collect_mir_operand_use(operand: &crate::mir::Operand, out: &mut HashSet<crate::mir::LocalId>) {
    if let crate::mir::Operand::Local(local) = operand {
        out.insert(*local);
    }
}

fn collect_mir_call_kind_uses(kind: &crate::mir::CallKind, out: &mut HashSet<crate::mir::LocalId>) {
    match kind {
        crate::mir::CallKind::Direct { .. } => {}
        crate::mir::CallKind::Closure { callee, .. }
        | crate::mir::CallKind::FunValue { callee } => collect_mir_operand_use(callee, out),
        crate::mir::CallKind::Virtual { receiver, .. }
        | crate::mir::CallKind::Interface { receiver, .. } => {
            collect_mir_operand_use(receiver, out);
        }
        crate::mir::CallKind::Resume { continuation, .. } => {
            collect_mir_operand_use(continuation, out);
        }
    }
}

fn collect_mir_rvalue_uses(value: &crate::mir::Rvalue, out: &mut HashSet<crate::mir::LocalId>) {
    match value {
        crate::mir::Rvalue::Use(operand)
        | crate::mir::Rvalue::Transport { value: operand, .. }
        | crate::mir::Rvalue::Unary { operand, .. }
        | crate::mir::Rvalue::TypeCheck { value: operand, .. }
        | crate::mir::Rvalue::Cast { value: operand, .. }
        | crate::mir::Rvalue::MemberAccess {
            receiver: operand, ..
        }
        | crate::mir::Rvalue::TupleGet { tuple: operand, .. }
        | crate::mir::Rvalue::CaptureBoxNew { value: operand, .. }
        | crate::mir::Rvalue::CaptureBoxGet {
            box_operand: operand,
            ..
        }
        | crate::mir::Rvalue::PatternMatch {
            subject: operand, ..
        }
        | crate::mir::Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_mir_operand_use(operand, out),
        crate::mir::Rvalue::Binary { lhs, rhs, .. } => {
            collect_mir_operand_use(lhs, out);
            collect_mir_operand_use(rhs, out);
        }
        crate::mir::Rvalue::Call { kind, args, .. } => {
            collect_mir_call_kind_uses(kind, out);
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_mir_operand_use(element, out);
            }
        }
        crate::mir::Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_mir_operand_use(&field.value, out);
            }
        }
        crate::mir::Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::mir::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_mir_operand_use(value, out);
                }
            }
        }
        crate::mir::Rvalue::CaptureBoxSet {
            box_operand, value, ..
        } => {
            collect_mir_operand_use(box_operand, out);
            collect_mir_operand_use(value, out);
        }
        crate::mir::Rvalue::MakeClosure { env, .. } => collect_mir_operand_use(env, out),
        crate::mir::Rvalue::TopLevelRef(_)
        | crate::mir::Rvalue::UnresolvedName { .. }
        | crate::mir::Rvalue::SizeOf { .. }
        | crate::mir::Rvalue::TypeMetadataLiteral(_)
        | crate::mir::Rvalue::PerformResult { .. }
        | crate::mir::Rvalue::Todo(_) => {}
    }
}

fn collect_mir_terminator_uses(
    terminator: &crate::mir::TerminatorKind,
    out: &mut HashSet<crate::mir::LocalId>,
) {
    match terminator {
        crate::mir::TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_mir_operand_use(value, out);
            }
        }
        crate::mir::TerminatorKind::CondBr { cond, .. } => collect_mir_operand_use(cond, out),
        crate::mir::TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::TerminatorKind::ResumeUnwind
        | crate::mir::TerminatorKind::Goto { .. }
        | crate::mir::TerminatorKind::Unreachable
        | crate::mir::TerminatorKind::Handle { .. }
        | crate::mir::TerminatorKind::Todo(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_unsupported_kind(result: Result<(), LlvmEmitError>, expected: &'static str) {
        match result.expect_err("helper should reject invalid member contract") {
            LlvmEmitError::UnsupportedMainBody { kind, .. } => assert_eq!(kind, expected),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn refactor_mir_member_access_codegen_rejects_unresolved_metadata() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let member = crate::mir::MemberAccessMetadata {
            name: "count".to_string(),
            receiver_ty: builtins.int,
            resolved: None,
            hidden_effects: crate::ty::EffectRow::pure(),
        };

        let result =
            mir_member_value_fqn_for_codegen(crate::span::Span::new(0, 1), &member).map(|_| ());

        assert_unsupported_kind(result, "pass MIR member target unresolved");
    }

    #[test]
    fn refactor_mir_store_member_codegen_rejects_ambiguous_continuation_route() {
        let body = crate::mir::Body::new_empty();
        let result = mir_store_member_continuation_route_is_lowerable(
            crate::span::Span::new(0, 1),
            &body,
            &crate::mir::StoredContinuationRoutePublication::Ambiguous,
        );

        assert_unsupported_kind(result, "pass MIR ambiguous member continuation route");
    }

    #[test]
    fn refactor_mir_store_member_codegen_validates_unique_continuation_route_source() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = crate::mir::Body::new_empty();
        let source_local = body.push_local(crate::mir::LocalDecl {
            span: crate::span::Span::new(0, 1),
            name: Some("k".to_string()),
            ty: builtins.unit,
            source: crate::mir::LocalSourceKind::SourceLocal,
        });

        let ok = mir_store_member_continuation_route_is_lowerable(
            crate::span::Span::new(0, 1),
            &body,
            &crate::mir::StoredContinuationRoutePublication::Unique(
                crate::mir::StoredContinuationValueRoute {
                    source_local,
                    source_ty: builtins.unit,
                    path: Vec::new(),
                },
            ),
        );
        assert!(ok.is_ok());

        let drift = mir_store_member_continuation_route_is_lowerable(
            crate::span::Span::new(0, 1),
            &body,
            &crate::mir::StoredContinuationRoutePublication::Unique(
                crate::mir::StoredContinuationValueRoute {
                    source_local,
                    source_ty: builtins.int,
                    path: Vec::new(),
                },
            ),
        );
        assert_unsupported_kind(
            drift,
            "pass MIR member continuation route source type drift",
        );
    }
}
