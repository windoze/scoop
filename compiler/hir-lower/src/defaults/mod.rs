//! Definition-site lowering of exported source-parameter interfaces.

use la_arena::Idx;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{FnParamCalling, Lowerer, Owner};

mod access;
mod instantiate;
mod lowering;

use lowering::has_explicit_default;

#[derive(Clone)]
pub(crate) struct LocalDefaultExpr {
    pub(crate) body: hir::ExportDefaultExpr,
    pub(crate) captures: Vec<hir::Capture>,
}

pub(crate) type LocalDefaultExprId = Idx<LocalDefaultExpr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultExprTemplateRef {
    Local(LocalDefaultExprId),
    Export(hir::ExportDefaultSourceId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InheritedDefaultSource {
    Local(LocalDefaultExprId),
    Export {
        expression: hir::ExportDefaultExprId,
        type_arguments: Vec<hir::TypeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceParameterOwner {
    Function(hir::FunctionId),
    StructConstructor(hir::StructConstructorId),
    ClassConstructor(hir::ClassConstructorId),
    VariantConstructor {
        enumeration: hir::EnumId,
        variant: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceParameterCalling {
    Required,
    Default(DefaultExprTemplateRef),
    Vararg {
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        omission: SourceVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceVarargOmission {
    EmptyArray,
    Default(DefaultExprTemplateRef),
}

#[derive(Clone)]
struct ParameterSource {
    name: ast::Ident,
    ty: hir::TypeId,
    calling: FnParamCalling,
}

#[derive(Clone)]
struct DefaultContext {
    type_parameters: Vec<hir::TypeParamDecl>,
    receiver: Option<(hir::TypeId, Option<Owner>)>,
    is_suspend: bool,
    safety: hir::Safety,
    callable_name: String,
}

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
            self.lower_function_parameter_interface(function);
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
            self.lower_function_parameter_interface(function);
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
                    type_parameters: self.structs[structure].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: self.structs[structure].name.clone(),
                };
                self.lower_parameter_interface(
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
                self.lower_constructor_parameter_interface(
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
                            type_parameters: self.classes[class].type_params.clone(),
                            receiver: None,
                            is_suspend: false,
                            safety: hir::Safety::Safe,
                            callable_name: self.classes[class].name.clone(),
                        };
                        self.lower_parameter_interface(
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
                self.lower_constructor_parameter_interface(
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
                let context = DefaultContext {
                    type_parameters: self.enums[enumeration].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: format!(
                        "{}.{}",
                        self.enums[enumeration].name, source_variant.name.text
                    ),
                };
                self.lower_parameter_interface(
                    hir::ExportParameterOwner::VariantConstructor {
                        enumeration,
                        variant: variant_index,
                    },
                    &sources,
                    &context,
                );
            }
        }
        self.inherit_override_parameter_interfaces(methods);
    }

    fn lower_constructor_parameter_interface(
        &mut self,
        owner: hir::ExportParameterOwner,
        source_parameters: &[ast::Param],
        parameters: Vec<hir::ConstructorParameter>,
        type_parameters: Vec<hir::TypeParamDecl>,
        callable_name: String,
    ) {
        let source_owner = match owner {
            hir::ExportParameterOwner::StructConstructor(constructor) => {
                SourceParameterOwner::StructConstructor(constructor)
            }
            hir::ExportParameterOwner::ClassConstructor(constructor) => {
                SourceParameterOwner::ClassConstructor(constructor)
            }
            _ => unreachable!("constructor helper receives a constructor owner"),
        };
        let callings = match source_owner {
            SourceParameterOwner::StructConstructor(constructor) => {
                self.struct_parameter_calling.get(&constructor)
            }
            SourceParameterOwner::ClassConstructor(constructor) => {
                self.class_parameter_calling.get(&constructor)
            }
            _ => unreachable!("constructor helper receives a constructor owner"),
        }
        .cloned()
        .unwrap_or_default();
        if source_parameters.len() != parameters.len() || parameters.len() != callings.len() {
            return;
        }
        let sources = source_parameters
            .iter()
            .zip(parameters.iter().zip(callings))
            .map(|(source, (parameter, calling))| ParameterSource {
                name: source.name.clone(),
                ty: parameter.ty,
                calling,
            })
            .collect::<Vec<_>>();
        self.lower_parameter_interface(
            owner,
            &sources,
            &DefaultContext {
                type_parameters,
                receiver: None,
                is_suspend: false,
                safety: hir::Safety::Safe,
                callable_name,
            },
        );
    }

    fn inherit_override_parameter_interfaces(
        &mut self,
        methods: &[(hir::FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(function, declaration, file, _) in methods {
            if !declaration.is_override {
                continue;
            }
            self.current_file = file;
            let parameter_count = self.signatures[&function].params.len();
            for index in 0..parameter_count {
                if self
                    .default_templates
                    .contains_key(&(SourceParameterOwner::Function(function), index as u32))
                {
                    continue;
                }
                let mut visiting = Vec::new();
                let mut sources = self.inherited_default_sources(function, index, &mut visiting);
                sources.sort();
                sources.dedup();
                match sources.as_slice() {
                    [] => {}
                    [source] => {
                        let source = self.record_inherited_default_source(source.clone());
                        self.default_templates.insert(
                            (SourceParameterOwner::Function(function), index as u32),
                            source,
                        );
                        self.patch_export_override_parameter(function, index, source);
                    }
                    _ => {
                        self.error(
                            declaration.params[index].name.span,
                            format!(
                                "override function `{}` inherits conflicting default expressions for parameter `{}`",
                                declaration.name.text, declaration.params[index].name.text
                            ),
                        );
                    }
                }
            }
        }
    }

    fn inherited_default_sources(
        &mut self,
        function: hir::FunctionId,
        parameter: usize,
        visiting: &mut Vec<hir::FunctionId>,
    ) -> Vec<InheritedDefaultSource> {
        if visiting.contains(&function) {
            return Vec::new();
        }
        if let Some(&source) = self
            .default_templates
            .get(&(SourceParameterOwner::Function(function), parameter as u32))
        {
            return vec![self.inherited_default_source(source)];
        }
        visiting.push(function);
        let parents = self
            .override_sources
            .get(&function)
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let mut sources = Vec::new();
        for parent in parents {
            let parent_parameters = self.signatures[&parent]
                .type_params
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>();
            let parent_arguments =
                self.override_default_type_arguments[&(function, parent)].clone();
            let bindings = parent_parameters
                .into_iter()
                .zip(parent_arguments)
                .collect::<Vec<_>>();
            for source in self.inherited_default_sources(parent, parameter, visiting) {
                sources.push(match source {
                    InheritedDefaultSource::Local(local) => InheritedDefaultSource::Local(local),
                    InheritedDefaultSource::Export {
                        expression,
                        type_arguments,
                    } => InheritedDefaultSource::Export {
                        expression,
                        type_arguments: type_arguments
                            .into_iter()
                            .map(|argument| self.instantiate_method_ty(argument, &bindings))
                            .collect(),
                    },
                });
            }
        }
        visiting.pop();
        sources
    }

    fn inherited_default_source(&self, source: DefaultExprTemplateRef) -> InheritedDefaultSource {
        match source {
            DefaultExprTemplateRef::Local(local) => InheritedDefaultSource::Local(local),
            DefaultExprTemplateRef::Export(source) => {
                let source = &self.export_default_sources[source];
                InheritedDefaultSource::Export {
                    expression: source.expression,
                    type_arguments: source.type_arguments.clone(),
                }
            }
        }
    }

    fn record_inherited_default_source(
        &mut self,
        source: InheritedDefaultSource,
    ) -> DefaultExprTemplateRef {
        match source {
            InheritedDefaultSource::Local(local) => DefaultExprTemplateRef::Local(local),
            InheritedDefaultSource::Export {
                expression,
                type_arguments,
            } => DefaultExprTemplateRef::Export(self.export_default_sources.alloc(
                hir::ExportDefaultSource {
                    expression,
                    type_arguments,
                },
            )),
        }
    }

    fn patch_export_override_parameter(
        &mut self,
        function: hir::FunctionId,
        index: usize,
        source: DefaultExprTemplateRef,
    ) {
        let DefaultExprTemplateRef::Export(source) = source else {
            unreachable!("exported overrides inherit only exported default templates")
        };
        let interface = self
            .source_parameter_interfaces
            .iter_mut()
            .find(|interface| interface.owner == hir::ExportParameterOwner::Function(function))
            .expect("every exported method has a source parameter interface");
        let parameter = &mut interface.parameters[index];
        parameter.calling = match parameter.calling {
            hir::ExportParameterCalling::Required { value_type } => {
                hir::ExportParameterCalling::Default { value_type, source }
            }
            hir::ExportParameterCalling::Vararg {
                parameter_type,
                omission: hir::ExportVarargOmission::EmptyArray,
            } => hir::ExportParameterCalling::Vararg {
                parameter_type,
                omission: hir::ExportVarargOmission::Default(source),
            },
            calling @ (hir::ExportParameterCalling::Default { .. }
            | hir::ExportParameterCalling::Vararg {
                omission: hir::ExportVarargOmission::Default(_),
                ..
            }) => calling,
        };
    }
}
