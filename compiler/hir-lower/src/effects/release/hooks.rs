use super::*;

mod applications;

impl Lowerer {
    pub(crate) fn check_release_blocks(
        &mut self,
        values: &ReleaseValueFacts,
        type_sites: &[(hir::TypeId, usize, hir::Span)],
    ) {
        let hooks = self
            .release_hooks
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in hooks {
            let hook = &self.release_hooks[id];
            let Some(class) = self.source_class_id(hook.owner) else {
                // Imported templates carry the result of their definition's analysis.
                continue;
            };
            let mut facts = BodyFacts::new();
            facts.release_block = true;
            self.release_body(&hook.body, values, &mut facts);
            for call in std::mem::take(&mut facts.calls) {
                let effect = match call.callee {
                    GenericCallable::Function(id) => &self.functions[id].release_callability,
                    GenericCallable::StructConstructor(id) => {
                        &self.struct_constructors[id].release_callability
                    }
                    _ => {
                        unreachable!("imported call conditions were consumed from their interfaces")
                    }
                };
                self.release_imported_call(effect, &call.arguments, values, &mut facts);
                if facts.requirements.is_none() {
                    facts.violation.get_or_insert(call.span);
                }
            }
            self.current_file = self.class_files[&class];
            let Some(required) = facts.requirements else {
                self.error(facts.violation.unwrap_or(hook.span),
                    "`release` requires ReleaseValue values and direct NoTransition calls; allocation, managed access, transitions and trapping operations are forbidden".into());
                continue;
            };
            let mut required = required.into_iter().collect::<Vec<_>>();
            required.sort_by_key(|parameter| parameter.into_raw());
            let parameters = self.classes[class].type_params.clone();
            for parameter in parameters {
                if required.contains(&parameter.id) && ref_bound(&parameter.bounds) {
                    self.error(
                        parameter.span,
                        format!(
                            "release requirement for `{}` contradicts its reference bound",
                            parameter.name
                        ),
                    );
                }
            }
            self.release_hooks[id].requirements = required;
        }
        self.check_release_type_applications(values, type_sites);
    }
}

fn ref_bound(bounds: &hir::TypeParamBounds) -> bool {
    matches!(bounds, hir::TypeParamBounds::Ref { .. })
        || matches!(bounds, hir::TypeParamBounds::Nominal(bounds) if bounds.class.is_some())
}
