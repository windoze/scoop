//! Registers all declaration inputs before preparing any default body.
use super::*;
use lowering::has_explicit_default;
mod constructors;
mod functions;
mod locals;

impl Lowerer {
    pub(crate) fn lower_export_parameter_interfaces(
        &mut self,
        functions: &[(hir::FunctionId, &ast::FunctionDecl, usize)],
        methods: &[(hir::FunctionId, &ast::FunctionDecl, usize, Owner)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
        classes: &[(hir::ClassId, &ast::ClassDecl, usize)],
        enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
    ) {
        for &(function, _, file) in functions {
            self.current_file = file;
            self.register_function_parameter_interface(function);
        }
        for &(function, declaration, file, _) in methods {
            self.current_file = file;
            if declaration.is_override && declaration.params.iter().any(has_explicit_default) {
                self.error(
                    declaration.name.span,
                    format!(
                        "override function `{}` cannot declare a new default expression",
                        declaration.name.text
                    ),
                );
            }
            self.register_function_parameter_interface(function);
        }
        for (function, _, _) in self.derived_encoding_methods.clone() {
            self.current_file = self.function_files[&function];
            self.register_function_parameter_interface(function);
        }
        for (function, _) in self.derived_decoding_methods.clone() {
            self.current_file = self.function_files[&function];
            self.register_function_parameter_interface(function);
        }
        for &(structure, declaration, file) in structs {
            self.current_file = file;
            if matches!(
                self.structs[structure].representation,
                hir::StructRepresentation::Declared(_)
            ) {
                let Some(&constructor) = self.structs[structure].constructors.first() else {
                    continue;
                };
                let Some(callings) = self.struct_parameter_calling.get(&constructor).cloned()
                else {
                    continue;
                };
                let fields = self.structs[structure].semantic_fields();
                if declaration.fields.len() != fields.len() || fields.len() != callings.len() {
                    continue;
                }
                let sources = declaration
                    .fields
                    .iter()
                    .zip(fields.iter().zip(callings))
                    .map(|(source, (field, calling))| ParameterSource {
                        name: source.name.clone(),
                        ty: field.ty,
                        calling,
                    })
                    .collect::<Vec<_>>();
                let context = DefaultContext {
                    definition_root: hir::LexicalDefinitionRoot::StructConstructor(constructor),
                    source_context: hir::SourceContextSubject::Constructor(
                        hir::SourceContextConstructor::Struct(constructor),
                    ),
                    type_parameters: self.structs[structure].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: self.structs[structure].name.clone(),
                };
                self.register_export_parameter_interface(
                    hir::ExportParameterOwner::StructConstructor(constructor),
                    &sources,
                    &context,
                );
            }
            let secondary_ids = self.structs[structure]
                .constructors
                .iter()
                .copied()
                .filter(|constructor| {
                    matches!(
                        self.struct_constructors[*constructor].kind,
                        hir::StructConstructorKind::Secondary { .. }
                    )
                })
                .collect::<Vec<_>>();
            for (constructor, source) in secondary_ids
                .into_iter()
                .zip(declaration.secondary_constructors())
            {
                self.register_constructor_parameter_interface(
                    hir::ExportParameterOwner::StructConstructor(constructor),
                    &source.params,
                    self.struct_constructors[constructor].parameters.clone(),
                    self.structs[structure].type_params.clone(),
                    self.structs[structure].name.clone(),
                );
            }
        }
        for &(class, declaration, file) in classes {
            self.current_file = file;
            if matches!(
                self.classes[class].representation,
                hir::ClassRepresentation::Declared
            ) {
                let primary =
                    self.classes[class]
                        .constructors
                        .iter()
                        .copied()
                        .find(|constructor| {
                            matches!(
                                self.class_constructors[*constructor].kind,
                                hir::ClassConstructorKind::Primary { .. }
                            )
                        });
                if let Some(primary) = primary
                    && let Some(callings) = self.class_parameter_calling.get(&primary).cloned()
                {
                    let parameters = &self.class_constructors[primary].parameters;
                    if declaration.constructor.len() == parameters.len()
                        && parameters.len() == callings.len()
                    {
                        let sources = declaration
                            .constructor
                            .iter()
                            .zip(parameters.iter().zip(callings))
                            .map(|(source, (parameter, calling))| ParameterSource {
                                name: source.name.clone(),
                                ty: parameter.ty,
                                calling,
                            })
                            .collect::<Vec<_>>();
                        let context = DefaultContext {
                            definition_root: hir::LexicalDefinitionRoot::ClassConstructor(primary),
                            source_context: hir::SourceContextSubject::Constructor(
                                hir::SourceContextConstructor::Class(primary),
                            ),
                            type_parameters: self.classes[class].type_params.clone(),
                            receiver: None,
                            is_suspend: false,
                            safety: self.class_constructors[primary].safety,
                            callable_name: self.classes[class].name.clone(),
                        };
                        self.register_export_parameter_interface(
                            hir::ExportParameterOwner::ClassConstructor(primary),
                            &sources,
                            &context,
                        );
                    }
                }
            }
            let secondary_ids = self.classes[class]
                .constructors
                .iter()
                .copied()
                .filter(|constructor| {
                    matches!(
                        self.class_constructors[*constructor].kind,
                        hir::ClassConstructorKind::Secondary { .. }
                    )
                })
                .collect::<Vec<_>>();
            for (constructor, source) in secondary_ids
                .into_iter()
                .zip(declaration.secondary_constructors())
            {
                self.register_constructor_parameter_interface(
                    hir::ExportParameterOwner::ClassConstructor(constructor),
                    &source.params,
                    self.class_constructors[constructor].parameters.clone(),
                    self.classes[class].type_params.clone(),
                    self.classes[class].name.clone(),
                );
            }
        }
        for &(enumeration, declaration, file) in enums {
            self.current_file = file;
            if declaration.variants.len() != self.enums[enumeration].variants.len() {
                continue;
            }
            for (variant_index, source_variant) in declaration.variants.iter().enumerate() {
                let variant_index = variant_index as u32;
                let Some(callings) = self
                    .variant_parameter_calling
                    .get(&(enumeration, variant_index))
                    .cloned()
                else {
                    continue;
                };
                let source_fields = match &source_variant.kind {
                    ast::VariantDeclKind::Unit | ast::VariantDeclKind::Positional(_) => None,
                    ast::VariantDeclKind::Named(fields)
                    | ast::VariantDeclKind::Constructor(fields) => Some(fields.as_slice()),
                };
                let fields = &self.enums[enumeration].variants[variant_index as usize].fields;
                if fields.len() != callings.len()
                    || source_fields.is_some_and(|source| source.len() != fields.len())
                {
                    continue;
                }
                let sources = fields
                    .iter()
                    .zip(callings)
                    .enumerate()
                    .map(|(index, (field, calling))| ParameterSource {
                        name: source_fields.map_or_else(
                            || ast::Ident {
                                text: field.name.clone(),
                                span: source_variant.span,
                            },
                            |source| source[index].name.clone(),
                        ),
                        ty: field.ty,
                        calling,
                    })
                    .collect::<Vec<_>>();
                let variant = hir::EnumVariantRef::checked(&self.enums, enumeration, variant_index)
                    .expect("a source enum variant index is checked against its declaration");
                let context = DefaultContext {
                    definition_root: hir::LexicalDefinitionRoot::VariantConstructor(variant),
                    source_context: hir::SourceContextSubject::Nominal(
                        hir::SourceContextNominal::Enum(enumeration),
                    ),
                    type_parameters: self.enums[enumeration].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: format!(
                        "{}.{}",
                        self.enums[enumeration].name, source_variant.name.text
                    ),
                };
                self.register_export_parameter_interface(
                    hir::ExportParameterOwner::VariantConstructor(variant),
                    &sources,
                    &context,
                );
            }
        }
        for (_, object) in self.objects.clone().iter() {
            let class = &self.classes[object.backing_class];
            for constructor in class.constructors.clone() {
                let context = DefaultContext {
                    definition_root: hir::LexicalDefinitionRoot::ClassConstructor(constructor),
                    source_context: hir::SourceContextSubject::Constructor(
                        hir::SourceContextConstructor::Class(constructor),
                    ),
                    type_parameters: self.classes[object.backing_class].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: object.name.clone(),
                };
                self.register_export_parameter_interface(
                    hir::ExportParameterOwner::ClassConstructor(constructor),
                    &[],
                    &context,
                );
            }
        }
        self.finish_export_parameter_interfaces();
    }
}
