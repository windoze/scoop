//! The editor reuses declaration collection and signature resolution in place.

use super::*;
use crate::editing::DeclarationAnalysis;

impl Lowerer {
    pub(crate) fn analyze_declaration_inputs<'a>(
        mut self,
        sources: &[ast::DeclarationSource<'a, '_>],
        headers: Vec<ast::SourceHeaderRef<'a>>,
        world: &hir::ImportedSemanticWorld<'_>,
    ) -> DeclarationAnalysis {
        let declarations = self.collect_declaration_inputs(
            headers,
            sources.iter().enumerate().flat_map(|(file, source)| {
                source
                    .declarations
                    .iter()
                    .copied()
                    .map(move |declaration| (file, declaration))
            }),
            Some(world),
        );
        let declarations = match declarations {
            Ok(declarations) => declarations,
            Err(diagnostics) => {
                self.diagnostics.extend(diagnostics);
                return DeclarationAnalysis {
                    lowerer: self,
                    functions: Vec::new(),
                    declarations_ready: false,
                    recovered_bodies: Default::default(),
                    receiver_facts: Default::default(),
                };
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
        let declarations_ready = match self.resolve_source_signatures(&declarations) {
            Ok(_) => true,
            Err(diagnostics) => {
                self.diagnostics.extend(diagnostics);
                false
            }
        };
        let recovered_bodies = declarations
            .recovered_functions
            .iter()
            .map(|(function, body)| (*function, (*body).clone()))
            .collect();
        self.declaration_inference.ready = declarations_ready;
        DeclarationAnalysis {
            lowerer: self,
            functions,
            declarations_ready,
            recovered_bodies,
            receiver_facts: Default::default(),
        }
    }
}
