//! Reverse indexes reuse completed bindings and ordinary declaration identities.

use std::{collections::HashMap, rc::Rc};

use super::*;
use hir::{EditingReferenceTarget as Target, EditingValue as V};

mod bodies;
mod imports;

#[derive(Default)]
struct ReferenceBuilder {
    occurrences: Vec<hir::EditingReference>,
    unavailable: Vec<hir::SemanticSourceLocation>,
    identities: super::identities::DeclarationIdentities,
    import_uses: Vec<(
        hir::SemanticSourceLocation,
        Vec<hir::SemanticSourceLocation>,
    )>,
    aliases: la_arena::Arena<hir::EditingImportAlias>,
}

impl DeclarationAnalysis {
    pub fn reference_index(
        &mut self,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<&hir::EditingReferenceIndex, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if self.reference_index.is_none() {
            let check: Rc<dyn Fn() -> bool> = Rc::new(is_cancelled);
            let mut builder = ReferenceBuilder::default();
            for function in self.functions.clone() {
                if check() {
                    return Err(EditingCancelled);
                }
                builder.function(self, &function);
                builder.body(self, &function, &check)?;
            }
            if self.declarations_ready {
                builder.types(&self.type_references);
                let properties = self
                    .lowerer
                    .property_name_spans
                    .iter()
                    .map(|(property, span)| (*property, *span))
                    .collect::<Vec<_>>();
                for (property, span) in properties {
                    if check() {
                        return Err(EditingCancelled);
                    }
                    let Some(file) = self.lowerer.property_definition_file(property) else {
                        continue;
                    };
                    let origin = hir::SemanticSourceLocation {
                        source: self.lowerer.source_identity(file).clone(),
                        span,
                    };
                    match builder
                        .identities
                        .property(&mut self.lowerer, hir::SemanticProperty::Current(property))
                    {
                        Some(target) => builder.record(Target::Property(target), origin, true),
                        None => builder.unavailable.push(origin),
                    }
                }
            }
            if check() {
                return Err(EditingCancelled);
            }
            builder.imports(&mut self.lowerer, &check)?;
            self.reference_index = Some(hir::EditingReferenceIndex::new(
                builder.occurrences,
                builder.unavailable,
                self.declarations_ready,
                builder.aliases,
            ));
        }
        Ok(self
            .reference_index
            .as_ref()
            .expect("the current input has its index"))
    }
}

impl ReferenceBuilder {
    fn types(&mut self, references: &[hir::EditingTypeReference]) {
        for reference in references {
            self.record(
                Target::Type(reference.target),
                reference.origin.clone(),
                reference.is_declaration,
            );
            if !reference.imports.is_empty() {
                self.import_uses
                    .push((reference.origin.clone(), reference.imports.clone()));
            }
        }
    }

    fn record(
        &mut self,
        target: Target,
        origin: hir::SemanticSourceLocation,
        is_declaration: bool,
    ) {
        self.occurrences.push(hir::EditingReference {
            target,
            origin,
            is_declaration,
            aliases: Vec::new(),
        });
    }

    fn function(&mut self, analysis: &mut DeclarationAnalysis, function: &hir::EditingFunction) {
        let V::Known(signature) = analysis.signature(function.function) else {
            self.unavailable.push(function.name_origin.clone());
            return;
        };
        let Some(callable) = self.identities.callable(
            &mut analysis.lowerer,
            hir::SemanticCallable::Current(function.function),
        ) else {
            self.unavailable.push(function.name_origin.clone());
            return;
        };
        self.record(
            Target::Callable(callable),
            function.name_origin.clone(),
            true,
        );
        for (index, parameter) in signature.parameters.iter().enumerate() {
            self.record(
                Target::Parameter {
                    callable,
                    index: index as u32,
                },
                parameter.origin.clone(),
                true,
            );
        }
    }
}
