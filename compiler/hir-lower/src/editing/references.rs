//! Reverse indexes reuse completed bindings and ordinary declaration identities.

use std::{collections::HashMap, rc::Rc};

use super::*;
use hir::{EditingReferenceTarget as Target, EditingValue as V};

mod bodies;
mod constructors;
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
    parameter_definitions: HashMap<hir::SemanticSourceLocation, Target>,
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
            builder
                .unavailable
                .extend(self.unavailable_files.iter().cloned());
            let mut checked_bodies = std::collections::HashSet::new();
            for function in self.functions.clone() {
                if check() {
                    return Err(EditingCancelled);
                }
                builder.function(self, &function);
                builder.body(
                    self,
                    &hir::EditingSourceBody {
                        id: function.function.into(),
                        owner: hir::EditingSourceBodyOwner::Callable(
                            hir::SemanticCallable::Current(function.function),
                        ),
                        origin: function.name_origin,
                    },
                    &check,
                )?;
                checked_bodies.insert(function.function.into());
            }
            loop {
                let bodies = self
                    .source_bodies()
                    .into_iter()
                    .filter(|body| !checked_bodies.contains(&body.id))
                    .collect::<Vec<_>>();
                if bodies.is_empty() {
                    break;
                }
                for body in bodies {
                    checked_bodies.insert(body.id);
                    builder.body(self, &body, &check)?;
                }
            }
            if self.declarations_ready {
                builder.types(&self.type_references);
                for reference in &self.constant_references {
                    if check() {
                        return Err(EditingCancelled);
                    }
                    match builder
                        .identities
                        .property(&mut self.lowerer, reference.target)
                    {
                        Some(target) => builder.record(
                            Target::Property(target),
                            reference.origin.clone(),
                            false,
                        ),
                        None => builder.unavailable.push(reference.origin.clone()),
                    }
                    if !reference.imports.is_empty() {
                        builder
                            .import_uses
                            .push((reference.origin.clone(), reference.imports.clone()));
                    }
                }
                for reference in &self.static_variant_references {
                    if check() {
                        return Err(EditingCancelled);
                    }
                    match builder
                        .identities
                        .callee(&mut self.lowerer, reference.target.into())
                    {
                        Some(target) => builder.record(
                            Target::Callable(target),
                            reference.origin.clone(),
                            false,
                        ),
                        None => builder.unavailable.push(reference.origin.clone()),
                    }
                    if !reference.imports.is_empty() {
                        builder
                            .import_uses
                            .push((reference.origin.clone(), reference.imports.clone()));
                    }
                }
                builder.static_calls(self, &check)?;
                builder.constructors(self, &check)?;
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
                    if !self.declaration_scope_available(&origin) {
                        continue;
                    }
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
                self.declarations_ready && self.unavailable_files.is_empty(),
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
    fn static_calls(
        &mut self,
        analysis: &mut DeclarationAnalysis,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        for reference in &analysis.static_calls {
            if check() {
                return Err(EditingCancelled);
            }
            let call = &reference.call;
            if let Some((callee, span)) = call.callee.zip(call.callee_span) {
                let origin = hir::SemanticSourceLocation {
                    source: reference.source.clone(),
                    span,
                };
                match self.identities.callee(&mut analysis.lowerer, callee) {
                    Some(target) => self.record(Target::Callable(target), origin, false),
                    None => self.unavailable.push(origin),
                }
            }
            self.call_parameters(&mut analysis.lowerer, &reference.source, call);
        }
        Ok(())
    }

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
        if is_declaration && matches!(target, Target::Parameter { .. }) {
            self.parameter_definitions.insert(origin.clone(), target);
        }
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
