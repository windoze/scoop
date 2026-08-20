use super::*;

impl Lowerer {
    pub(super) fn check_release_type_applications(
        &mut self,
        values: &ReleaseValueFacts,
        sites: &[(hir::TypeId, usize, hir::Span)],
    ) {
        let mut failures = HashSet::new();
        for (_, application) in self.class_applications.iter() {
            let definition = self.class_definition(application.template);
            let required = match &definition.release_policy {
                hir::ReleasePolicy::None => continue,
                hir::ReleasePolicy::SynchronousGcFree {
                    hook: hir::ExportReleaseHookRef::Template(hook),
                } => &self.release_hooks[*hook].requirements,
                hir::ReleasePolicy::SynchronousGcFree {
                    hook: hir::ExportReleaseHookRef::Imported { requirements },
                } => requirements,
            };
            let arguments = definition
                .type_params
                .iter()
                .zip(&application.arguments)
                .filter_map(|(parameter, &argument)| {
                    required.contains(&parameter.id).then_some(argument)
                });
            if values.values(self, arguments).is_none() {
                failures.insert(application.canonical_type);
            }
        }
        if failures.is_empty() {
            return;
        }
        let mut reported = HashSet::new();
        for &(ty, file, span) in sites {
            let mut pending = vec![ty];
            let mut visited = HashSet::new();
            while let Some(ty) = pending.pop() {
                if !visited.insert(ty) {
                    continue;
                }
                if failures.contains(&ty) && reported.insert(ty) {
                    self.current_file = file;
                    self.error(
                        span,
                        format!(
                            "type {} requires ReleaseValue arguments for its `release` block",
                            self.type_name(ty)
                        ),
                    );
                }
                match &self.types[ty] {
                    hir::Type::Class(application) => {
                        pending.extend(&self.class_applications[*application].arguments)
                    }
                    hir::Type::Struct(application) => {
                        pending.extend(&self.struct_applications[*application].arguments)
                    }
                    hir::Type::Enum(application) => {
                        pending.extend(&self.enum_applications[*application].arguments)
                    }
                    hir::Type::Interface(application) => {
                        pending.extend(&self.interface_applications[*application].arguments)
                    }
                    hir::Type::Tuple(elements) => pending.extend(elements),
                    hir::Type::Ptr(pointee) => pending.push(*pointee),
                    _ => {}
                }
            }
        }
    }
}
