//! Shared annotation argument schemas and primitive validators.

use scoop_ast as ast;
use scoop_hir as hir;

use super::ExternAnnotation;
use crate::Lowerer;

impl Lowerer {
    pub(super) fn annotation_marker(&mut self, annotation: &ast::Annotation) -> bool {
        if annotation.args.is_empty() {
            true
        } else {
            self.error(
                annotation.span,
                format!("`@{}` does not accept arguments", annotation.name.text),
            );
            false
        }
    }

    pub(super) fn annotation_string(
        &mut self,
        annotation: &ast::Annotation,
        parameter: &str,
    ) -> Option<String> {
        if annotation.args.len() != 1 {
            self.error(
                annotation.span,
                format!(
                    "`@{}` requires exactly one string argument `{parameter}`",
                    annotation.name.text
                ),
            );
            return None;
        }
        let arg = &annotation.args[0];
        if let Some(name) = &arg.name
            && name.text != parameter
        {
            self.error(
                name.span,
                format!(
                    "unknown argument `{}` for `@{}`; expected `{parameter}`",
                    name.text, annotation.name.text
                ),
            );
            return None;
        }
        let ast::AnnotationLiteral::String(value) = &arg.value else {
            self.error(
                arg.span,
                format!(
                    "argument `{parameter}` of `@{}` must be a string",
                    annotation.name.text
                ),
            );
            return None;
        };
        Some(value.clone())
    }

    pub(super) fn annotation_c_layout(
        &mut self,
        annotation: &ast::Annotation,
    ) -> Option<hir::CLayout> {
        let mut aligned = None;
        let mut packed = None;
        let mut positional = 0;
        for arg in &annotation.args {
            let slot = match arg.name.as_ref().map(|name| name.text.as_str()) {
                Some("aligned") => &mut aligned,
                Some("packed") => &mut packed,
                Some(name) => {
                    self.error(
                        arg.span,
                        format!("unknown argument `{name}` for `@CLayout`"),
                    );
                    continue;
                }
                None if positional == 0 => {
                    positional += 1;
                    &mut aligned
                }
                None if positional == 1 => {
                    positional += 1;
                    &mut packed
                }
                None => {
                    self.error(
                        arg.span,
                        "`@CLayout` accepts at most two arguments".to_string(),
                    );
                    continue;
                }
            };
            if slot.is_some() {
                self.error(arg.span, "duplicate `@CLayout` argument".to_string());
                continue;
            }
            let ast::AnnotationLiteral::Int(value) = arg.value else {
                self.error(
                    arg.span,
                    "`@CLayout` arguments must be integers".to_string(),
                );
                continue;
            };
            if !matches!(value, 0 | 1 | 2 | 4 | 8 | 16) {
                self.error(
                    arg.span,
                    "`@CLayout` alignment values must be one of 0, 1, 2, 4, 8 or 16".to_string(),
                );
                continue;
            }
            *slot = Some(value as u8);
        }
        Some(hir::CLayout {
            aligned: aligned.unwrap_or(0),
            packed: packed.unwrap_or(0),
        })
    }

    pub(crate) fn annotation_extern(
        &mut self,
        annotation: &ast::Annotation,
        source_name: &str,
    ) -> Option<ExternAnnotation> {
        let mut library = None;
        let mut native_symbol = None;
        let mut abi = None;
        let mut positional = 0;
        for arg in &annotation.args {
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
                        "`@Extern` accepts at most three arguments".to_string(),
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
            "c" => hir::ExternAbi::C,
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

pub(super) fn is_core_annotation(name: &str) -> bool {
    matches!(
        name,
        "Intrinsic"
            | "Extern"
            | "NoGC"
            | "Unsafe"
            | "Safe"
            | "CLayout"
            | "CallingConvention"
            | "Global"
            | "ThreadLocal"
            | "InteriorMutable"
    )
}
