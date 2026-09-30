//! M21 top-level logical properties plus M12 raw/TLS storage and C data imports.

use scoop_ast as ast;
use scoop_hir as hir;
use std::path::{Component, Path};

use crate::call_resolution::applicability::NominalApplicabilityInput;
use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::{
    FnSig, ForbiddenSuspendContext, Function, FunctionKind, Lowerer, SuspensionContext,
    VariantStyle,
};

mod constant_images;
mod consts;
mod delegates;
mod extensions;
mod initialization;
mod static_initializers;
mod storage;

pub(crate) use consts::{evaluate_hir_integer_constant, integer_wrapping_neg};

struct PendingConst<'a> {
    import_source: crate::imports::PropertyImportSource,
    declaration: &'a ast::PropertyDecl,
    file: usize,
    access: hir::DeclarationAccess,
    ty: hir::TypeId,
    owner: hir::PropertyOwner,
}

struct PendingOrdinary<'a> {
    import_source: crate::imports::PropertyImportSource,
    declaration: &'a ast::PropertyDecl,
    file: usize,
    access: hir::DeclarationAccess,
    ty: hir::TypeId,
}

#[derive(Clone)]
pub(crate) struct PendingRuntimeInitializer {
    pub(crate) unit: hir::InitializationUnitId,
    pub(crate) function: hir::FunctionId,
    pub(crate) file: usize,
    pub(crate) span: ast::Span,
    pub(crate) kind: PendingRuntimeInitializerKind,
}

