//! M12 compiler-annotation schemas and typed attribute lowering.

use std::collections::HashSet;

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};

mod arguments;
mod constructors;
mod functions;

pub(crate) use arguments::is_core_annotation;

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

pub(crate) struct CheckedStructAnnotations {
    pub(crate) attributes: hir::StructAttributes,
    pub(crate) intrinsic: Option<&'static hir::IntrinsicTypeSpec>,
}

pub(crate) struct CheckedClassAnnotations {
    pub(crate) intrinsic: Option<&'static hir::IntrinsicTypeSpec>,
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
            if !extern_annotation.abi.is_c() {
                self.error(
                    decl.span,
                    "an extern global supports only the C data ABI".to_string(),
                );
            }
            if decl.initializer().is_some() {
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
            if decl.initializer().is_none() {
                self.error(
                    decl.span,
                    "a local global requires a compile-time constant initializer".to_string(),
                );
            }
        }

        CheckedGlobalAnnotations { extern_, storage }
    }

    pub(crate) fn check_struct_annotations(
        &mut self,
        decl: &ast::StructDecl,
    ) -> CheckedStructAnnotations {
        let mut attributes = hir::StructAttributes::default();
        let mut intrinsic = None;
        let mut seen = HashSet::new();
        for annotation in decl
            .annotations
            .iter()
            .filter(|annotation| is_core_annotation(&annotation.name.text))
        {
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
                "Intrinsic" => {
                    intrinsic = self.check_intrinsic_type_annotation(
                        annotation,
                        hir::IntrinsicTypeTarget::Struct,
                    );
                }
                "Extern" | "Unsafe" | "Safe" | "CallingConvention" | "Global" | "ThreadLocal" => {
                    self.error(
                        annotation.span,
                        format!("`@{name}` is not allowed on a struct"),
                    )
                }
                _ => self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M12"),
                ),
            }
        }
        if intrinsic.is_some() && decl.annotations.len() != 1 {
            self.error(
                decl.span,
                "`@Intrinsic` cannot be combined with other annotations on a type".to_string(),
            );
        }
        CheckedStructAnnotations {
            attributes,
            intrinsic,
        }
    }

    pub(crate) fn check_class_annotations(
        &mut self,
        decl: &ast::ClassDecl,
    ) -> CheckedClassAnnotations {
        let mut intrinsic = None;
        let mut seen = HashSet::new();
        for annotation in decl
            .annotations
            .iter()
            .filter(|annotation| is_core_annotation(&annotation.name.text))
        {
            let name = annotation.name.text.as_str();
            if !seen.insert(name.to_string()) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
                continue;
            }
            if name == "Intrinsic" {
                intrinsic = self
                    .check_intrinsic_type_annotation(annotation, hir::IntrinsicTypeTarget::Class);
            } else if is_core_annotation(name) {
                self.error(
                    annotation.span,
                    format!("`@{name}` is not allowed on a class"),
                );
            } else {
                self.error(
                    annotation.span,
                    format!("unsupported annotation `@{name}` in milestone M14"),
                );
            }
        }
        if intrinsic.is_some() && decl.annotations.len() != 1 {
            self.error(
                decl.span,
                "`@Intrinsic` cannot be combined with other annotations on a type".to_string(),
            );
        }
        CheckedClassAnnotations { intrinsic }
    }

    fn check_intrinsic_type_annotation(
        &mut self,
        annotation: &ast::Annotation,
        target: hir::IntrinsicTypeTarget,
    ) -> Option<&'static hir::IntrinsicTypeSpec> {
        let value = self.annotation_string(annotation, "name")?;
        let Some(spec) = hir::intrinsic_type_spec(&value) else {
            self.error(annotation.span, format!("unknown intrinsic type `{value}`"));
            return None;
        };
        if spec.kind.target() != target {
            self.error(
                annotation.span,
                format!(
                    "intrinsic type `{value}` requires a {} declaration",
                    match spec.kind.target() {
                        hir::IntrinsicTypeTarget::Struct => "struct",
                        hir::IntrinsicTypeTarget::Class => "class",
                    }
                ),
            );
            return None;
        }
        if !self.current_provider_may_declare_intrinsics() {
            self.error(
                annotation.span,
                "`@Intrinsic` is only allowed in the core library".to_string(),
            );
            return None;
        }
        Some(spec)
    }

    pub(crate) fn check_enum_annotations(&mut self, decl: &ast::EnumDecl) -> bool {
        let mut no_gc = false;
        let mut seen = HashSet::new();
        for annotation in decl
            .annotations
            .iter()
            .filter(|annotation| is_core_annotation(&annotation.name.text))
        {
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
        for annotation in annotations
            .iter()
            .filter(|annotation| is_core_annotation(&annotation.name.text))
        {
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

    pub(crate) fn reject_logical_property_annotations(
        &mut self,
        kind: &str,
        annotations: &[ast::Annotation],
    ) {
        let mut seen = HashSet::new();
        for annotation in annotations
            .iter()
            .filter(|annotation| is_core_annotation(&annotation.name.text))
        {
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
                    format!("unsupported annotation `@{name}` on {kind}"),
                );
            }
        }
    }
}
