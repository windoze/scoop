use super::*;

impl ReferenceBuilder {
    pub(super) fn constructors(
        &mut self,
        analysis: &mut DeclarationAnalysis,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let sources = analysis.lowerer.editing_source_constructors();
        for (callee, parameter_count) in sources {
            if check() {
                return Err(EditingCancelled);
            }
            let Some(origin) = analysis.lowerer.editing_callee_origin(callee) else {
                continue;
            };
            if !analysis.declaration_scope_available(&origin) {
                continue;
            }
            let Some(callable) = self.identities.callee(&mut analysis.lowerer, callee) else {
                self.unavailable.push(origin);
                continue;
            };
            self.record(Target::Callable(callable), origin, true);
            for index in 0..parameter_count {
                self.record(
                    Target::Parameter {
                        callable,
                        index: index as u32,
                    },
                    analysis
                        .lowerer
                        .editing_callee_parameter_origin(callee, index)
                        .expect("a declared constructor retains each source parameter"),
                    true,
                );
            }
        }
        Ok(())
    }
}
