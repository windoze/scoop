//! Failed import syntax occupies its original lookup layer without a target.

use super::*;
use crate::Lowerer;

impl CurrentUnitImports {
    pub(super) fn unavailable_import(
        &mut self,
        lowerer: &Lowerer,
        import: &ast::ImportSyntax,
    ) -> UnavailableImport {
        let (name, local_name, span) = match import {
            ast::ImportSyntax::Exact {
                selector,
                alias,
                span,
                ..
            } => {
                let name = alias.as_ref().map(|alias| &alias.name).unwrap_or_else(|| {
                    selector
                        .segments()
                        .last()
                        .expect("exact selector has a name")
                });
                (name.text.clone(), Some(name.text.clone()), *span)
            }
            ast::ImportSyntax::Star {
                namespace, span, ..
            } => (
                namespace
                    .segments()
                    .last()
                    .expect("star selector has a namespace")
                    .text
                    .clone(),
                None,
                *span,
            ),
        };
        let file = lowerer.current_file;
        let binding = CurrentUnitBindingId(self.bindings.len());
        self.bindings.push(CurrentUnitBinding {
            target: CurrentUnitTarget::UnavailableImport,
            source: lowerer.visibility_file(file),
            file,
            span,
            access: hir::EffectiveLookupDomain(
                lowerer.top_level_domain(hir::DeclaredVisibility::Private, file),
            ),
            name,
        });
        UnavailableImport {
            local_name,
            binding,
        }
    }
}
