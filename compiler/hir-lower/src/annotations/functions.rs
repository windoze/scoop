//! Function annotation validation and typed effect lowering.

use std::collections::HashSet;

use scoop_ast as ast;
use scoop_hir as hir;

use super::{CheckedFunctionAnnotations, FunctionTarget};
use crate::Lowerer;

impl Lowerer {
    pub(crate) fn check_function_annotations(
        &mut self,
        decl: &ast::FunctionDecl,
        target: FunctionTarget,
    ) -> CheckedFunctionAnnotations {
        let mut attributes = hir::FunctionAttributes::default();
        let mut intrinsic = None;
        let mut intrinsic_spec = None;
        let mut extern_ = None;
        let mut saw_safe = false;
        let mut saw_unsafe = false;
        let mut saw_no_gc = false;
        let mut gc_leaf = None;
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
                    let Some(spec) = hir::intrinsic_spec(&value) else {
                        self.error(annotation.span, format!("unknown intrinsic `{value}`"));
                        continue;
                    };
                    let target_matches = matches!(
                        (spec.target(), target),
                        (hir::IntrinsicTarget::TopLevel, FunctionTarget::TopLevel)
                            | (hir::IntrinsicTarget::Member, FunctionTarget::Member(_))
                    );
                    if !target_matches {
                        self.error(
                            annotation.span,
                            "`@Intrinsic` is not allowed on this function target".to_string(),
                        );
                    } else if !self.current_provider_may_declare_intrinsics() {
                        self.error(
                            annotation.span,
                            "`@Intrinsic` is only allowed in the core library".to_string(),
                        );
                    } else {
                        intrinsic = Some(hir::IntrinsicFunction {
                            kind: spec.kind(),
                            provider: self.current_intrinsic_provider(),
                        });
                        intrinsic_spec = Some(spec);
                    }
                }
                "NoGC" => {
                    if self.annotation_marker(annotation) {
                        saw_no_gc = true;
                        attributes.gc_effect = hir::GcEffect::NoGc;
                    }
                }
                "GCLeaf" => {
                    if self.annotation_marker(annotation) {
                        gc_leaf = Some(annotation.span);
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
                    extern_ = self.annotation_extern(annotation, &decl.name.text);
                }
                "CLayout" | "Global" | "ThreadLocal" | "InteriorMutable" => self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on a function"),
                ),
                _ => self.error(
                    annotation.span,
                    format!("user annotation `@{name}` is not allowed on a function"),
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
        if let Some(span) = gc_leaf {
            match &mut extern_ {
                Some(annotation) if annotation.abi.is_c() => {
                    annotation.abi = hir::ExternAbi::C(hir::CAbiCallMode::GcLeaf);
                }
                _ => self.error(
                    span,
                    "`@GCLeaf` requires a C ABI `@Extern` function".to_string(),
                ),
            }
        }
        if let Some(extern_annotation) = &extern_ {
            if !matches!(target, FunctionTarget::TopLevel) {
                self.error(
                    decl.span,
                    "`@Extern` is only allowed on a top-level function".to_string(),
                );
            }
            if decl.receiver_ty.is_some() {
                self.error(
                    decl.span,
                    "an `@Extern` function must not have an extension receiver".to_string(),
                );
            }
            if decl.is_suspend {
                self.error(
                    decl.span,
                    "`@Extern` cannot be used on a suspend function".to_string(),
                );
            }
            if !decl.type_params.is_empty() {
                self.error(
                    decl.span,
                    "an `@Extern` function must not be generic".to_string(),
                );
            }
            if !matches!(decl.body, ast::FunctionBody::None) {
                self.error(
                    decl.span,
                    "an `@Extern` function must not have a body".to_string(),
                );
            }
            match extern_annotation.abi {
                hir::ExternAbi::C(_) => {
                    if saw_no_gc {
                        self.error(
                            decl.span,
                            "a C ABI `@Extern` function cannot be marked `@NoGC`; use `@GCLeaf` for a no-transition call".to_string(),
                        );
                    }
                    if saw_safe {
                        self.error(
                            decl.span,
                            "a C ABI `@Extern` function cannot be marked `@Safe`".to_string(),
                        );
                    }
                    attributes.safety = hir::Safety::Unsafe;
                    attributes.gc_effect = hir::GcEffect::NoGc;
                }
                hir::ExternAbi::Scoop => {
                    attributes.safety = if saw_unsafe {
                        hir::Safety::Unsafe
                    } else {
                        hir::Safety::Safe
                    };
                }
            }
        }
        if saw_calling_convention {
            if !matches!(target, FunctionTarget::TopLevel) {
                self.error(
                    decl.span,
                    "`@CallingConvention` is only allowed on a top-level function".to_string(),
                );
            }
            if !saw_no_gc && extern_.is_none() {
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
        if let (Some(intrinsic), Some(spec)) = (intrinsic, intrinsic_spec) {
            if spec.effects() == hir::IntrinsicEffects::NONE
                && (saw_safe || saw_unsafe || saw_no_gc || saw_calling_convention)
            {
                self.error(
                    decl.span,
                    "this intrinsic does not allow effect annotations".to_string(),
                );
            } else if spec.effects().no_gc != saw_no_gc
                || spec.effects().unsafe_ != saw_unsafe
                || saw_safe
                || saw_calling_convention
            {
                let mut required = Vec::new();
                if spec.effects().no_gc {
                    required.push("`@NoGC`");
                }
                if spec.effects().unsafe_ {
                    required.push("`@Unsafe`");
                }
                self.error(
                    decl.span,
                    format!(
                        "intrinsic `{}` requires exactly {} effect annotation(s)",
                        intrinsic.kind.name(),
                        if required.is_empty() {
                            "no".to_string()
                        } else {
                            required.join(" and ")
                        }
                    ),
                );
            }
        }
        if intrinsic.is_some() && extern_.is_some() {
            self.error(
                decl.span,
                "`@Intrinsic` and `@Extern` cannot be combined".to_string(),
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
            extern_,
        }
    }
}
