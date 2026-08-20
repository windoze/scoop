//! GC effects of value construction and source secondary constructors.

use super::*;

impl Lowerer {
    pub(super) fn check_no_gc_constructors(&mut self) {
        let constructors = self
            .struct_constructors
            .iter()
            .filter_map(|(id, c)| (c.source_gc_effect() == hir::GcEffect::NoGc).then_some(id))
            .collect::<Vec<_>>();
        for id in constructors {
            let constructor = self.struct_constructors[id].clone();
            self.current_file = constructor.origin.file as usize;
            let hir::StructConstructorKind::Secondary {
                delegation, body, ..
            } = &constructor.kind
            else {
                unreachable!("only a source secondary constructor has a NoGC contract");
            };
            let owner = &self.structs[constructor.owner];
            let result = self.struct_applications[owner.self_application].canonical_type;
            let name = owner.name.clone();
            let mut requirements = HashSet::new();
            let mut violations = Vec::new();
            self.collect_no_gc_constructor_value(
                result,
                constructor.span,
                &format!("constructor `{name}` result"),
                &mut violations,
                &mut requirements,
            );
            for parameter in &constructor.parameters {
                self.collect_no_gc_constructor_value(
                    parameter.ty,
                    constructor.span,
                    &format!("constructor `{name}` parameter `{}`", parameter.name),
                    &mut violations,
                    &mut requirements,
                );
            }
            for (_, local) in delegation.arguments.locals.iter().chain(body.locals.iter()) {
                self.collect_no_gc_constructor_value(
                    local.ty,
                    constructor.span,
                    &format!("constructor `{name}` local `{}`", local.name),
                    &mut violations,
                    &mut requirements,
                );
            }
            self.check_no_gc_constructor_call(
                self.struct_constructor_applications[delegation.target].constructor,
                constructor.span,
                &mut violations,
            );
            self.collect_no_gc_statement_violations(
                &delegation.arguments.statements,
                &mut violations,
                &mut requirements,
            );
            for argument in &delegation.arguments.args {
                self.collect_no_gc_expr_violations(argument, &mut violations, &mut requirements);
            }
            self.collect_no_gc_statement_violations(
                &body.statements,
                &mut violations,
                &mut requirements,
            );
            for (span, message) in violations {
                self.error(span, message);
            }
            let mut requirements = requirements.into_iter().collect::<Vec<_>>();
            requirements.sort_by_key(|p| p.into_raw());
            self.struct_constructors[id].no_gc_type_params = requirements;
        }
    }

    fn collect_no_gc_constructor_value(
        &self,
        ty: hir::TypeId,
        span: Span,
        context: &str,
        violations: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        match self.gc_free_requirements(ty) {
            Some(required) => requirements.extend(required),
            None => violations.push((
                span,
                format!(
                    "`@NoGC` {context} has non-GC-free type {}",
                    self.type_name(ty)
                ),
            )),
        }
    }

    pub(super) fn check_no_gc_constructor_call(
        &self,
        constructor: hir::StructConstructorDefinition,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        let (name, managed) = match constructor {
            hir::StructConstructorDefinition::Local(constructor) => {
                let constructor = &self.struct_constructors[constructor];
                (
                    &self.structs[constructor.owner].name,
                    matches!(
                        constructor.kind,
                        hir::StructConstructorKind::Secondary {
                            gc_effect: hir::GcEffect::Managed,
                            ..
                        }
                    ),
                )
            }
            hir::StructConstructorDefinition::Template(template) => {
                let template = &self.imported_constructor_templates[template];
                let constructor = &template.initialization.constructors()[template.constructor];
                (
                    &template.signature.name,
                    matches!(
                        constructor.kind(),
                        hir::ExportConstructorInitializationKindV1::StructSecondary { .. }
                    ) && template.signature.effects.gc_effect()
                        == scoop_identity::GcEffect::Managed,
                )
            }
        };
        if managed {
            out.push((
                span,
                format!("`@NoGC` code may not call managed constructor `{name}`"),
            ));
        }
    }
}
