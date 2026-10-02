use scoop_ast as ast;
use scoop_hir as hir;

use super::{TypeLookupCandidate, TypeLookupOrigin, TypeLookupTarget};
use crate::{Lowerer, NominalTarget, namespace::TopLevelTypeTarget};

pub(super) fn sort_candidate_notes(notes: &mut [ast::DiagnosticNote]) {
    // Presentation order never chooses a semantic winner.
    notes.sort_by(|left, right| {
        (&left.source, left.file, left.span.start, left.span.end).cmp(&(
            &right.source,
            right.file,
            right.span.start,
            right.span.end,
        ))
    });
}

impl Lowerer {
    pub(super) fn dependency_candidate_note(
        &self,
        target: hir::ImportedTarget,
    ) -> Result<ast::DiagnosticNote, String> {
        let origin = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.definition_origin(target))
            .ok_or_else(|| {
                format!("internal error: missing definition origin for imported target {target:?}")
            })?;
        let span = origin.span();
        let start = u32::try_from(span.start_byte())
            .map_err(|_| "imported declaration span exceeds compiler source offsets".to_owned())?;
        let end = u32::try_from(span.end_byte())
            .map_err(|_| "imported declaration span exceeds compiler source offsets".to_owned())?;
        let mut note =
            ast::DiagnosticNote::at(0, ast::Span::new(start, end), "candidate declared here");
        // A canonical source supersedes the stage-local file index.
        note.source = Some(Box::new(origin.source().clone()));
        Ok(note)
    }

    pub(super) fn type_candidate_note(
        &self,
        candidate: &TypeLookupCandidate,
    ) -> Result<ast::DiagnosticNote, String> {
        let (file, span) = match candidate.origin {
            TypeLookupOrigin::CurrentUnit(binding) => {
                let binding = self.imports.binding(binding);
                (binding.file, binding.span)
            }
            TypeLookupOrigin::ExistingM22Core => match &candidate.target {
                TypeLookupTarget::Dependency(_) => {
                    unreachable!("the M22 core lookup layer contains only current HIR targets")
                }
                TypeLookupTarget::Current(target) => match *target {
                    TopLevelTypeTarget::Alias(id) => {
                        let origin = self.source_type_aliases[id].origin;
                        (origin.file as usize, origin.span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)) => {
                        (self.struct_files[&id], self.structs[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)) => {
                        (self.enum_files[&id], self.enums[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Class(id)) => {
                        (self.class_files[&id], self.classes[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)) => {
                        (self.interface_files[&id], self.interfaces[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                        (self.object_files[&id], self.objects[id].span)
                    }
                },
            },
            TypeLookupOrigin::Dependency => {
                return match &candidate.target {
                    TypeLookupTarget::Dependency(binding) => {
                        self.dependency_candidate_note(binding.target())
                    }
                    TypeLookupTarget::Current(_) => Err(
                        "internal error: imported type candidate has a current target".to_owned(),
                    ),
                };
            }
        };
        Ok(ast::DiagnosticNote::at(
            file,
            span,
            "candidate declared here",
        ))
    }
}
