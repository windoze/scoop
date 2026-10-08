//! Resolve errno capture after the declaration surface is complete.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ExternResult;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn resolve_errno_annotations(
        &mut self,
        functions: &[(hir::FunctionId, &ast::FunctionDecl, usize)],
        globals: &[(&ast::GlobalDecl, usize)],
    ) -> bool {
        let mut valid = true;
        for &(function, declaration, file) in functions {
            let hir::FunctionKind::Extern(external) = self.functions[function].kind else {
                continue;
            };
            self.current_file = file;
            let Some(capture) = self.capture_errno(&declaration.annotations) else {
                valid = false;
                continue;
            };
            if !capture {
                continue;
            }
            if !self.extern_functions[external].abi.is_c() {
                self.error(
                    declaration.span,
                    "`captureErrno = true` requires a C ABI extern function".into(),
                );
                valid = false;
                continue;
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
                    valid = false;
                    continue;
                }
            };
            self.extern_functions[external].result = ExternResult::CaptureErrno { native, scoop };
        }
        for &(declaration, file) in globals {
            self.current_file = file;
            match self.capture_errno(&declaration.annotations) {
                Some(false) => {}
                Some(true) => {
                    self.error(
                        declaration.span,
                        "`captureErrno = true` is not allowed on an extern variable".into(),
                    );
                    valid = false;
                }
                None => valid = false,
            }
        }
        valid
    }

    fn capture_errno(&mut self, annotations: &[ast::Annotation]) -> Option<bool> {
        let Some(annotation) = annotations.iter().find(|item| item.name.text == "Extern") else {
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
