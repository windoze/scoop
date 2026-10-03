//! Validate concrete NoGC applications with their actual use-site locations.

use std::collections::HashMap;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn check_no_gc_types(&mut self, type_sites: Vec<(hir::TypeId, usize, Span)>) {
        let mut sites = HashMap::new();
        for (ty, file, span) in self.nominal_type_uses.iter().copied().chain(type_sites) {
            self.record_nominal_type_sites(ty, file, span, &mut sites);
        }
        let types = self
            .types
            .iter()
            .filter_map(|(ty, kind)| {
                let (kind, source) = match *kind {
                    hir::Type::Struct(application) => {
                        let application = &self.struct_applications[application];
                        if !self
                            .struct_definition(application.template)
                            .attributes
                            .no_gc
                        {
                            return None;
                        }
                        let source = self
                            .source_struct_id(application.template)
                            .map(|id| (self.struct_files[&id], self.structs[id].span));
                        ("struct", source)
                    }
                    hir::Type::Enum(application) => {
                        let application = &self.enum_applications[application];
                        if !self.enum_definition(application.template).no_gc {
                            return None;
                        }
                        let source = self
                            .source_enum_id(application.template)
                            .map(|id| (self.enum_files[&id], self.enums[id].span));
                        ("enum", source)
                    }
                    _ => return None,
                };
                if self.type_contains_param(ty) {
                    return None;
                }
                let (file, span) = source.or_else(|| sites.get(&ty).copied())?;
                Some((ty, kind, file, span))
            })
            .collect::<Vec<_>>();
        for (ty, kind, file, span) in types {
            if self.is_gc_free(ty) {
                continue;
            }
            self.current_file = file;
            self.error(span, format!(
                "`@NoGC` {kind} specialization `{}` is not GC-free because it directly or indirectly contains a ref type",
                self.type_name(ty),
            ));
        }
    }

    fn record_nominal_type_sites(
        &self,
        ty: hir::TypeId,
        file: usize,
        span: Span,
        sites: &mut HashMap<hir::TypeId, (usize, Span)>,
    ) {
        if sites.contains_key(&ty) {
            return;
        }
        sites.insert(ty, (file, span));
        match &self.types[ty] {
            hir::Type::Struct(application) => {
                for &argument in &self.struct_applications[*application].arguments {
                    self.record_nominal_type_sites(argument, file, span, sites);
                }
            }
            hir::Type::Enum(application) => {
                for &argument in &self.enum_applications[*application].arguments {
                    self.record_nominal_type_sites(argument, file, span, sites);
                }
            }
            hir::Type::Class(application) => {
                for &argument in &self.class_applications[*application].arguments {
                    self.record_nominal_type_sites(argument, file, span, sites);
                }
            }
            hir::Type::Interface(application) => {
                for &argument in &self.interface_applications[*application].arguments {
                    self.record_nominal_type_sites(argument, file, span, sites);
                }
            }
            hir::Type::Tuple(elements) => {
                for &element in elements {
                    self.record_nominal_type_sites(element, file, span, sites);
                }
            }
            hir::Type::Ptr(pointee) => self.record_nominal_type_sites(*pointee, file, span, sites),
            hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                let function = &self.function_types[*function];
                for &parameter in &function.parameter_types {
                    self.record_nominal_type_sites(parameter, file, span, sites);
                }
                self.record_nominal_type_sites(function.return_type, file, span, sites);
            }
            hir::Type::Unit
            | hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Any
            | hir::Type::Param(_) => {}
        }
    }
}
