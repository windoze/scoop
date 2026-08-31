//! M12 compiler-annotation schemas and typed attribute lowering.

use std::collections::HashSet;

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionTarget {
    TopLevel,
    Member(Owner),
    Local,
}

pub(crate) struct CheckedFunctionAnnotations {
    pub(crate) attributes: hir::FunctionAttributes,
    pub(crate) intrinsic: Option<String>,
}

impl Lowerer {
    pub(crate) fn check_function_annotations(
        &mut self,
        decl: &ast::FunctionDecl,
        is_core: bool,
        target: FunctionTarget,
    ) -> CheckedFunctionAnnotations {
        let mut attributes = hir::FunctionAttributes::default();
        let mut intrinsic = None;
        let mut saw_safe = false;
        let mut saw_unsafe = false;
        let mut saw_no_gc = false;
        let mut saw_calling_convention = false;
        let mut seen = HashSet::new();

        for annotation in &decl.annotations {
            let name = annotation.name.text.as_str();
            if !seen.insert(name.to_string()) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
                continue;
            }
            match name {
                "Intrinsic" => {
                    let Some(value) = self.annotation_string(annotation, "name") else {
                        continue;
                    };
                    if !matches!(target, FunctionTarget::TopLevel) {
                        self.error(
                            annotation.span,
                            "`@Intrinsic` is not allowed on this function target".to_string(),
                        );
                    } else if !is_core {
                        self.error(
                            annotation.span,
                            "`@Intrinsic` is only allowed in the core library".to_string(),
                        );
                    } else if hir::intrinsic_spec(&value).is_none() {
                        self.error(annotation.span, format!("unknown intrinsic `{value}`"));
                    } else {
                        intrinsic = Some(value);
                    }
                }
                "NoGC" => {
                    if self.annotation_marker(annotation) {
                        saw_no_gc = true;
                        attributes.gc_effect = hir::GcEffect::NoGc;
                    }
                }
                "Unsafe" => {
                    if self.annotation_marker(annotation) {
                        saw_unsafe = true;
                        attributes.safety = hir::Safety::Unsafe;
                    }
                }
                "Safe" => {
                    if self.annotation_marker(annotation) {
                        saw_safe = true;
                        attributes.safety = hir::Safety::Safe;
                    }
                }
                "CallingConvention" => {
                    saw_calling_convention = true;
                    let Some(value) = self.annotation_string(annotation, "name") else {
                        continue;
                    };
                    if value != "cdecl" {
                        self.error(
                            annotation.span,
                            format!(
                                "calling convention `{value}` is not supported; M12 supports only `cdecl`"
                            ),
                        );
                    }
                }
                "Extern" => {
                    // Extern entities and their ABI are introduced by M12's
                    // dedicated extern-function implementation step. Keep the
                    // name out of the custom-annotation diagnostic while the
                    // effect baseline is independently usable.
                    self.error(
                        annotation.span,
                        "`@Extern` is not valid until an extern function entity can be produced"
                            .to_string(),
                    );
                }
                "CLayout" | "Global" | "ThreadLocal" | "InteriorMutable" => self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on a function"),
                ),
                _ => self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M12"),
                ),
            }
        }

        if saw_safe && saw_unsafe {
            self.error(
                decl.span,
                "`@Safe` and `@Unsafe` cannot be combined".to_string(),
            );
        }
        if saw_no_gc && decl.is_suspend {
            self.error(
                decl.span,
                "`@NoGC` cannot be used on a suspend function".to_string(),
            );
        }
        if saw_calling_convention {
            if !matches!(target, FunctionTarget::TopLevel) {
                self.error(
                    decl.span,
                    "`@CallingConvention` is only allowed on a top-level function".to_string(),
                );
            }
            if !saw_no_gc {
                self.error(
                    decl.span,
                    "`@CallingConvention` requires `@NoGC` on a non-extern function".to_string(),
                );
            }
            if decl.is_suspend {
                self.error(
                    decl.span,
                    "`@CallingConvention` cannot be used on a suspend function".to_string(),
                );
            }
        }
        if intrinsic.is_some() && decl.annotations.len() != 1 {
            self.error(
                decl.span,
                "this intrinsic does not allow effect annotations".to_string(),
            );
        }
        if intrinsic.is_some() && !matches!(decl.body, ast::FunctionBody::None) {
            self.error(
                decl.span,
                "`@Intrinsic` functions must not have a body (spec 13.1)".to_string(),
            );
        }

        CheckedFunctionAnnotations {
            attributes,
            intrinsic,
        }
    }

    pub(crate) fn check_struct_annotations(
        &mut self,
        decl: &ast::StructDecl,
    ) -> hir::StructAttributes {
        let mut attributes = hir::StructAttributes::default();
        let mut seen = HashSet::new();
        for annotation in &decl.annotations {
            let name = annotation.name.text.as_str();
            if !seen.insert(name.to_string()) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
                continue;
            }
            match name {
                "CLayout" => {
                    if let Some(layout) = self.annotation_c_layout(annotation) {
                        attributes.c_layout = Some(layout);
                    }
                }
                "InteriorMutable" => {
                    if self.annotation_marker(annotation) {
                        attributes.interior_mutable = true;
                    }
                }
                "Extern" | "NoGC" | "Unsafe" | "Safe" | "CallingConvention" | "Global"
                | "ThreadLocal" | "Intrinsic" => self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on a struct"),
                ),
                _ => self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M12"),
                ),
            }
        }
        attributes
    }

    pub(crate) fn reject_type_annotations(&mut self, kind: &str, annotations: &[ast::Annotation]) {
        let mut seen = HashSet::new();
        for annotation in annotations {
            let name = annotation.name.text.as_str();
            if !seen.insert(name.to_string()) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
            } else if is_core_annotation(name) {
                self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on {kind}"),
                );
            } else {
                self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M12"),
                );
            }
        }
    }

    fn annotation_marker(&mut self, annotation: &ast::Annotation) -> bool {
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

    fn annotation_string(
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

    fn annotation_c_layout(&mut self, annotation: &ast::Annotation) -> Option<hir::CLayout> {
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
}

fn is_core_annotation(name: &str) -> bool {
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
