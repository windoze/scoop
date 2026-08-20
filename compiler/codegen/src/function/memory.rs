use super::*;

mod allocation;
mod arrays;
mod barrier;
mod card;
mod poll;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Get or declare a runtime function with the given signature.
    pub(super) fn runtime_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        self.llvm
            .get_function(symbol)
            .unwrap_or_else(|| self.llvm.add_function(symbol, ty, None))
    }

    /// Declare a helper which is proven not to enter Scoop GC or park the
    /// current managed segment. RS4GC must leave these calls untouched.
    pub(super) fn gc_leaf_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(symbol, ty);
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    pub(super) fn native_global_bridge(
        &self,
        symbol: &str,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(
            symbol,
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    pub(super) fn emit_native_global_call(
        &mut self,
        callee: inkwell::values::FunctionValue<'ctx>,
        storage: PointerValue<'ctx>,
        global: scoop_lir::NativeGlobalId,
        protocol: &scoop_lir::NativeStorageProtocol,
    ) -> Result<(), CodegenError> {
        match protocol {
            scoop_lir::NativeStorageProtocol::NoTransition => {
                if self.native_globals[global].thread_local
                    || self.function.gc_effect != scoop_lir::GcEffect::NoGc
                {
                    return Err(CodegenError(
                        "a native storage leaf requires a non-TLS global in a NoGc body".into(),
                    ));
                }
                self.builder
                    .build_call(callee, &[storage.into()], "native_global_call")
                    .map_err(|error| CodegenError(format!("native global call: {error}")))?;
            }
            scoop_lir::NativeStorageProtocol::NativeSafe { safepoint, roots } => {
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    self.safepoint_id(*safepoint),
                )?;
                self.builder
                    .build_call(callee, &[storage.into()], "native_global_call")
                    .map_err(|error| CodegenError(format!("native global call: {error}")))?;
                self.finish_native_transition(
                    transition,
                    NativeTransitionKind::Safe,
                    roots.as_slice(),
                )?;
            }
        }
        Ok(())
    }

    /// Byte-offset GEP from an opaque pointer (object field access).
    pub(super) fn byte_gep(
        &self,
        ptr: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: `ptr` addresses an object at least `offset` bytes
        // large (callers address fields of the runtime object model).
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                ptr,
                &[self.context.i64_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "gep {name} @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })
    }

    /// Address of the tag field of a tagged enum value in memory.
    pub(super) fn tag_ptr(
        &self,
        slot: PointerValue<'ctx>,
        ty: inkwell::types::StructType<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: constant indexes 0, 0 address the i64 tag of the
        // `{ i64, [M x i8] }` object `slot` points to.
        unsafe {
            self.builder.build_gep(
                ty,
                slot,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "tag_ptr",
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "tag gep @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })
    }

    /// Address of one tagged-enum field at its enum-relative byte
    /// offset. LIR owns slot assignment and natural field layout.
    pub(super) fn enum_field_ptr(
        &self,
        slot: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: LIR guarantees every field offset lies within the
        // complete tagged-enum storage represented by `slot`.
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                slot,
                &[self.context.i64_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "enum field gep @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })
    }
}
