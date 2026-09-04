//! M12 top-level local/TLS storage and C data imports.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::call_resolution::applicability::NominalApplicabilityInput;
use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};

impl Lowerer {
    pub(crate) fn resolve_globals(&mut self, pending: &[(&ast::GlobalDecl, usize)]) {
        let mut initializers = Vec::new();
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
                });
            if duplicate {
                self.error(
                    decl.name.span,
                    format!("duplicate global `{}`", decl.name.text),
                );
                continue;
            }
            self.type_params_in_scope.clear();
            let ty = self.resolve_type_ref(&decl.ty).unwrap_or(self.int);
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
            match &decl.body {
                ast::PropertyBodySyntax::Delegated { by_span, .. } => {
                    self.error(
                        *by_span,
                        "delegated properties require the M21 delegate protocol".to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::Const(_) => {
                    self.error(
                        decl.span,
                        "const property lowering requires the M21 top-level initialization gate"
                            .to_string(),
                    );
                    continue;
                }
                ast::PropertyBodySyntax::OptionalOmitted => {
                    self.error(
                        decl.span,
                        "ordinary top-level Option storage requires the M21 initialization gate"
                            .to_string(),
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
                hir::GlobalStorage::Local {
                    thread_local: checked.storage.unwrap_or(false),
                    initializer: hir::ConstantValue::Int(0),
                }
            };
            let expected_property = self.next_property_id();
            let id = self.globals.alloc(hir::Global {
                name: decl.name.text.clone(),
                property: expected_property,
                ty,
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
            ast::PropertyBodySyntax::Delegated { by_span, .. } => {
                self.error(
                    *by_span,
                    "delegated properties require the M21 delegate protocol".to_string(),
                );
                return;
            }
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
        let Some(capability) = self.allocate_property_accessors(
            expected_property,
            hir::PropertyOwner::Extension(expected_extension),
            access.clone(),
            declaration,
            None,
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let extension = self.extension_properties.alloc(hir::ExtensionProperty {
            property: expected_property,
            receiver_ty,
            type_params,
        });
        assert_eq!(extension, expected_extension);
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
        self.extension_properties_by_name
            .entry(declaration.name.text.clone())
            .or_default()
            .push(property);
        self.property_files.insert(property, file_index);
        let getter = match self.property_getters[capability.getter()].implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
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
    ) -> Option<hir::ConstantValue> {
        match (self.types[expected].clone(), expr) {
            (hir::Type::Int | hir::Type::UInt, ast::Expr::IntLiteral { value, .. }) => {
                Some(hir::ConstantValue::Int(*value))
            }
            (
                hir::Type::Int,
                ast::Expr::Unary {
                    op: ast::UnOp::Neg,
                    operand,
                    ..
                },
            ) => match &**operand {
                ast::Expr::IntLiteral { value, .. } => {
                    value.checked_neg().map(hir::ConstantValue::Int)
                }
                _ => None,
            },
            (hir::Type::Boolean, ast::Expr::BoolLiteral { value, .. }) => {
                Some(hir::ConstantValue::Bool(*value))
            }
            (hir::Type::Ptr(pointee), ast::Expr::Call(call)) => {
                let structure = self.global_struct_callee(call)?;
                if Some(structure) != self.ffi_ptr {
                    return None;
                }
                let constructor = self.struct_primary_constructor(structure)?;
                let view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                let (values, _) = self.global_nominal_constant(&view, call, &[pointee])?;
                matches!(values.as_slice(), [hir::ConstantValue::Int(0)])
                    .then_some(hir::ConstantValue::NullPtr)
            }
            (hir::Type::FunPtr(signature), ast::Expr::Call(call)) => {
                let structure = self.global_struct_callee(call)?;
                if Some(structure) != self.ffi_fun_ptr {
                    return None;
                }
                let constructor = self.struct_primary_constructor(structure)?;
                let mut view =
                    self.nominal_constructor_view(NominalConstructorSource::Struct(constructor));
                view.value_parameters.clear();
                let function_ty = self.function_types[signature].canonical_type;
                self.global_nominal_constant(&view, call, &[function_ty])?;
                Some(hir::ConstantValue::NullFunPtr)
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
                Some(hir::ConstantValue::Struct {
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
    ) -> Option<(Vec<hir::ConstantValue>, Vec<hir::TypeId>)> {
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
