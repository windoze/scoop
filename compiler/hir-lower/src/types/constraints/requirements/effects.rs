use super::*;

impl Lowerer {
    pub(super) fn member_requirement_effects(
        &mut self,
        source: &ast::MemberRequirement,
    ) -> Option<hir::RequiredCallEffects> {
        let mut effects = hir::RequiredCallEffects {
            is_suspend: source.suspend_span.is_some(),
            safety: hir::Safety::Safe,
            gc_effect: hir::GcEffect::Managed,
        };
        let mut seen = std::collections::HashSet::new();
        for annotation in &source.annotations {
            let Some(name @ ("NoGC" | "Unsafe")) = annotation.name.single_text() else {
                self.error(
                    annotation.span,
                    "member requirements only allow `@NoGC` and `@Unsafe` annotations".into(),
                );
                return None;
            };
            if annotation.use_site.is_some() {
                self.error(
                    annotation.span,
                    "member requirement effects do not accept annotation use-site targets".into(),
                );
                return None;
            }
            if !seen.insert(name) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
                return None;
            }
            if !self.annotation_marker(annotation) {
                return None;
            }
            if name == "NoGC" {
                effects.gc_effect = hir::GcEffect::NoGc;
            } else {
                effects.safety = hir::Safety::Unsafe;
            }
        }
        if effects.is_suspend && effects.gc_effect == hir::GcEffect::NoGc {
            self.error(
                source.span,
                "`@NoGC` cannot be used on a suspend member requirement".into(),
            );
            return None;
        }
        Some(effects)
    }
}
