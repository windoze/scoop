//! Emit callable-owned recursive GC scan programs from the closed LIR plan.

use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::targets::TargetData;
use inkwell::values::{BasicValueEnum, GlobalValue, PointerValue};

use crate::atom_boundaries::{GlobalAtomMaterializationV1, emit_global_atom_boundaries_v1};
use crate::{CodegenError, RefScan, SCAN_ARRAY, SCAN_SEQUENCE, ptr_ty};

pub(crate) struct CallableRuntimeScanEmitter<'a, 'ctx> {
    context: &'ctx Context,
    llvm: &'a LlvmModule<'ctx>,
    target_data: &'a TargetData,
    surface: &'a scoop_lir::StrongObjectSymbolSurfaceV1,
    plan: &'a scoop_lir::StrongCallableRuntimeScanPlanV1,
    next: usize,
    materializations: Vec<GlobalAtomMaterializationV1<'ctx>>,
}

impl<'a, 'ctx> CallableRuntimeScanEmitter<'a, 'ctx> {
    pub(crate) fn new(
        context: &'ctx Context,
        llvm: &'a LlvmModule<'ctx>,
        target_data: &'a TargetData,
        surface: &'a scoop_lir::StrongObjectSymbolSurfaceV1,
        plan: &'a scoop_lir::StrongCallableRuntimeScanPlanV1,
    ) -> Self {
        Self {
            context,
            llvm,
            target_data,
            surface,
            plan,
            next: 0,
            materializations: Vec::with_capacity(plan.atoms().len()),
        }
    }

    pub(crate) fn emit(
        &mut self,
        scan: &RefScan,
    ) -> Result<Option<PointerValue<'ctx>>, CodegenError> {
        let i64 = self.context.i64_type();
        let words = match scan {
            RefScan::None => return Ok(None),
            RefScan::References(offsets) if offsets.is_empty() => return Ok(None),
            RefScan::References(offsets) => {
                std::iter::once(i64.const_int(offsets.len() as u64, false))
                    .chain(offsets.iter().map(|offset| i64.const_int(*offset, false)))
                    .collect()
            }
            RefScan::Sequence(parts) => {
                let mut children = Vec::new();
                for part in parts {
                    if let Some(child) = self.emit(part)? {
                        children.push(child);
                    }
                }
                if children.is_empty() {
                    return Ok(None);
                }
                std::iter::once(i64.const_int(SCAN_SEQUENCE, false))
                    .chain(std::iter::once(i64.const_int(children.len() as u64, false)))
                    .chain(children.into_iter().map(|child| child.const_to_int(i64)))
                    .collect()
            }
            RefScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => {
                let child = self.emit(element.as_ref_scan())?.ok_or_else(|| {
                    CodegenError(format!(
                        "callable {} has an empty array-element runtime scan",
                        self.plan.body()
                    ))
                })?;
                vec![
                    i64.const_int(SCAN_ARRAY, false),
                    i64.const_int(*length_offset, false),
                    i64.const_int(*first_element_offset, false),
                    i64.const_int(stride.get(), false),
                    child.const_to_int(i64),
                ]
            }
        };
        self.emit_planned_atom(scan, i64.const_array(&words).into())
            .map(Some)
    }

    fn emit_planned_atom(
        &mut self,
        scan: &RefScan,
        value: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let planned = self.plan.atoms().get(self.next).ok_or_else(|| {
            CodegenError(format!(
                "callable {} emitted more runtime scan atoms than planned",
                self.plan.body()
            ))
        })?;
        if planned.scan() != scan {
            return Err(CodegenError(format!(
                "callable {} runtime scan atom {} diverges from its closed LIR plan",
                self.plan.body(),
                planned.atom()
            )));
        }
        let boundary = self
            .surface
            .plans()
            .iter()
            .flat_map(|definition| definition.atom_boundaries())
            .find(|boundary| boundary.atom() == planned.atom())
            .copied()
            .ok_or_else(|| {
                CodegenError(format!(
                    "callable runtime scan atom {} has no strong boundary plan",
                    planned.atom()
                ))
            })?;
        if boundary.atom_role() != scoop_lir::DefinitionAtomRole::RuntimeRecord {
            return Err(CodegenError(format!(
                "callable runtime scan atom {} has role {:?}",
                planned.atom(),
                boundary.atom_role()
            )));
        }
        let symbol = boundary.start().symbol();
        let name = symbol.as_str();
        if self.llvm.get_global(name).is_some() || self.llvm.get_function(name).is_some() {
            return Err(CodegenError(format!(
                "callable runtime scan symbol `{}` collides with an LLVM value",
                name
            )));
        }
        let global: GlobalValue<'ctx> = self.llvm.add_global(value.get_type(), None, name);
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::External);
        global.set_initializer(&value);
        self.materializations
            .push(GlobalAtomMaterializationV1::new(planned.atom(), global));
        self.next += 1;
        Ok(global.as_pointer_value())
    }

    pub(crate) fn finish(&mut self) -> Result<(), CodegenError> {
        if self.next != self.plan.atoms().len() {
            return Err(CodegenError(format!(
                "callable {} emitted {} runtime scan atoms, but its closed LIR plan requires {}",
                self.plan.body(),
                self.next,
                self.plan.atoms().len()
            )));
        }
        emit_global_atom_boundaries_v1(
            self.llvm,
            self.target_data,
            self.surface,
            self.materializations.iter().copied(),
        )
    }
}

pub(crate) fn null_runtime_scan<'ctx>(context: &'ctx Context) -> PointerValue<'ctx> {
    ptr_ty(context).const_null()
}
