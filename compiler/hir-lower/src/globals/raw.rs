//! Raw storage initializers use the complete global name and type inventory.

use super::*;

impl Lowerer {
    pub(super) fn resolve_raw_global_initializers(
        &mut self,
        initializers: Vec<(hir::GlobalId, &ast::PropertyDecl)>,
    ) {
        // Every global name and type is available before constants are
        // inspected. References to another global are nevertheless rejected:
        // initialization has no runtime ordering phase in M12.
        for (id, decl) in initializers {
            self.current_file = self.property_files[&self.globals[id].property];
            let global = self.globals[id].clone();
            match global.storage {
                hir::GlobalStorage::Managed { .. } => {
                    unreachable!("the M12 initializer worklist contains only raw storage")
                }
                hir::GlobalStorage::Extern { .. } => {
                    let property_name = self.properties[global.property].name.clone();
                    if let Err(reason) = self.validate_c_global_type(global.ty, &property_name) {
                        self.error(
                            decl.ty.span,
                            format!("extern global type is not C-FFI-safe: {reason}"),
                        );
                    }
                }
                hir::GlobalStorage::Local { thread_local, .. } => {
                    if self.type_contains_param(global.ty) || !self.is_gc_free(global.ty) {
                        self.error(
                            decl.ty.span,
                            format!(
                                "global storage requires a concrete GC-free value type, found {}",
                                self.type_name(global.ty)
                            ),
                        );
                    }
                    if let Some(expr) = decl.initializer() {
                        if let Some(initializer) = self.global_constant(expr, global.ty) {
                            self.globals[id].storage = hir::GlobalStorage::Local {
                                thread_local,
                                initializer,
                            };
                        } else {
                            self.error(
                                expr.span(),
                                "global initializer must be a GC-free compile-time constant and must not call functions or read another global"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
    }
}
