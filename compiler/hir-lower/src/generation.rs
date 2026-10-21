//! Driver-supplied bodies fill ordinary, previously declared interface methods.

use crate::{Lowerer, semantic_queries::FrontendQueries};
use scoop_ast::{Diagnostic, DiagnosticNote};
use scoop_hir as hir;
use std::collections::{BTreeMap, HashMap};

mod sources;

#[derive(Clone, Default)]
pub(crate) struct PendingGeneration {
    pub(crate) enabled: bool,
    pub(crate) methods: BTreeMap<hir::FunctionId, PendingGenerationMethod>,
    source_scopes: HashMap<usize, usize>,
    function_sources: HashMap<hir::FunctionId, usize>,
    nominal_sources: HashMap<crate::Owner, usize>,
}

#[derive(Clone)]
pub(crate) struct PendingGenerationMethod {
    pub(crate) task: hir::PendingMethod,
    pub(crate) diagnostic: Diagnostic,
}

impl Lowerer {
    pub(crate) fn generate_method_bodies(&mut self, generator: &mut dyn hir::MethodBodyGenerator) {
        if !self.diagnostics.is_empty() {
            self.report_missing_generated_methods();
            return;
        }
        let bodies = generator.generate(&mut FrontendQueries { lowerer: self });
        match bodies {
            Ok(bodies) => {
                for generated in bodies {
                    self.install_generated_method(generated);
                }
            }
            Err(diagnostics) => self.diagnostics.extend(diagnostics),
        }
        self.report_missing_generated_methods();
    }

    fn install_generated_method(&mut self, generated: hir::GeneratedMethodBody) {
        let function = generated.function;
        let Some(pending) = self.generation.methods.remove(&function) else {
            let mut diagnostic = Diagnostic::at(
                generated.origin.span,
                "generated body does not name a remaining implementation method",
            );
            diagnostic.source = Some(Box::new(generated.origin.source));
            self.diagnostics.push(diagnostic);
            return;
        };
        let lexical_file = self.function_files[&function];
        if generated.source.identity.cone() != self.current_cone() {
            self.diagnostics.push(Diagnostic::at_file(
                lexical_file,
                self.functions[function].span,
                "generated body source belongs to a different Cone",
            ));
            return;
        }
        let source_file = self.intrinsic_sources.len() + self.imported_source_files.len();
        let file_context = hir::SourceContext::new(
            generated.source.identity.clone(),
            hir::SourceContextSubject::File,
        );
        let context = self.source_contexts.alloc(file_context.clone());
        self.source_context_by_value.insert(file_context, context);
        self.imported_source_files.push(hir::SourceFileMetadata {
            provider: self.intrinsic_sources[lexical_file].provider,
            identity: generated.source.identity,
            name: generated.source.name,
            source: generated.source.text.as_str().to_owned(),
            canonical_record: None,
        });
        self.generation
            .source_scopes
            .insert(source_file, lexical_file);
        let previous_file = std::mem::replace(&mut self.current_file, lexical_file);
        let before_errors = self.diagnostics.len();
        let before_warnings = self.warnings.len();
        let body = self.lower_generated_body(function, &generated.body, source_file);
        self.functions[function].kind = hir::FunctionKind::User(body);
        self.current_file = previous_file;
        let note = DiagnosticNote {
            file: pending.diagnostic.file,
            source: Some(Box::new(generated.origin.source)),
            span: generated.origin.span,
            message: "implementation requested here".into(),
        };
        for diagnostic in self.diagnostics[before_errors..]
            .iter_mut()
            .chain(&mut self.warnings[before_warnings..])
        {
            if diagnostic.source.is_none() && diagnostic.file == lexical_file {
                diagnostic.file = source_file;
            }
            diagnostic.notes.push(note.clone());
        }
    }

    fn report_missing_generated_methods(&mut self) {
        self.diagnostics.extend(
            std::mem::take(&mut self.generation.methods)
                .into_values()
                .map(|method| method.diagnostic),
        );
    }

    pub(crate) fn current_text_file(&self) -> usize {
        self.generated_source_file.unwrap_or(self.current_file)
    }

    pub(crate) fn source_identity(&self, file: usize) -> &scoop_identity::SourceIdentity {
        if let Some(source) = self.intrinsic_sources.get(file) {
            &source.identity
        } else {
            &self.imported_source_files[file - self.intrinsic_sources.len()].identity
        }
    }

    pub(crate) fn lexical_source_file(&self, file: usize) -> usize {
        self.generation
            .source_scopes
            .get(&file)
            .copied()
            .unwrap_or(file)
    }
}
