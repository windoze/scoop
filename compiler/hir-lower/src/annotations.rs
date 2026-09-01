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
    pub(crate) intrinsic: Option<hir::IntrinsicFunction>,
    pub(crate) extern_: Option<ExternAnnotation>,
}

#[derive(Clone)]
pub(crate) struct ExternAnnotation {
    pub(crate) library: String,
    pub(crate) native_symbol: String,
    pub(crate) abi: hir::ExternAbi,
}

pub(crate) struct CheckedGlobalAnnotations {
    pub(crate) extern_: Option<ExternAnnotation>,
    /// `Some(false)` is `@Global`; `Some(true)` is `@ThreadLocal`.
    pub(crate) storage: Option<bool>,
}

impl Lowerer {
    pub(crate) fn check_global_annotations(
        &mut self,
        decl: &ast::GlobalDecl,
    ) -> CheckedGlobalAnnotations {
        let mut extern_ = None;
        let mut storage = None;
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
                "Extern" => extern_ = self.annotation_extern(annotation, &decl.name.text),
                "Global" | "ThreadLocal" => {
                    if self.annotation_marker(annotation) {
                        if storage.is_some() {
                            self.error(
                                annotation.span,
                                "`@Global` and `@ThreadLocal` cannot be combined".to_string(),
                            );
                        } else {
                            storage = Some(name == "ThreadLocal");
                        }
                    }
                }
                _ => self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on a top-level storage declaration"),
                ),
            }
        }

        if let Some(extern_annotation) = &extern_ {
            if extern_annotation.abi != hir::ExternAbi::C {
                self.error(
                    decl.span,
                    "an extern global supports only the C data ABI".to_string(),
                );
            }
            if decl.init.is_some() {
                self.error(
                    decl.span,
                    "an `@Extern` global must not have an initializer".to_string(),
                );
            }
            if decl.mutable && storage.is_none() {
                self.error(
                    decl.span,
                    "an extern `var` requires exactly one of `@Global` or `@ThreadLocal`"
                        .to_string(),
                );
            }
            if !decl.mutable && storage.is_some() {
                self.error(
                    decl.span,
                    "an extern `val` must not use `@Global` or `@ThreadLocal`".to_string(),
                );
            }
        } else {
            if !decl.mutable {
                self.error(
                    decl.span,
                    "a local top-level `val` is not supported; use a local binding or `@Extern val`"
                        .to_string(),
                );
            }
            if storage.is_none() {
                self.error(
                    decl.span,
                    "a local top-level `var` requires exactly one of `@Global` or `@ThreadLocal`"
                        .to_string(),
                );
            }
            if decl.init.is_none() {
                self.error(
                    decl.span,
                    "a local global requires a compile-time constant initializer".to_string(),
                );
            }
        }

        CheckedGlobalAnnotations { extern_, storage }
    }

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
                        (spec.target, target),
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
                            kind: spec.kind,
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
                hir::ExternAbi::C => {
                    if saw_safe {
                        self.error(
                            decl.span,
                            "a C ABI `@Extern` function cannot be marked `@Safe`".to_string(),
                        );
                    }
                    attributes.safety = hir::Safety::Unsafe;
                    // The generated storage bridge is a verified GC leaf.
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
            if spec.effects == hir::IntrinsicEffects::NONE
                && (saw_safe || saw_unsafe || saw_no_gc || saw_calling_convention)
            {
                self.error(
                    decl.span,
                    "this intrinsic does not allow effect annotations".to_string(),
                );
            } else if spec.effects.no_gc != saw_no_gc
                || spec.effects.unsafe_ != saw_unsafe
                || saw_safe
                || saw_calling_convention
            {
                let mut required = Vec::new();
                if spec.effects.no_gc {
                    required.push("`@NoGC`");
                }
                if spec.effects.unsafe_ {
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
                "NoGC" => {
                    if self.annotation_marker(annotation) {
                        attributes.no_gc = true;
                    }
                }
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
                "Extern" | "Unsafe" | "Safe" | "CallingConvention" | "Global" | "ThreadLocal"
                | "Intrinsic" => self.error(
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

    pub(crate) fn check_enum_annotations(&mut self, decl: &ast::EnumDecl) -> bool {
        let mut no_gc = false;
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
                "NoGC" => no_gc = self.annotation_marker(annotation),
                _ if is_core_annotation(name) => self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on an enum"),
                ),
                _ => self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M13"),
                ),
            }
        }
        no_gc
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
