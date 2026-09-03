use super::*;

impl Lowerer {
    pub(super) fn lower_extern_functions(&mut self, module: &hir::Module) {
        for (hir_id, extern_) in module.extern_functions.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let params = extern_
                .params
                .iter()
                .map(|&ty| {
                    types.lower(
                        ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                extern_.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let id = self.extern_functions.alloc(mir::ExternFunction {
                source_name: extern_.source_name.clone(),
                native_symbol: extern_.native_symbol.clone(),
                library: extern_.library.clone(),
                abi: match extern_.abi {
                    hir::ExternAbi::C => mir::ExternAbi::C,
                    hir::ExternAbi::Scoop => mir::ExternAbi::Scoop,
                },
                calling_convention: match extern_.calling_convention {
                    hir::CallingConvention::Cdecl => mir::CallingConvention::Cdecl,
                },
                gc_effect: match extern_.gc_effect {
                    hir::GcEffect::Managed => mir::GcEffect::Managed,
                    hir::GcEffect::NoGc => mir::GcEffect::NoGc,
                },
                params,
                return_type,
            });
            self.extern_map.insert(hir_id, id);
        }
    }

    /// Transpose the concrete HIR function-type arena one-to-one. MIR keeps
    /// the same typed identity order, so type lowering never scans signatures
    /// or re-interns an equal shape.
    pub(super) fn lower_function_types(&mut self, module: &hir::Module) {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        for (source_id, source) in module.function_types.iter() {
            let parameter_types = source
                .parameter_types
                .iter()
                .map(|parameter| {
                    types.lower(
                        *parameter,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                source.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let target_id = self.shell.function_types.alloc(mir::FunctionType {
                is_suspend: source.is_suspend,
                parameter_types,
                return_type,
            });
            assert_eq!(source_id.into_raw(), target_id.into_raw());
        }
    }

    pub(super) fn lower_globals(&mut self, module: &hir::Module) {
        for (hir_id, global) in module.globals.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let ty = types.lower(
                global.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let storage = match &global.storage {
                hir::GlobalStorage::Local {
                    thread_local,
                    initializer,
                } => mir::GlobalStorage::Local {
                    thread_local: *thread_local,
                    initializer: lower_global_constant(initializer, &ty, &self.structs.defs),
                },
                hir::GlobalStorage::Extern {
                    library,
                    native_symbol,
                    thread_local,
                } => mir::GlobalStorage::Extern {
                    library: library.clone(),
                    native_symbol: native_symbol.clone(),
                    thread_local: *thread_local,
                },
            };
            let id = self.globals.alloc(mir::Global {
                name: global.name.clone(),
                symbol: mir::mangle_global(&global.name),
                ty,
                mutable: global.mutable,
                storage,
            });
            self.global_map.insert(hir_id, id);
        }
    }
}
