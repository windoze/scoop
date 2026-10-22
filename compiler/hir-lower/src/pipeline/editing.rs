//! The editor reuses declaration collection and signature resolution in place.

use super::*;
use crate::editing::{DeclarationAnalysis, EditingCancelled};

mod documentation;

impl Lowerer {
    pub(crate) fn analyze_declaration_inputs<'a>(
        mut self,
        sources: &[ast::DeclarationSource<'a, '_>],
        headers: Vec<ast::SourceHeaderRef<'a>>,
        world: &hir::ImportedSemanticWorld<'_>,
        check: &impl Fn() -> Result<(), EditingCancelled>,
    ) -> Result<DeclarationAnalysis, EditingCancelled> {
        self.editing_types = Some(Default::default());
        let declarations = self.collect_declaration_inputs_with_cancellation(
            headers,
            sources.iter().enumerate().flat_map(|(file, source)| {
                source
                    .declarations
                    .iter()
                    .copied()
                    .map(move |declaration| (file, declaration))
            }),
            Some(world),
            check,
        );
        let declarations = match declarations {
            Ok(declarations) => declarations,
            Err(PhaseFailure::Cancelled(cancelled)) => return Err(cancelled),
            Err(PhaseFailure::Diagnostics(diagnostics)) => {
                self.diagnostics.extend(diagnostics);
                return Ok(DeclarationAnalysis {
                    lowerer: self,
                    functions: Vec::new(),
                    declarations_ready: false,
                    recovered_bodies: Default::default(),
                    recovered_facts: Default::default(),
                    body_inputs: Default::default(),
                    body_facts: Default::default(),
                    reference_index: None,
                    type_references: Vec::new(),
                    type_applications: Vec::new(),
                    documentation: None,
                });
            }
        };
        let functions = declarations
            .function_headers
            .iter()
            .copied()
            .chain(
                declarations
                    .method_headers
                    .iter()
                    .map(|&(function, header, file, _)| (function, header, file)),
            )
            .map(|(function, header, file)| hir::EditingFunction {
                function,
                name: header.name.text.clone(),
                name_origin: hir::SemanticSourceLocation {
                    source: self.intrinsic_sources[file].identity.clone(),
                    span: header.name.span,
                },
                span: header.span,
            })
            .collect();
        let declarations_ready =
            match self.resolve_source_signatures_with_cancellation(&declarations, check) {
                Ok(_) => true,
                Err(PhaseFailure::Cancelled(cancelled)) => return Err(cancelled),
                Err(PhaseFailure::Diagnostics(diagnostics)) => {
                    self.diagnostics.extend(diagnostics);
                    false
                }
            };
        check()?;
        let recovered_bodies = declarations
            .recovered_functions
            .iter()
            .map(|(function, body)| {
                check()?;
                Ok((*function, (*body).clone()))
            })
            .collect::<Result<_, EditingCancelled>>()?;
        self.declaration_inference.ready = declarations_ready;
        let body_inputs = declarations
            .pending_functions
            .iter()
            .copied()
            .chain(
                declarations
                    .pending_methods
                    .iter()
                    .map(|&(function, declaration, file, _)| (function, declaration, file)),
            )
            .map(|(function, declaration, _)| {
                check()?;
                Ok((function, declaration.clone()))
            })
            .collect::<Result<_, EditingCancelled>>()?;
        check()?;
        let (type_references, type_applications) = self
            .editing_types
            .take()
            .expect("declaration type observation")
            .finish(&mut self);
        check()?;
        let documentation = if declarations_ready {
            self.capture_editing_documentation(&declarations, check)?;
            self.finish_editing_documentation(world, check)?
                .map(std::sync::Arc::new)
        } else {
            None
        };
        Ok(DeclarationAnalysis {
            lowerer: self,
            functions,
            declarations_ready,
            recovered_bodies,
            recovered_facts: Default::default(),
            body_inputs,
            body_facts: Default::default(),
            reference_index: None,
            type_references,
            type_applications,
            documentation,
        })
    }
}
