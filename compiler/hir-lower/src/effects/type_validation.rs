//! Declaration-site validation of concrete `@NoGC` type applications.

use std::collections::HashSet;

use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn check_no_gc_types(&mut self) {
        let mut seen = HashSet::new();
        let concrete_enums = self
            .enums
            .iter()
            .filter(|(_, declaration)| declaration.no_gc && declaration.type_params.is_empty())
            .map(|(_, declaration)| {
                (
                    declaration.name.clone(),
                    declaration.span,
                    declaration.variants.iter().all(|variant| {
                        variant.fields.iter().all(|field| self.is_gc_free(field.ty))
                    }),
                )
            })
            .collect::<Vec<_>>();
        for (name, span, gc_free) in concrete_enums {
            if !gc_free {
                self.error(
                    span,
                    format!(
                        "`@NoGC` enum specialization `{name}` is not GC-free because it directly or indirectly contains a ref type"
                    ),
                );
            }
        }
        let types =
            self.types
                .iter()
                .filter_map(|(ty, kind)| match kind {
                    hir::Type::Struct(application) => {
                        let id =
                            self.source_struct_id(self.struct_applications[*application].template)?;
                        self.structs[id].attributes.no_gc.then_some((
                            ty,
                            "struct",
                            id.into_raw().into_u32(),
                            self.structs[id].span,
                        ))
                    }
                    hir::Type::Enum(application) => {
                        let id =
                            self.source_enum_id(self.enum_applications[*application].template)?;
                        (self.enums[id].no_gc && !self.enums[id].type_params.is_empty())
                            .then_some((ty, "enum", id.into_raw().into_u32(), self.enums[id].span))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
        for (ty, kind, raw_id, span) in types {
            if self.type_contains_param(ty) {
                continue;
            }
            let name = self.type_name(ty);
            if !seen.insert((kind, raw_id, name.clone())) {
                continue;
            }
            if !self.is_gc_free(ty) {
                self.error(
                    span,
                    format!(
                        "`@NoGC` {kind} specialization `{name}` is not GC-free because it directly or indirectly contains a ref type"
                    ),
                );
            }
        }
    }
}
