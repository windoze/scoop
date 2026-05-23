//! Stable symbol key and FQN production for a materialized instance — names every body the materializer outputs in a way the LLVM backend can resolve.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn template_symbol_suffix(&self, template: &TemplateKey) -> &str {
        self.template_symbol_suffixes
            .get(template)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub(super) fn stable_template_key(&self, template: &TemplateKey) -> StableTemplateKey {
        self.stable_template_keys
            .get(template)
            .cloned()
            .unwrap_or_else(|| {
                StableTemplateKey::new(StableDefKey::new(
                    self.stable_cone_key.clone(),
                    StableDefNamespace::Fun,
                    &template.fqn,
                    "materialized_callable",
                    None,
                ))
            })
    }

    pub(super) fn stable_instance_key(&self, instance: &InstanceKey) -> StableInstanceKey {
        StableInstanceKey::from_type_arguments(
            self.stable_template_key(&instance.template),
            &self.types,
            &instance.type_args,
            &instance.eff_args,
            &NoTypeParamResolver,
        )
        .unwrap_or_else(|err| {
            panic!(
                "failed to build stable instance key for `{}`: {err}",
                instance.template.fqn
            )
        })
    }

    pub(super) fn instance_exported_fun_symbol(&self, instance: &InstanceKey) -> String {
        AbiMangler.fun_symbol(&self.stable_instance_key(instance))
    }

    /// Display-only instance name used by dumps and HIR/MIR debugging.
    pub(super) fn instance_display_fqn(&self, instance: &InstanceKey) -> String {
        let symbol_suffix = self.template_symbol_suffix(&instance.template);
        if instance.type_args.is_empty() && instance.eff_args.is_empty() {
            return format!("{}{symbol_suffix}", instance.template.fqn);
        }
        let mut args = instance
            .type_args
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>();
        args.extend(
            instance
                .eff_args
                .iter()
                .map(|row| format!("eff {}", self.format_effect_row_stable(row))),
        );
        format!(
            "{}::<{}>{symbol_suffix}",
            instance.template.fqn,
            args.join(", ")
        )
    }

    pub(super) fn format_effect_row_stable(&self, row: &EffectRow) -> String {
        if row.terms.is_empty() {
            return "Pure".to_string();
        }
        row.terms
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>()
            .join(" + ")
    }

    pub(super) fn non_generic_pass_view_instance_key(
        &self,
        source_path: &Path,
        fun: &FunDecl,
    ) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: fun.fqn.clone(),
                source_path: source_path.to_path_buf(),
                decl_span: fun.span,
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        }
    }
}
