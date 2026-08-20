//! Extern annotation argument schema.

use scoop_ast as ast;
use scoop_hir as hir;

use super::ExternAnnotation;
use crate::Lowerer;

impl Lowerer {
    pub(crate) fn annotation_extern(
        &mut self,
        annotation: &ast::Annotation,
        source_name: &str,
    ) -> Option<ExternAnnotation> {
        let mut library = None;
        let mut native_symbol = None;
        let mut abi = None;
        let mut capture_errno = false;
        let mut positional = 0;
        for arg in &annotation.args {
            if arg
                .name
                .as_ref()
                .is_some_and(|name| name.text == "captureErrno")
                || (arg.name.is_none() && positional == 3)
            {
                if arg.name.is_none() {
                    positional += 1;
                }
                if capture_errno {
                    self.error(
                        arg.span,
                        "duplicate `@Extern` argument `captureErrno`".into(),
                    );
                }
                capture_errno = true;
                continue;
            }
            let (slot, expected) = match arg.name.as_ref().map(|name| name.text.as_str()) {
                Some("lib") => (&mut library, "lib"),
                Some("name") => (&mut native_symbol, "name"),
                Some("abi") => (&mut abi, "abi"),
                Some(name) => {
                    self.error(arg.span, format!("unknown argument `{name}` for `@Extern`"));
                    continue;
                }
                None if positional == 0 => {
                    positional += 1;
                    (&mut library, "lib")
                }
                None if positional == 1 => {
                    positional += 1;
                    (&mut native_symbol, "name")
                }
                None if positional == 2 => {
                    positional += 1;
                    (&mut abi, "abi")
                }
                None => {
                    self.error(
                        arg.span,
                        "`@Extern` accepts at most four arguments".to_string(),
                    );
                    continue;
                }
            };
            if slot.is_some() {
                self.error(
                    arg.span,
                    format!("duplicate `@Extern` argument `{expected}`"),
                );
                continue;
            }
            let ast::AnnotationLiteral::String(value) = &arg.value else {
                self.error(
                    arg.span,
                    format!("argument `{expected}` of `@Extern` must be a string"),
                );
                continue;
            };
            *slot = Some(value.clone());
        }

        let library = library.unwrap_or_default();
        if !library.is_empty()
            && (!library
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '+' | '.'))
                || library.starts_with('-'))
        {
            self.error(
                annotation.span,
                "`@Extern` library must be a logical library name, not a path or linker flag"
                    .to_string(),
            );
        }
        let native_symbol = native_symbol
            .filter(|symbol| !symbol.is_empty())
            .unwrap_or_else(|| source_name.to_string());
        if !is_c_identifier(&native_symbol) {
            self.error(
                annotation.span,
                format!("extern symbol `{native_symbol}` is not a portable C identifier"),
            );
        }
        let abi = match abi.as_deref().unwrap_or("c") {
            "c" => hir::ExternAbi::C(scoop_identity::CAbiCallMode::NativeSafe),
            "scoop" => hir::ExternAbi::Scoop,
            value => {
                self.error(
                    annotation.span,
                    format!("extern ABI `{value}` is not supported; expected `c` or `scoop`"),
                );
                return None;
            }
        };
        Some(ExternAnnotation {
            library,
            native_symbol,
            abi,
        })
    }
}

fn is_c_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
