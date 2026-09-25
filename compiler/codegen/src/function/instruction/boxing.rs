use super::*;
use scoop_lir::{BoxPayload, BoxPayloadRooting, NoGcRuntimeFunction, UnboxResult};

/// The frame and entry share one lexical lifetime around a single managed box.
struct BoxRegionFrame<'ctx>(PointerValue<'ctx>);

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_boxing(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        match instruction {
            Instruction::BoxValue {
                out,
                payload,
                safepoint,
                live,
            } => {
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                let descriptor = self
                    .value(Value::TypeDescriptor(payload.descriptor()))?
                    .into_pointer_value();
                let (runtime, frame, args) = match payload {
                    BoxPayload::ZeroSized(_) => (
                        scoop_lir::ManagedRuntimeFunction::BoxZst,
                        None,
                        vec![descriptor.into()],
                    ),
                    BoxPayload::NonZero(place) => {
                        let source = self.local_pointer(place.local())?;
                        let frame = match place.rooting() {
                            BoxPayloadRooting::GcFree => None,
                            BoxPayloadRooting::RecursiveRegion(scan) => {
                                Some(self.push_box_region(source, scan)?)
                            }
                        };
                        (
                            scoop_lir::ManagedRuntimeFunction::BoxValue,
                            frame,
                            vec![descriptor.into(), source.into()],
                        )
                    }
                };
                let pointer = ptr_ty(self.context);
                let params = vec![pointer.into(); args.len()];
                let runtime = scoop_lir::RuntimeFunction::Managed(runtime);
                let callee = self.runtime_fn(
                    runtime.symbol(),
                    managed_ptr_ty(self.context, self.managed_address_space)
                        .fn_type(&params, false),
                );
                let call = self
                    .builder
                    .build_call(callee, &args, &format!("box.t{}", out.into_raw()))
                    .map_err(box_error)?;
                self.apply_safepoint_id(call, safepoint);
                self.apply_nounwind(call);
                let result = call
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| CodegenError("managed box produced no object".into()))?;
                self.restore_statepoint_live(live, safepoint)?;
                if let Some(BoxRegionFrame(frame)) = frame {
                    self.box_leaf(NoGcRuntimeFunction::PopRecursiveRegion, &[frame.into()])?;
                }
                self.temps.insert(*out, result);
            }
            Instruction::UnboxValue { object, result } => {
                let object = self.value(*object)?;
                let descriptor = self
                    .value(Value::TypeDescriptor(result.descriptor()))?
                    .into_pointer_value();
                match result {
                    UnboxResult::ZeroSized {
                        descriptor: value,
                        out,
                    } => {
                        self.box_leaf(
                            NoGcRuntimeFunction::UnboxZst,
                            &[object.into(), descriptor.into()],
                        )?;
                        let ty = basic_ty(
                            self.context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            value.value().representation().storage_type(),
                        )?;
                        self.temps.insert(*out, ty.const_zero());
                    }
                    UnboxResult::NonZero(place) => {
                        let destination = self.local_pointer(place.local())?;
                        self.box_leaf(
                            NoGcRuntimeFunction::UnboxValue,
                            &[object.into(), descriptor.into(), destination.into()],
                        )?;
                    }
                }
            }
            _ => unreachable!("boxing instruction dispatch is exhaustive"),
        }
        Ok(())
    }

    fn push_box_region(
        &mut self,
        source: PointerValue<'ctx>,
        scan: &scoop_lir::NonEmptyRefScan,
    ) -> Result<BoxRegionFrame<'ctx>, CodegenError> {
        let pointer = ptr_ty(self.context);
        let entry_type = self
            .context
            .struct_type(&[pointer.into(), pointer.into()], false);
        let frame_type = self.context.struct_type(
            &[
                pointer.into(),
                pointer.into(),
                self.context.i64_type().into(),
            ],
            false,
        );
        let entry = self.entry_alloca(entry_type.into(), "box.region.entry")?;
        let frame = self.entry_alloca(frame_type.into(), "box.region.frame")?;
        let scan = self
            .runtime_scans
            .emit(scan.as_ref_scan())?
            .ok_or_else(|| CodegenError("box region requires a nonempty planned scan".into()))?;
        for (index, value) in [source, scan].into_iter().enumerate() {
            let field = self
                .builder
                .build_struct_gep(entry_type, entry, index as u32, "box.region.field")
                .map_err(box_error)?;
            self.builder.build_store(field, value).map_err(box_error)?;
        }
        self.box_leaf(
            NoGcRuntimeFunction::PushRecursiveRegion,
            &[
                frame.into(),
                entry.into(),
                self.context.i64_type().const_int(1, false).into(),
            ],
        )?;
        Ok(BoxRegionFrame(frame))
    }

    fn box_leaf(
        &self,
        runtime: NoGcRuntimeFunction,
        args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<(), CodegenError> {
        let params = args
            .iter()
            .map(|arg| match arg {
                inkwell::values::BasicMetadataValueEnum::PointerValue(value) => {
                    Ok(value.get_type().into())
                }
                inkwell::values::BasicMetadataValueEnum::IntValue(value) => {
                    Ok(value.get_type().into())
                }
                _ => Err(CodegenError(
                    "boxing leaf ABI accepts only pointers and its internal root count".into(),
                )),
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let callee = self.gc_leaf_fn(
            scoop_lir::RuntimeFunction::NoGc(runtime).symbol(),
            self.context.void_type().fn_type(&params, false),
        );
        let call = self
            .builder
            .build_call(callee, args, "")
            .map_err(box_error)?;
        self.apply_nounwind(call);
        Ok(())
    }
}

fn box_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("box/unbox emission: {error}"))
}
