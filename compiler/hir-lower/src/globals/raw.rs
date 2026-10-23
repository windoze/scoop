//! Raw storage initializers use the complete global name and type inventory.

use super::*;

impl Lowerer {
    pub(super) fn allocate_raw_global(
        &mut self,
        decl: &ast::PropertyDecl,
        ty: hir::TypeId,
        access: hir::DeclarationAccess,
        import_source: crate::imports::PropertyImportSource,
    ) -> Option<hir::GlobalId> {
        match &decl.body {
            ast::PropertyBodySyntax::Delegated { by_span, .. } => {
                self.error(
                    *by_span,
                    "delegated properties cannot use raw or extern storage annotations".to_string(),
                );
                return None;
            }
            ast::PropertyBodySyntax::Const(_) => unreachable!("handled above"),
            ast::PropertyBodySyntax::OptionalOmitted => {
                self.error(
                    decl.span,
                    "Option shorthand cannot use raw or extern storage annotations".to_string(),
                );
                return None;
            }
            ast::PropertyBodySyntax::Abstract => {
                self.error(
                    decl.span,
                    "top-level properties cannot be abstract".to_string(),
                );
                return None;
            }
            ast::PropertyBodySyntax::Initializer { accessors, .. }
                if accessors.getter.is_some() || accessors.setter.is_some() =>
            {
                self.error(
                    decl.span,
                    "raw or extern storage cannot declare property accessors".to_string(),
                );
                return None;
            }
            ast::PropertyBodySyntax::Initializer { .. }
            | ast::PropertyBodySyntax::ExternStorage => {}
            ast::PropertyBodySyntax::Computed(_) => unreachable!("handled above"),
        }
        // Raw allocation needs a typed zero image. The registered property
        // query replaces this allocation type and image before either is used.
        let ty = if decl.ty.is_none() {
            self.integer_type(hir::IntegerKind::SIGNED_32)
        } else {
            ty
        };
        let checked = self.check_global_annotations(decl);
        let storage = if let Some(extern_) = checked.extern_ {
            hir::GlobalStorage::Extern {
                library: extern_.library,
                native_symbol: extern_.native_symbol,
                thread_local: checked.storage.unwrap_or(false),
            }
        } else {
            let Some(initializer) = self.zero_constant_image(ty) else {
                self.error(
                    decl.span,
                    format!(
                        "raw storage of type {} has no valid all-zero initial image",
                        self.type_name(ty)
                    ),
                );
                return None;
            };
            hir::GlobalStorage::Local {
                thread_local: checked.storage.unwrap_or(false),
                initializer,
            }
        };
        let expected_property = self.next_property_id();
        let id = self.globals.alloc(hir::Global {
            name: decl.name.text.clone(),
            property: expected_property,
            ty,
            mutable: decl.mutable,
            storage,
            span: decl.span,
        });
        let capability = if matches!(self.globals[id].storage, hir::GlobalStorage::Local { .. }) {
            self.allocate_property_accessors(
                expected_property,
                hir::PropertyOwner::TopLevel,
                access.clone(),
                decl,
                Some(hir::PropertyBacking::TopLevelGlobal {
                    storage: id,
                    initialization: hir::TopLevelInitialization::Image,
                }),
                hir::MethodModifier::Final,
            )?
        } else {
            self.allocate_storage_capability(access.clone(), decl.mutable, decl.span)
        };
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: decl.name.text.clone(),
            access,
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            ty,
            capability,
            representation: hir::PropertyRepresentation::NativeStorage { storage: id },
            span: decl.span,
        });
        assert_eq!(property, expected_property);
        self.property_name_spans.insert(property, decl.name.span);
        self.top_level_namespaces.register_property(
            self.current_file,
            decl.name.text.clone(),
            property,
            false,
        );
        self.property_files.insert(property, self.current_file);
        self.imports.bind_property(import_source, property);
        Some(id)
    }

    pub(crate) fn resolve_raw_global_initializers(
        &mut self,
        initializers: Vec<(hir::GlobalId, ast::PropertyDecl)>,
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
                            decl.type_span(),
                            format!("extern global type is not C-FFI-safe: {reason}"),
                        );
                    }
                }
                hir::GlobalStorage::Local { thread_local, .. } => {
                    if decl.ty.is_none() && self.zero_constant_image(global.ty).is_none() {
                        self.error(
                            decl.span,
                            format!(
                                "raw storage of type {} has no valid all-zero initial image",
                                self.type_name(global.ty)
                            ),
                        );
                        continue;
                    }
                    if self.type_contains_param(global.ty) || !self.is_gc_free(global.ty) {
                        self.error(
                            decl.type_span(),
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
