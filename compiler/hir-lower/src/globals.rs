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

mod consts;
mod delegates;
mod static_initializers;

pub(crate) use consts::{evaluate_hir_integer_constant, integer_wrapping_neg};

struct PendingConst<'a> {
    declaration: &'a ast::PropertyDecl,
    file: usize,
    access: hir::DeclarationAccess,
    ty: hir::TypeId,
    owner: hir::PropertyOwner,
}

struct PendingOrdinary<'a> {
    declaration: &'a ast::PropertyDecl,
    file: usize,
    access: hir::DeclarationAccess,
    ty: hir::TypeId,
}

pub(crate) fn stable_source_identity(name: &str) -> String {
    let path = Path::new(name);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => components.push(value.to_string_lossy().into_owned()),
            Component::ParentDir => components.push("__parent__".to_string()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    if path.is_absolute() {
        components.pop().unwrap_or_else(|| "<user>".to_string())
    } else if components.is_empty() {
        "<user>".to_string()
    } else {
        components.join("/")
    }
}

#[derive(Clone)]
pub(crate) struct PendingRuntimeInitializer {
    pub(crate) unit: hir::InitializationUnitId,
    pub(crate) function: hir::FunctionId,
    pub(crate) storage: hir::GlobalId,
    pub(crate) file: usize,
    pub(crate) span: ast::Span,
    pub(crate) kind: PendingRuntimeInitializerKind,
}

#[derive(Clone)]
pub(crate) enum PendingRuntimeInitializerKind {
    Stored {
        ty: hir::TypeId,
        expression: ast::Expr,
    },
    Delegated {
        property: hir::PropertyId,
        delegate_storage: hir::DelegateStorageId,
        expression: ast::Expr,
    },
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
        for &(decl, file_index) in pending {
            self.current_file = file_index;
            let access =
                self.top_level_access(decl.visibility, decl.name.span, "property", file_index);
            if decl.receiver_ty.is_some() {
                self.declare_extension_property(decl, file_index, access);
                continue;
            }
            let duplicate = self
                .properties_by_name
                .get(&decl.name.text)
                .into_iter()
                .flatten()
                .copied()
                .any(|other| {
                    access.declared != hir::DeclaredVisibility::Private
                        || self.properties[other].access.declared
                            != hir::DeclaredVisibility::Private
                        || self.property_files[&other] == file_index
                })
                || constants.iter().any(|other: &PendingConst<'_>| {
                    other.declaration.name.text == decl.name.text
                        && (access.declared != hir::DeclaredVisibility::Private
                            || other.access.declared != hir::DeclaredVisibility::Private
                            || other.file == file_index)
                })
                || ordinary.iter().any(|other: &PendingOrdinary<'_>| {
                    other.declaration.name.text == decl.name.text
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
                    override_access: Vec::new(),
                    ty,
                    capability,
                    representation: hir::PropertyRepresentation::AccessorOnly,
                    span: decl.span,
                });
                assert_eq!(property, expected_property);
                self.properties_by_name
                    .entry(decl.name.text.clone())
                    .or_default()
                    .push(property);
                self.property_files.insert(property, file_index);
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
                override_access: Vec::new(),
                ty,
                capability,
                representation: hir::PropertyRepresentation::NativeStorage { storage: id },
                span: decl.span,
            });
            assert_eq!(property, expected_property);
            self.properties_by_name
                .entry(decl.name.text.clone())
                .or_default()
                .push(property);
            self.property_files.insert(property, file_index);
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
                    if let Err(reason) = self.validate_c_global_type(global.ty, &global.name) {
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
            for property in source.members().iter().filter_map(|member| match member {
                ast::ClassMember::StoredProperty(property)
                    if matches!(property.body, ast::PropertyBodySyntax::Const(_)) =>
                {
                    Some(property)
                }
                _ => None,
            }) {
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

    fn resolve_static_top_level_properties(&mut self, declarations: &[PendingOrdinary<'_>]) {
        for declaration in declarations {
            self.current_file = declaration.file;
            let initializer = match &declaration.declaration.body {
                ast::PropertyBodySyntax::Initializer { expression, .. } => {
                    self.static_property_constant(expression, declaration.ty)
                }
                ast::PropertyBodySyntax::OptionalOmitted => {
                    if !declaration.declaration.mutable || self.as_option(declaration.ty).is_none()
                    {
                        self.error(
                            declaration.declaration.span,
                            format!(
                                "property `{}` without an initializer must be a mutable Option property",
                                declaration.declaration.name.text
                            ),
                        );
                        None
                    } else {
                        self.static_none_constant(declaration.ty)
                    }
                }
                ast::PropertyBodySyntax::Const(_)
                | ast::PropertyBodySyntax::Computed(_)
                | ast::PropertyBodySyntax::Abstract
                | ast::PropertyBodySyntax::ExternStorage => {
                    unreachable!(
                        "the ordinary property worklist contains only stored or delegated properties"
                    )
                }
                ast::PropertyBodySyntax::Delegated { .. } => {
                    self.allocate_runtime_top_level_delegate(declaration);
                    continue;
                }
            };
            if let Some(initializer) = initializer {
                self.allocate_image_top_level_property(declaration, initializer);
            } else if let Some(expression) = declaration.declaration.initializer() {
                self.allocate_runtime_top_level_property(declaration, expression.clone());
            }
        }
    }

    fn allocate_image_top_level_property(
        &mut self,
        declaration: &PendingOrdinary<'_>,
        initializer: hir::HirConstantImage,
    ) {
        let expected_property = self.next_property_id();
        let expected_global = hir::GlobalId::from_raw((self.globals.len() as u32).into());
        let backing = hir::PropertyBacking::TopLevelGlobal {
            storage: expected_global,
            initialization: hir::TopLevelInitialization::Image,
        };
        let Some(capability) = self.allocate_property_accessors(
            expected_property,
            hir::PropertyOwner::TopLevel,
            declaration.access.clone(),
            declaration.declaration,
            Some(backing),
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let global = self.globals.alloc(hir::Global {
            name: declaration.declaration.name.text.clone(),
            property: expected_property,
            ty: declaration.ty,
            mutable: declaration.declaration.mutable,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: initializer,
                },
            },
            span: declaration.declaration.span,
        });
        assert_eq!(global, expected_global);
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: declaration.declaration.name.text.clone(),
            access: declaration.access.clone(),
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            override_access: Vec::new(),
            ty: declaration.ty,
            capability,
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty { backing }),
            span: declaration.declaration.span,
        });
        assert_eq!(property, expected_property);
        self.properties_by_name
            .entry(declaration.declaration.name.text.clone())
            .or_default()
            .push(property);
        self.property_files.insert(property, declaration.file);
    }

    fn allocate_runtime_top_level_property(
        &mut self,
        declaration: &PendingOrdinary<'_>,
        expression: ast::Expr,
    ) {
        let expected_property = self.next_property_id();
        let expected_global = hir::GlobalId::from_raw((self.globals.len() as u32).into());
        let expected_unit =
            hir::InitializationUnitId::from_raw((self.initialization_units.len() as u32).into());
        let failure_root =
            self.initialization_failure_roots
                .alloc(hir::InitializationFailureRoot {
                    unit: expected_unit,
                });
        let (initializer, ensure) = self.allocate_initialization_functions(
            expected_unit,
            declaration.declaration.span,
            declaration.file,
        );
        let stable_key = self.top_level_initialization_key(declaration);
        let unit = self.initialization_units.alloc(hir::InitializationUnit {
            stable_key,
            schedule: hir::InitializationSchedule::EagerStartup,
            kind: hir::InitializationUnitKind::EagerTopLevel {
                property: expected_property,
                storage: expected_global,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: declaration.declaration.span,
        });
        assert_eq!(unit, expected_unit);
        let backing = hir::PropertyBacking::TopLevelGlobal {
            storage: expected_global,
            initialization: hir::TopLevelInitialization::Runtime(unit),
        };
        let Some(capability) = self.allocate_property_accessors(
            expected_property,
            hir::PropertyOwner::TopLevel,
            declaration.access.clone(),
            declaration.declaration,
            Some(backing),
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let global = self.globals.alloc(hir::Global {
            name: declaration.declaration.name.text.clone(),
            property: expected_property,
            ty: declaration.ty,
            mutable: declaration.declaration.mutable,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { unit },
            },
            span: declaration.declaration.span,
        });
        assert_eq!(global, expected_global);
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: declaration.declaration.name.text.clone(),
            access: declaration.access.clone(),
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            override_access: Vec::new(),
            ty: declaration.ty,
            capability,
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty { backing }),
            span: declaration.declaration.span,
        });
        assert_eq!(property, expected_property);
        self.properties_by_name
            .entry(declaration.declaration.name.text.clone())
            .or_default()
            .push(property);
        self.property_files.insert(property, declaration.file);
        self.pending_runtime_initializers
            .push(PendingRuntimeInitializer {
                unit,
                function: initializer,
                storage: global,
                file: declaration.file,
                span: declaration.declaration.span,
                kind: PendingRuntimeInitializerKind::Stored {
                    ty: declaration.ty,
                    expression,
                },
            });
    }

    pub(crate) fn allocate_initialization_functions(
        &mut self,
        unit: hir::InitializationUnitId,
        span: ast::Span,
        file: usize,
    ) -> (hir::FunctionId, hir::FunctionId) {
        let allocate = |this: &mut Self, role: &str| {
            let function = this.functions.alloc(Function {
                name: format!("$init${role}${}", unit.into_raw()),
                access: this.local_declaration_access(),
                override_access: Vec::new(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: this.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: FunctionKind::User(hir::Body {
                    locals: la_arena::Arena::new(),
                    statements: Vec::new(),
                }),
                method: None,
                span,
            });
            this.function_files.insert(function, file);
            this.signatures.insert(
                function,
                FnSig {
                    is_suspend: false,
                    modifiers: hir::CallableModifiers::default(),
                    attributes: hir::FunctionAttributes::default(),
                    owner_type_param_count: 0,
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_ty: this.unit,
                },
            );
            this.top_level.push(function);
            function
        };
        (allocate(self, "body"), allocate(self, "ensure"))
    }

    fn top_level_initialization_key(&self, declaration: &PendingOrdinary<'_>) -> String {
        if declaration.access.declared == hir::DeclaredVisibility::Private {
            let source = stable_source_identity(&self.intrinsic_sources[declaration.file].name);
            format!(
                "top-level-private:{source}:{}",
                declaration.declaration.name.text
            )
        } else {
            format!("top-level:{}", declaration.declaration.name.text)
        }
    }

    pub(crate) fn lower_runtime_top_level_initializers(&mut self) {
        for pending in self.pending_runtime_initializers.clone() {
            self.current_file = pending.file;
            let outer_source_context = self.current_source_context;
            self.type_params_in_scope.clear();
            self.current_return_ty = self.unit;
            self.current_fn_name = self.functions[pending.function].name.clone();
            self.push_suspension_context(SuspensionContext::Forbidden(
                ForbiddenSuspendContext::Function,
            ));
            self.push_safety_context(hir::Safety::Safe);
            self.current_owner = None;
            self.current_this = None;
            self.set_source_context(self.current_fn_name.clone());
            self.push_scope();
            self.current_initialization_unit = Some(pending.unit);

            let mut statements = Vec::new();
            let mut sink = Vec::new();
            let value = match &pending.kind {
                PendingRuntimeInitializerKind::Stored { ty, expression } => {
                    self.lower_expr(expression, &mut sink, Some(*ty))
                        .and_then(|value| {
                            if self.is_subtype(value.ty, *ty) {
                                Some(self.adapt_to(value, *ty))
                            } else {
                                let expected = self.type_name(*ty);
                                let found = self.type_name(value.ty);
                                self.error(
                                    expression.span(),
                                    format!(
                                        "top-level property initializer must be of type {expected}, found {found}"
                                    ),
                                );
                                None
                            }
                        })
                }
                PendingRuntimeInitializerKind::Delegated {
                    property,
                    delegate_storage,
                    expression,
                } => self
                    .lower_expr(expression, &mut sink, None)
                    .and_then(|delegate| {
                        match self.resolve_delegate_role_call(
                            delegate.clone(),
                            hir::PropertyDelegateOperatorKind::ProvideDelegate,
                            Vec::new(),
                            pending.span,
                        ) {
                            crate::properties::DelegateRoleCall::Resolved(effective) => {
                                Some(effective.expression)
                            }
                            crate::properties::DelegateRoleCall::NoApplicable => Some(delegate),
                            crate::properties::DelegateRoleCall::Failed => None,
                        }
                    })
                    .inspect(|effective| {
                        self.globals[pending.storage].ty = effective.ty;
                        self.delegate_storages[*delegate_storage].ty = effective.ty;
                        debug_assert_eq!(
                            self.delegate_storages[*delegate_storage].property,
                            *property
                        );
                    }),
            };
            if let Some(value) = value {
                statements.extend(sink);
                statements.push(hir::Statement {
                    kind: hir::StatementKind::Assign {
                        target: hir::AssignTarget::Global(pending.storage),
                        value,
                    },
                    span: pending.span,
                });
            }
            self.current_initialization_unit = None;
            self.pop_scope();
            self.current_source_context = outer_source_context;
            self.pop_safety_context();
            self.pop_suspension_context();
            self.functions[pending.function].kind = FunctionKind::User(hir::Body {
                locals: std::mem::take(&mut self.locals),
                statements,
            });
        }
        self.diagnose_initialization_cycles();
    }

    fn diagnose_initialization_cycles(&mut self) {
        let mut states = vec![0_u8; self.initialization_units.len()];
        let mut stack = Vec::new();
        for raw in 0..self.initialization_units.len() {
            let unit = hir::InitializationUnitId::from_raw((raw as u32).into());
            self.visit_initialization_unit(unit, &mut states, &mut stack);
        }
    }

    fn visit_initialization_unit(
        &mut self,
        unit: hir::InitializationUnitId,
        states: &mut [u8],
        stack: &mut Vec<hir::InitializationUnitId>,
    ) {
        let index = unit.into_raw().into_u32() as usize;
        if states[index] != 0 {
            return;
        }
        states[index] = 1;
        stack.push(unit);
        for dependency in self.initialization_units[unit].dependencies.clone() {
            let dependency_index = dependency.unit.into_raw().into_u32() as usize;
            if states[dependency_index] == 0 {
                self.visit_initialization_unit(dependency.unit, states, stack);
            } else if states[dependency_index] == 1 {
                let start = stack
                    .iter()
                    .position(|candidate| *candidate == dependency.unit)
                    .expect("an active dependency is present in the DFS stack");
                let mut path = stack[start..]
                    .iter()
                    .map(|candidate| self.initialization_units[*candidate].stable_key.clone())
                    .collect::<Vec<_>>();
                path.push(
                    self.initialization_units[dependency.unit]
                        .stable_key
                        .clone(),
                );
                self.current_file = match self.initialization_units[unit].kind {
                    hir::InitializationUnitKind::EagerTopLevel { property, .. } => {
                        self.property_files[&property]
                    }
                    hir::InitializationUnitKind::LazySingleton { value, .. } => {
                        let object = self.singleton_values[value].declaration;
                        self.object_files[&object]
                    }
                };
                self.error(
                    dependency.span,
                    format!("initialization cycle: {}", path.join(" -> ")),
                );
            }
        }
        stack.pop();
        states[index] = 2;
    }

    fn zero_constant_image(&mut self, ty: hir::TypeId) -> Option<hir::HirConstantImage> {
        match self.types[ty].clone() {
            hir::Type::Integer(kind) => Some(hir::HirConstantImage::Integer(
                hir::HirIntegerConstant::from_magnitude(kind, 0, false)
                    .expect("zero is representable by every integer kind"),
            )),
            hir::Type::Boolean => Some(hir::HirConstantImage::Boolean(false)),
            hir::Type::Enum(_) => self.static_none_constant(ty),
            hir::Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                let fields = self.structs[application_value.template]
                    .semantic_fields()
                    .to_vec();
                let fields = fields
                    .into_iter()
                    .map(|field| {
                        let ty = self.instantiate_ty(field.ty, &application_value.arguments);
                        self.zero_constant_image(ty)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(hir::HirConstantImage::Struct {
                    application,
                    fields,
                })
            }
            hir::Type::Unit
            | hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Tuple(_)
            | hir::Type::Function(_)
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_)
            | hir::Type::Param(_) => None,
        }
    }

    fn static_none_constant(&self, ty: hir::TypeId) -> Option<hir::HirConstantImage> {
        let hir::Type::Enum(application) = self.types[ty] else {
            return None;
        };
        let application_value = &self.enum_applications[application];
        let option = self.option_core?;
        if application_value.template != option.enumeration() {
            return None;
        }
        let variant = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            application,
            option.none(),
        )
        .expect("the exact Option application belongs to its checked None variant");
        Some(hir::HirConstantImage::EnumUnit { variant })
    }

    fn static_unit_variant_constant(
        &self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        let ast::Expr::Var(name) = expression else {
            return None;
        };
        if self.scopes.lookup(&name.text).is_some()
            || self.available_capture(&name.text).is_some()
            || self.host_has_property(&name.text)
            || self.visible_property(&name.text, None).is_some()
        {
            return None;
        }
        let hir::Type::Enum(application) = self.types[expected] else {
            return None;
        };
        let application_value = &self.enum_applications[application];
        let prelude = self
            .core_prelude_variant_refs(&name.text)
            .iter()
            .copied()
            .filter(|target| target.enumeration() == application_value.template)
            .filter(|target| self.resolved_variant_style(*target) == VariantStyle::Unit)
            .collect::<Vec<_>>();
        let target = match prelude.as_slice() {
            [target] => Some(*target),
            [] => self.contextual_variant_ref(&name.text, Some(expected)),
            _ => None,
        }?;
        let variant = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            application,
            target,
        )?;
        (self.resolved_variant_style(target) == VariantStyle::Unit)
            .then_some(hir::HirConstantImage::EnumUnit { variant })
    }

    fn declare_extension_property(
        &mut self,
        declaration: &ast::PropertyDecl,
        file_index: usize,
        access: hir::DeclarationAccess,
    ) {
        if declaration.modifier != ast::MethodModifier::Final || declaration.is_override {
            self.error(
                declaration.span,
                "extension properties cannot be open, abstract, or override".to_string(),
            );
            return;
        }
        match &declaration.body {
            ast::PropertyBodySyntax::Computed(_) => {}
            ast::PropertyBodySyntax::Delegated { by_span, .. }
                if !declaration.type_params.is_empty() =>
            {
                self.error(
                    *by_span,
                    "generic extension properties cannot be delegated".to_string(),
                );
                return;
            }
            ast::PropertyBodySyntax::Delegated { .. } => {}
            _ => {
                self.error(
                    declaration.span,
                    format!(
                        "extension property `{}` must be computed or delegated",
                        declaration.name.text
                    ),
                );
                return;
            }
        }
        self.reject_logical_property_annotations("an extension property", &declaration.annotations);

        let mut type_params = Vec::new();
        for parameter in &declaration.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == parameter.name.text)
            {
                self.error(
                    parameter.span,
                    format!("duplicate type parameter `{}`", parameter.name.text),
                );
                continue;
            }
            let id = self.fresh_type_param(type_params.len());
            type_params.push(crate::lower_type_param_decl(parameter, id));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            0,
            &declaration.type_params,
            declaration.where_clause.as_ref(),
            "extension property",
        );
        self.type_params_in_scope = type_params.clone();
        let receiver_ty = declaration
            .receiver_ty
            .as_ref()
            .and_then(|receiver| self.resolve_type_ref(receiver));
        let property_ty = self.resolve_type_ref(&declaration.ty);
        self.type_params_in_scope.clear();
        let (Some(receiver_ty), Some(property_ty)) = (receiver_ty, property_ty) else {
            return;
        };

        let expected_property = self.next_property_id();
        let expected_extension =
            hir::ExtensionPropertyId::from_raw((self.extension_properties.len() as u32).into());
        let extension = self.extension_properties.alloc(hir::ExtensionProperty {
            property: expected_property,
            receiver_ty,
            type_params,
        });
        assert_eq!(extension, expected_extension);
        let (property, capability) =
            if matches!(declaration.body, ast::PropertyBodySyntax::Delegated { .. }) {
                self.allocate_runtime_extension_delegate(
                    declaration,
                    file_index,
                    access,
                    extension,
                    receiver_ty,
                    property_ty,
                )
            } else {
                let Some(capability) = self.allocate_property_accessors(
                    expected_property,
                    hir::PropertyOwner::Extension(extension),
                    access.clone(),
                    declaration,
                    None,
                    hir::MethodModifier::Final,
                ) else {
                    return;
                };
                let property = self.properties.alloc(hir::Property {
                    owner: hir::PropertyOwner::Extension(extension),
                    name: declaration.name.text.clone(),
                    access,
                    modifier: hir::MethodModifier::Final,
                    is_override: false,
                    overrides: Vec::new(),
                    override_access: Vec::new(),
                    ty: property_ty,
                    capability,
                    representation: hir::PropertyRepresentation::AccessorOnly,
                    span: declaration.span,
                });
                assert_eq!(property, expected_property);
                (property, capability)
            };
        self.extension_properties_by_name
            .entry(declaration.name.text.clone())
            .or_default()
            .push(property);
        self.property_files.insert(property, file_index);
        let getter = match self.property_getters[capability.getter()].implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("a computed extension property has a getter body")
            }
        };
        assert!(
            self.extension_property_by_getter
                .insert(getter, property)
                .is_none(),
            "an accessor function belongs to one logical property"
        );
    }

    pub(crate) fn check_extension_property_signatures(&mut self) {
        let mut property_groups = self
            .extension_properties_by_name
            .values()
            .cloned()
            .collect::<Vec<_>>();
        property_groups.sort_by_key(|properties| {
            properties
                .first()
                .expect("an extension-property name group is non-empty")
                .into_raw()
                .into_u32()
        });
        for properties in property_groups {
            for (index, property) in properties.iter().copied().enumerate() {
                let declaration = self.properties[property].clone();
                let hir::PropertyOwner::Extension(extension) = declaration.owner else {
                    unreachable!("the extension-property index contains only extensions")
                };
                let template = self.extension_properties[extension].clone();
                let getter =
                    match self.property_getters[declaration.capability.getter()].implementation {
                        hir::PropertyAccessorImplementation::Body(function) => function,
                        _ => unreachable!("extension properties have concrete getter bodies"),
                    };
                let mut receiver_bound = vec![false; self.signatures[&getter].type_params.len()];
                self.mark_type_params(self.extension_receivers[&getter], &mut receiver_bound);
                for (parameter, bound) in template.type_params.iter().zip(receiver_bound) {
                    if !bound {
                        self.current_file = self.property_files[&property];
                        self.error(
                            parameter.span,
                            format!(
                                "type parameter `{}` of extension property `{}` cannot be inferred solely from its receiver",
                                parameter.name, declaration.name
                            ),
                        );
                    }
                }

                let duplicate = properties[..index].iter().copied().any(|other| {
                    let other_declaration = &self.properties[other];
                    let other_getter = match self.property_getters
                        [other_declaration.capability.getter()]
                    .implementation
                    {
                        hir::PropertyAccessorImplementation::Body(function) => function,
                        _ => unreachable!("extension properties have concrete getter bodies"),
                    };
                    self.same_parameter_signature(getter, other_getter)
                        && (declaration.access.declared != hir::DeclaredVisibility::Private
                            || other_declaration.access.declared
                                != hir::DeclaredVisibility::Private
                            || self.property_files[&property] == self.property_files[&other])
                });
                if duplicate {
                    self.current_file = self.property_files[&property];
                    self.error(
                        declaration.span,
                        format!(
                            "extension property `{}` is already declared for the same receiver type",
                            declaration.name
                        ),
                    );
                }
            }
        }
    }

    fn global_constant(
        &mut self,
        expr: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        if let Some(variant) = self.static_unit_variant_constant(expr, expected) {
            return Some(variant);
        }
        match (self.types[expected].clone(), expr) {
            (hir::Type::Integer(_), ast::Expr::IntLiteral(literal)) => self
                .lower_integer_literal(*literal, Some(expected), false, literal.span)
                .and_then(|value| match value.kind {
                    hir::ExprKind::IntegerLiteral(value) => {
                        Some(hir::HirConstantImage::Integer(value))
                    }
                    _ => None,
                }),
            (
                hir::Type::Integer(_),
                ast::Expr::Unary {
                    op: ast::UnOp::Neg,
                    operand,
                    span,
                },
            ) => match &**operand {
                ast::Expr::IntLiteral(literal)
                    if matches!(
                        literal.suffix,
                        ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                    ) =>
                {
                    self.lower_integer_literal(*literal, Some(expected), true, *span)
                        .and_then(|value| match value.kind {
                            hir::ExprKind::IntegerLiteral(value) => {
                                Some(hir::HirConstantImage::Integer(value))
                            }
                            _ => None,
                        })
                }
                _ => None,
            },
            (hir::Type::Boolean, ast::Expr::BoolLiteral { value, .. }) => {
                Some(hir::HirConstantImage::Boolean(*value))
            }
            (hir::Type::Struct(application), ast::Expr::Call(call)) => {
                let application_value = self.struct_applications[application].clone();
                let struct_id = application_value.template;
                if self.global_struct_callee(call) != Some(struct_id) {
                    return None;
                }
                let constructor = self.struct_primary_constructor(struct_id)?;
                let view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                let (values, _) =
                    self.global_nominal_constant(&view, call, &application_value.arguments)?;
                Some(hir::HirConstantImage::Struct {
                    application,
                    fields: values,
                })
            }
            _ => None,
        }
    }

    fn global_struct_callee(&self, call: &ast::CallExpr) -> Option<hir::StructId> {
        self.structs_by_name
            .get(&call.callee.text)
            .map(|&(structure, _)| structure)
    }

    fn global_nominal_constant(
        &mut self,
        view: &NominalConstructorView,
        call: &ast::CallExpr,
        expected_arguments: &[hir::TypeId],
    ) -> Option<(Vec<hir::HirConstantImage>, Vec<hir::TypeId>)> {
        if expected_arguments.len() != view.owner_parameters.len() {
            return None;
        }
        let argument_map = CandidateArgumentMap::source_nominal(view, &call.args).ok()?;
        let explicit_arguments = self.resolve_call_type_args(&call.type_args)?;
        if !explicit_arguments.is_empty() && explicit_arguments.len() != view.owner_parameters.len()
        {
            return None;
        }
        let seed = if explicit_arguments.is_empty() {
            expected_arguments.to_vec()
        } else {
            explicit_arguments
                .iter()
                .zip(expected_arguments)
                .map(|(argument, &expected)| match argument {
                    crate::expr::ResolvedCallTypeArgument::Explicit { ty, .. } => *ty,
                    crate::expr::ResolvedCallTypeArgument::Infer { .. } => expected,
                })
                .collect()
        };
        let parameter_types = view
            .value_parameters
            .iter()
            .map(|parameter| self.instantiate_ty(parameter.ty, &seed))
            .collect::<Vec<_>>();
        let values = call
            .args
            .iter()
            .zip(&parameter_types)
            .map(|(argument, &parameter)| self.global_constant(&argument.expression, parameter))
            .collect::<Option<Vec<_>>>()?;
        let argument_types = parameter_types
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>();
        let solution = self
            .solve_nominal_applicability(NominalApplicabilityInput {
                view,
                argument_map: &argument_map,
                explicit_arguments: &explicit_arguments,
                expected_arguments: Some(expected_arguments),
                argument_types: &argument_types,
            })
            .ok()?;
        solution
            .iter()
            .zip(expected_arguments)
            .all(|(&actual, &expected)| self.types_equal(actual, expected))
            .then_some((values, solution))
    }
}
