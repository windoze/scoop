//! Resolve errno capture after the declaration surface is complete.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ExternResult;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn resolve_errno_annotations(
        &mut self,
        functions: &[crate::declarations::FunctionHeaderInput<'_>],
        globals: &[(&ast::GlobalDecl, usize)],
    ) -> bool {
        let mut valid = true;
        for &(function, declaration, file) in functions {
            let hir::FunctionKind::Extern(external) = self.functions[function].kind else {
                continue;
            };
            self.current_file = file;
            if !self.resolve_function_errno(external, declaration) {
                valid = false;
                if self.editing_declarations.is_some() {
                    self.declaration_surface
                        .mark_signature_unavailable(function, declaration);
                    self.fail_editing_source_declaration(declaration.span);
                }
            }
        }
        for &(declaration, file) in globals {
            self.current_file = file;
            let capture = self.capture_errno(&declaration.annotations);
            if capture == Some(false) {
                continue;
            }
            if capture == Some(true) {
                self.error(
                    declaration.span,
                    "`captureErrno = true` is not allowed on an extern variable".into(),
                );
            }
            valid = false;
            self.fail_editing_source_declaration(declaration.span);
        }
        valid
    }

    fn resolve_function_errno(
        &mut self,
        external: hir::ExternFunctionId,
        declaration: ast::FunctionHeaderRef<'_>,
    ) -> bool {
        let Some(capture) = self.capture_errno(&declaration.annotations) else {
            return false;
        };
        if !capture {
            return true;
        }
        if !self.extern_functions[external].abi.is_c() {
            self.error(
                declaration.span,
                "`captureErrno = true` requires a C ABI extern function".into(),
            );
            return false;
        }
        let scoop = *self.extern_functions[external].result.scoop_type();
        let native = match &self.types[scoop] {
            hir::Type::Tuple(elements)
                if elements.len() == 2
                    && matches!(
                        self.types[elements[1]],
                        hir::Type::Integer(hir::IntegerKind::SIGNED_32)
                    ) =>
            {
                elements[0]
            }
            _ => {
                self.error(
                    declaration.span,
                    "an errno-capturing extern must return `(R, Int)`".into(),
                );
                return false;
            }
        };
        self.extern_functions[external].result = ExternResult::CaptureErrno { native, scoop };
        true
    }

    fn capture_errno(&mut self, annotations: &[ast::Annotation]) -> Option<bool> {
        let Some(annotation) = annotations.iter().find(|item| item.name.is_ident("Extern")) else {
            return Some(false);
        };
        let mut positional = 0;
        let argument = annotation
            .args
            .iter()
            .find(|argument| match &argument.name {
                Some(name) => name.text == "captureErrno",
                None => {
                    positional += 1;
                    positional == 4
                }
            });
        let Some(argument) = argument else {
            return Some(false);
        };
        let value = self.annotation_constant(&argument.value, self.boolean, argument.span)?;
        let hir::CanonicalConstValueV1::Boolean(value) = value else {
            unreachable!("the annotation constant has the requested Boolean type")
        };
        Some(value.value())
    }
}
