use super::*;
use hir::{EditingCallee as C, SemanticConstructor as Constructor};

impl ReferenceBuilder {
    pub(super) fn constructors(
        &mut self,
        analysis: &mut DeclarationAnalysis,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let nominals = analysis
            .lowerer
            .nominal_identities
            .as_ref()
            .expect("constructor declarations have nominal identities");
        let classes = analysis
            .lowerer
            .class_constructors
            .iter()
            .filter(|(_, declaration)| {
                declaration.identity_kind == hir::ClassConstructorIdentityKind::Source
                    && nominals[declaration.owner].source().is_some()
            })
            .map(|(constructor, declaration)| {
                (
                    C::Constructor(Constructor::CurrentClass(constructor)),
                    declaration.parameters.len(),
                )
            });
        let structs = analysis
            .lowerer
            .struct_constructors
            .iter()
            .filter(|(_, declaration)| nominals[declaration.owner].source().is_some())
            .map(|(constructor, declaration)| {
                (
                    C::Constructor(Constructor::CurrentStruct(constructor)),
                    declaration.parameters.len(),
                )
            });
        let variants = analysis
            .lowerer
            .enums
            .iter()
            .filter(|(enumeration, _)| nominals[*enumeration].source().is_some())
            .flat_map(|(enumeration, declaration)| {
                declaration
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let source = hir::EnumVariantRef::checked(
                            &analysis.lowerer.enums,
                            enumeration,
                            index as u32,
                        )
                        .expect("a source variant belongs to its enum");
                        (C::CurrentVariant(source), variant.fields.len())
                    })
                    .collect::<Vec<_>>()
            });
        let sources = classes.chain(structs).chain(variants).collect::<Vec<_>>();
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
