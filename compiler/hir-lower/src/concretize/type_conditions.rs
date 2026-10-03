//! Concrete type predicates use completed GC facts and the requesting source site.

use super::*;
use scoop_ast::{Diagnostic, DiagnosticSeverity, Span};

pub(super) type SourceSite = (usize, Span);

impl Concretizer<'_> {
    pub(super) fn current_source_site(
        &self,
        origin: export::EvaluationOrigin,
    ) -> Option<SourceSite> {
        let file = origin.file as usize;
        (self.source.source_files[file].identity.cone() == self.source.cone)
            .then_some((file, origin.span))
    }

    pub(super) fn source_context_site(
        &self,
        subject: export::SourceContextSubject,
        span: Span,
    ) -> Option<SourceSite> {
        let context = self
            .source
            .source_contexts
            .iter()
            .find(|(_, context)| context.subject() == &subject)?
            .1;
        let file = self.source.source_files.iter().position(|file| {
            &file.identity == context.source() && file.identity.cone() == self.source.cone
        })?;
        Some((file, span))
    }

    pub(super) fn source_nominal_site(&self, owner: export::SourceNominalId) -> Option<SourceSite> {
        let subject = match owner {
            export::SourceNominalId::Concrete(id) => {
                scoop_identity::DefinitionOriginSubject::Type(id)
            }
            export::SourceNominalId::GenericTemplate(id) => {
                scoop_identity::DefinitionOriginSubject::GenericType(id)
            }
        };
        let origin = self.source.export_definition_origins.get(subject)?.origin();
        let file = self.source.source_files.iter().position(|file| {
            &file.identity == origin.source() && file.identity.cone() == self.source.cone
        })?;
        Some((
            file,
            Span::new(
                u32::try_from(origin.span().start_byte()).expect("source spans fit HIR"),
                u32::try_from(origin.span().end_byte()).expect("source spans fit HIR"),
            ),
        ))
    }

    pub(super) fn check_completed_no_gc_type(
        &mut self,
        kind: &str,
        name: &str,
        no_gc: bool,
        gc_free: bool,
    ) {
        if !no_gc || gc_free {
            return;
        }
        let message = format!(
            "`@NoGC` {kind} `{name}` specialization is not GC-free because it directly or indirectly contains a ref type"
        );
        let diagnostic = match self.type_use_site {
            Some((file, span)) => Diagnostic::at_file(file, span, message),
            None => Diagnostic::without_span(DiagnosticSeverity::Error, 0, message),
        };
        self.type_condition_errors.push(diagnostic);
    }
}