#[derive(Clone)]
pub(crate) enum PendingRuntimeInitializerKind {
    Stored {
        storage: hir::GlobalId,
        ty: hir::TypeId,
        expression: ast::Expr,
    },
    Delegated {
        property: hir::PropertyId,
        storage: PendingDelegateStorage,
        expression: ast::Expr,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum PendingDelegateStorage {
    Global {
        storage: hir::GlobalId,
        delegate: hir::DelegateStorageId,
    },
    Generic(hir::GenericDelegateTemplateId),
}

impl Lowerer {
    pub(crate) fn resolve_globals(
        &mut self,
        pending: &[(&ast::GlobalDecl, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        let mut initializers = Vec::new();
        let mut constants = Vec::new();
        let mut ordinary = Vec::new();
        for (declaration_index, &(decl, file_index)) in pending.iter().enumerate() {
            let import_source = self.imports.global_property_source(declaration_index);
            self.current_file = file_index;
            let access =
                self.top_level_access(decl.visibility, decl.name.span, "property", file_index);
            if decl.receiver_ty.is_some() {
                self.declare_extension_property(decl, file_index, access, import_source);
                continue;
            }
            let duplicate = self
                .top_level_namespaces
                .properties_in_declaration_scope(file_index, &decl.name.text)
                .into_iter()
                .any(|other| {
                    access.declared != hir::DeclaredVisibility::Private
                        || self.properties[other].access.declared
                            != hir::DeclaredVisibility::Private
                        || self.property_files[&other] == file_index
                })
                || constants.iter().any(|other: &PendingConst<'_>| {
                    self.top_level_namespaces
                        .sources_share_namespace(file_index, other.file)
                        && other.declaration.name.text == decl.name.text
                        && (access.declared != hir::DeclaredVisibility::Private
                            || other.access.declared != hir::DeclaredVisibility::Private
                            || other.file == file_index)
                })
                || ordinary.iter().any(|other: &PendingOrdinary<'_>| {
                    self.top_level_namespaces
                        .sources_share_namespace(file_index, other.file)
                        && other.declaration.name.text == decl.name.text
                        && (access.declared != hir::DeclaredVisibility::Private
                            || other.access.declared != hir::DeclaredVisibility::Private
                            || other.file == file_index)
                });
            if duplicate {
                self.error(
                    decl.name.span,
                    format!("duplicate global `{}`", decl.name.text),
                );
                continue;
            }
            self.type_params_in_scope.clear();
            let ty = self
                .resolve_type_ref(&decl.ty)
                .unwrap_or_else(|| self.integer_type(hir::IntegerKind::SIGNED_32));
            if matches!(decl.body, ast::PropertyBodySyntax::Const(_)) {
                if decl.mutable {
                    self.error(decl.name.span, "const property must be a `val`".to_string());
                    continue;
                }
                if decl.modifier != ast::MethodModifier::Final || decl.is_override {
                    self.error(
                        decl.span,
                        "top-level const properties cannot be open, abstract, or override"
                            .to_string(),
                    );
                    continue;
                }
                if !decl.type_params.is_empty() {
                    self.error(
                        decl.span,
                        "const properties cannot declare type parameters".to_string(),
                    );
                    continue;
                }
                self.reject_logical_property_annotations("a const property", &decl.annotations);
                if !matches!(
                    self.types[ty],
                    hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String
                ) {
                    self.error(
                        decl.ty.span,
                        format!(
                            "const property `{}` has unsupported type {}",
                            decl.name.text,
                            self.type_name(ty)
                        ),
                    );
                    continue;
                }
                constants.push(PendingConst {
                    import_source,
                    declaration: decl,
                    file: file_index,
                    access,
                    ty,
                    owner: hir::PropertyOwner::TopLevel,
                });
                continue;
            }
            if matches!(decl.body, ast::PropertyBodySyntax::Computed(_)) {
                if decl.modifier != ast::MethodModifier::Final || decl.is_override {
                    self.error(
                        decl.span,
                        "top-level computed properties cannot be open, abstract, or override"
                            .to_string(),
                    );
                    continue;
                }
                if !decl.type_params.is_empty() {
                    self.error(
                        decl.span,
                        "ordinary top-level properties cannot declare type parameters".to_string(),
                    );
                    continue;
                }
                self.reject_logical_property_annotations(
                    "a top-level computed property",
                    &decl.annotations,
                );
                let expected_property = self.next_property_id();
                let Some(capability) = self.allocate_property_accessors(
                    expected_property,
                    hir::PropertyOwner::TopLevel,
                    access.clone(),
                    decl,
                    None,
                    hir::MethodModifier::Final,
                ) else {
                    continue;
                };
                let property = self.properties.alloc(hir::Property {
                    owner: hir::PropertyOwner::TopLevel,
                    name: decl.name.text.clone(),
                    access,
                    modifier: hir::MethodModifier::Final,
                    is_override: false,
                    overrides: Vec::new(),
                    ty,
                    capability,
                    representation: hir::PropertyRepresentation::AccessorOnly,
                    span: decl.span,
                });
                assert_eq!(property, expected_property);
                self.top_level_namespaces.register_property(
                    file_index,
                    decl.name.text.clone(),
                    property,
                    false,
                );
                self.property_files.insert(property, file_index);
                self.imports.bind_property(import_source, property);
                continue;
            }
            let raw_storage = matches!(decl.body, ast::PropertyBodySyntax::ExternStorage)
                || decl.annotations.iter().any(|annotation| {
                    matches!(
                        annotation.name.text.as_str(),
                        "Extern" | "Global" | "ThreadLocal"
                    )
                });
            if !raw_storage {
                match &decl.body {
                    ast::PropertyBodySyntax::Initializer { .. }
                    | ast::PropertyBodySyntax::OptionalOmitted
                    | ast::PropertyBodySyntax::Delegated { .. } => {}
                    ast::PropertyBodySyntax::Abstract => {
                        self.error(
                            decl.span,
                            "top-level properties cannot be abstract".to_string(),
                        );
                        continue;
                    }
                    ast::PropertyBodySyntax::ExternStorage => {
                        unreachable!("extern storage is classified as raw storage")
                    }
                    ast::PropertyBodySyntax::Const(_) | ast::PropertyBodySyntax::Computed(_) => {
                        unreachable!("const and computed properties were handled above")
                    }
                }
                if decl.modifier != ast::MethodModifier::Final || decl.is_override {
                    self.error(
                        decl.span,
                        "ordinary top-level properties cannot be open, abstract, or override"
                            .to_string(),
                    );
                    continue;
                }
                if !decl.type_params.is_empty() {
                    self.error(
                        decl.span,
                        "ordinary top-level properties cannot declare type parameters".to_string(),
                    );
                    continue;
                }
                self.reject_logical_property_annotations(
                    "an ordinary top-level property",
                    &decl.annotations,
                );
                ordinary.push(PendingOrdinary {
                    import_source,
                    declaration: decl,
                    file: file_index,
                    access,
                    ty,
                });
                continue;
            }
            match &decl.body {
                ast::PropertyBodySyntax::Delegated { by_span, .. } => {
                    self.error(
                        *by_span,
                        "delegated properties cannot use raw or extern storage annotations"
                            .to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::Const(_) => unreachable!("handled above"),
                ast::PropertyBodySyntax::OptionalOmitted => {
                    self.error(
                        decl.span,
                        "Option shorthand cannot use raw or extern storage annotations".to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::Abstract => {
                    self.error(
                        decl.span,
                        "top-level properties cannot be abstract".to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::Initializer { accessors, .. }
                    if accessors.getter.is_some() || accessors.setter.is_some() =>
                {
                    self.error(
                        decl.span,
                        "raw or extern storage cannot declare property accessors".to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::Initializer { .. }
                | ast::PropertyBodySyntax::ExternStorage => {}
                ast::PropertyBodySyntax::Computed(_) => unreachable!("handled above"),
            }
            let checked = self.check_global_annotations(decl);
            let storage = if let Some(extern_) = checked.extern_ {
                hir::GlobalStorage::Extern {
                    library: extern_.library,
                    native_symbol: extern_.native_symbol,
                    thread_local: checked.storage.unwrap_or(false),
                }
            } else {
                let Some(initializer) = self.zero_constant_image(ty) else {
                    self.error(
                        decl.span,
                        format!(
                            "raw storage of type {} has no valid all-zero initial image",
                            self.type_name(ty)
                        ),
                    );
                    continue;
                };
                hir::GlobalStorage::Local {
                    thread_local: checked.storage.unwrap_or(false),
                    initializer,
                }
            };
            let expected_property = self.next_property_id();
            let id = self.globals.alloc(hir::Global {
                name: decl.name.text.clone(),
                property: expected_property,
                ty,
                mutable: decl.mutable,
                storage,
                span: decl.span,
            });
            let capability =
                self.allocate_storage_capability(access.clone(), decl.mutable, decl.span);
            let property = self.properties.alloc(hir::Property {
                owner: hir::PropertyOwner::TopLevel,
                name: decl.name.text.clone(),
                access,
                modifier: hir::MethodModifier::Final,
                is_override: false,
                overrides: Vec::new(),
                ty,
                capability,
                representation: hir::PropertyRepresentation::NativeStorage { storage: id },
                span: decl.span,
            });
            assert_eq!(property, expected_property);
            self.top_level_namespaces.register_property(
                file_index,
                decl.name.text.clone(),
                property,
                false,
            );
            self.property_files.insert(property, file_index);
            self.imports.bind_property(import_source, property);
            initializers.push((id, decl));
        }

        // Every global name and type is available before constants are
        // inspected. References to another global are nevertheless rejected:
        // initialization has no runtime ordering phase in M12.
        for (id, decl) in initializers {
            self.current_file = self.property_files[&self.globals[id].property];
            let global = self.globals[id].clone();
            match global.storage {
                hir::GlobalStorage::Managed { .. } => {
                    unreachable!("the M12 initializer worklist contains only raw storage")
                }
                hir::GlobalStorage::Extern { .. } => {
                    let property_name = self.properties[global.property].name.clone();
                    if let Err(reason) = self.validate_c_global_type(global.ty, &property_name) {
                        self.error(
                            decl.ty.span,
                            format!("extern global type is not C-FFI-safe: {reason}"),
                        );
                    }
                }
                hir::GlobalStorage::Local { thread_local, .. } => {
                    if self.type_contains_param(global.ty) || !self.is_gc_free(global.ty) {
                        self.error(
                            decl.ty.span,
                            format!(
                                "global storage requires a concrete GC-free value type, found {}",
                                self.type_name(global.ty)
                            ),
                        );
                    }
                    if let Some(expr) = decl.initializer() {
                        if let Some(initializer) = self.global_constant(expr, global.ty) {
                            self.globals[id].storage = hir::GlobalStorage::Local {
                                thread_local,
                                initializer,
                            };
                        } else {
                            self.error(
                                expr.span(),
                                "global initializer must be a GC-free compile-time constant and must not call functions or read another global"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        for &(object, source, file) in objects {
            self.current_file = file;
            self.current_owner = Some(crate::Owner::Object(object));
            for (member_index, property) in
                source
                    .members()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, member)| match member {
                        ast::ClassMember::StoredProperty(property)
                            if matches!(property.body, ast::PropertyBodySyntax::Const(_)) =>
                        {
                            Some((index, property))
                        }
                        _ => None,
                    })
            {
                let access = self.member_access(
                    property.visibility,
                    property.name.span,
                    "const property",
                    crate::Owner::Object(object),
                    file,
                    crate::visibility::MemberSlotAccess::None,
                );
                let ty = self
                    .resolve_type_ref(&property.ty)
                    .unwrap_or_else(|| self.integer_type(hir::IntegerKind::SIGNED_32));
                if property.mutable {
                    self.error(
                        property.name.span,
                        "const property must be a `val`".to_string(),
                    );
                    continue;
                }
                if property.modifier != ast::MethodModifier::Final || property.is_override {
                    self.error(
                        property.span,
                        format!(
                            "{} const properties cannot be open, abstract, or override",
                            source.description()
                        ),
                    );
                    continue;
                }
                if property.receiver_ty.is_some() || !property.type_params.is_empty() {
                    self.error(
                        property.span,
                        format!(
                            "{} const properties cannot be extensions or declare type parameters",
                            source.description()
                        ),
                    );
                    continue;
                }
                self.reject_logical_property_annotations(
                    &format!("{} const property", source.article_description()),
                    &property.annotations,
                );
                if !matches!(
                    self.types[ty],
                    hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String
                ) {
                    self.error(
                        property.ty.span,
                        format!(
                            "const property `{}` has unsupported type {}",
                            property.name.text,
                            self.type_name(ty)
                        ),
                    );
                    continue;
                }
                constants.push(PendingConst {
                    import_source: self.imports.object_property_source(
                        object,
                        member_index,
                        !self.source_is_current_cone(file),
                    ),
                    declaration: property,
                    file,
                    access,
                    ty,
                    owner: hir::PropertyOwner::Object(object),
                });
            }
        }
        self.current_owner = None;
        self.resolve_const_properties(&constants, &ordinary);
        self.resolve_static_top_level_properties(&ordinary);
    }
}
