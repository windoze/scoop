use la_arena::Idx;
use scoop_identity::{LexicalCallableParent, StructuralDefinitionPath};

use super::HirCallbackRegistrationIdentityInputs;
use super::error::HirCallbackRegistrationIdentityErrorDetail as Detail;
use crate::{
    Function, FunctionId, HirFunctionIdentity, HirPropertyAccessorFunction, HirSignatureBinder,
    LexicalDefinitionRoot,
};

#[derive(Clone)]
pub(super) struct CallbackSiteContext {
    pub(super) parent: LexicalCallableParent,
    pub(super) binders: Vec<HirSignatureBinder>,
}

pub(super) struct CallbackContextResolver<'a, 'input> {
    inputs: &'a HirCallbackRegistrationIdentityInputs<'input>,
    function_contexts: Vec<Option<CallbackSiteContext>>,
    visiting: Vec<bool>,
}

impl<'a, 'input> CallbackContextResolver<'a, 'input> {
    pub(super) fn new(inputs: &'a HirCallbackRegistrationIdentityInputs<'input>) -> Self {
        Self {
            function_contexts: vec![None; inputs.functions.len()],
            visiting: vec![false; inputs.functions.len()],
            inputs,
        }
    }

    pub(super) fn resolve(
        &mut self,
        root: LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
    ) -> Result<CallbackSiteContext, Detail> {
        if path.segments().is_empty() {
            return Err(Detail::InvalidDefinitionPath);
        }
        if let Some(parent) = self.immediate_parent(root, path)? {
            self.function_context(parent)
        } else {
            self.root_context(root)
        }
    }

    fn function_context(&mut self, function: FunctionId) -> Result<CallbackSiteContext, Detail> {
        let index = local_index(function);
        if index >= self.inputs.functions.len() {
            return Err(Detail::UnknownFunction);
        }
        if let Some(context) = &self.function_contexts[index] {
            return Ok(context.clone());
        }
        if std::mem::replace(&mut self.visiting[index], true) {
            return Err(Detail::CyclicLexicalParent);
        }

        let declaration = &self.inputs.functions[function];
        let identity = &self.inputs.function_identities[function];
        let context = match identity {
            HirFunctionIdentity::Source(identity) => {
                let own_count = usize::try_from(
                    identity
                        .declaration()
                        .duplicate_signature()
                        .type_parameter_count(),
                )
                .map_err(|_| Detail::TooManyBinders)?;
                let parameters = function_type_parameters(declaration);
                if own_count > parameters.len() {
                    return Err(Detail::InvalidBinderRelation);
                }
                let split = parameters.len() - own_count;
                let mut binders = if let Some(site) = self.local_function_site(function)? {
                    let outer = self.resolve(site.0, &site.1)?.binders;
                    if split != outer.len()
                        || parameters[..split]
                            .iter()
                            .zip(&outer)
                            .any(|(parameter, binder)| *parameter != binder.parameter)
                    {
                        return Err(Detail::InvalidBinderRelation);
                    }
                    outer
                } else {
                    binder_group(&parameters[..split])?
                };
                append_binder_group(&mut binders, &parameters[split..])?;
                CallbackSiteContext {
                    parent: identity.lexical_parent(),
                    binders,
                }
            }
            HirFunctionIdentity::PropertyAccessor(accessor) => {
                let parent = match accessor {
                    HirPropertyAccessorFunction::Getter(getter) => self
                        .inputs
                        .property_accessor_identities
                        .get_getter(*getter)
                        .map(|identity| LexicalCallableParent::accessor(identity.id())),
                    HirPropertyAccessorFunction::Setter(setter) => self
                        .inputs
                        .property_accessor_identities
                        .get_setter(*setter)
                        .map(|identity| LexicalCallableParent::accessor(identity.id())),
                }
                .ok_or(Detail::InvalidFunctionIdentity)?;
                CallbackSiteContext {
                    parent,
                    binders: binder_group(&function_type_parameters(declaration))?,
                }
            }
            HirFunctionIdentity::LexicalGenerated(record) => {
                let site = self
                    .generated_function_site(function)?
                    .ok_or(Detail::InvalidFunctionIdentity)?;
                let outer = self.resolve(site.0, &site.1)?;
                let parameters = function_type_parameters(declaration);
                if parameters.len() != outer.binders.len()
                    || parameters
                        .iter()
                        .zip(&outer.binders)
                        .any(|(parameter, binder)| *parameter != binder.parameter)
                {
                    return Err(Detail::InvalidBinderRelation);
                }
                CallbackSiteContext {
                    parent: LexicalCallableParent::from_generated_key(record.key())
                        .map_err(|_| Detail::InvalidFunctionIdentity)?,
                    binders: outer.binders,
                }
            }
            HirFunctionIdentity::Initialization { record, .. } => CallbackSiteContext {
                parent: LexicalCallableParent::from_generated_key(record.key())
                    .map_err(|_| Detail::InvalidFunctionIdentity)?,
                binders: binder_group(&function_type_parameters(declaration))?,
            },
            HirFunctionIdentity::DerivedEquality(_) => {
                return Err(Detail::InvalidFunctionIdentity);
            }
        };
        self.visiting[index] = false;
        self.function_contexts[index] = Some(context.clone());
        Ok(context)
    }

    fn root_context(&mut self, root: LexicalDefinitionRoot) -> Result<CallbackSiteContext, Detail> {
        match root {
            LexicalDefinitionRoot::Function(function) => self.function_context(function),
            LexicalDefinitionRoot::ClassConstructor(constructor) => {
                if local_index(constructor) >= self.inputs.class_constructors.len() {
                    return Err(Detail::InvalidDefinitionRoot);
                }
                let declaration = &self.inputs.class_constructors[constructor];
                if local_index(declaration.owner) >= self.inputs.type_inputs.classes.len() {
                    return Err(Detail::InvalidDefinitionRoot);
                }
                let identity = &self.inputs.constructor_identities[constructor];
                let parent = if let Some(record) = identity.source_record() {
                    LexicalCallableParent::constructor(record.id())
                } else if let Some(record) = identity.generated_record() {
                    LexicalCallableParent::from_generated_key(record.key())
                        .map_err(|_| Detail::InvalidDefinitionRoot)?
                } else {
                    return Err(Detail::InvalidDefinitionRoot);
                };
                Ok(CallbackSiteContext {
                    parent,
                    binders: binder_group(
                        &self.inputs.type_inputs.classes[declaration.owner]
                            .type_params
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>(),
                    )?,
                })
            }
            LexicalDefinitionRoot::StructConstructor(constructor) => {
                if local_index(constructor) >= self.inputs.struct_constructors.len() {
                    return Err(Detail::InvalidDefinitionRoot);
                }
                let declaration = &self.inputs.struct_constructors[constructor];
                if local_index(declaration.owner) >= self.inputs.type_inputs.structs.len() {
                    return Err(Detail::InvalidDefinitionRoot);
                }
                Ok(CallbackSiteContext {
                    parent: LexicalCallableParent::constructor(
                        self.inputs.constructor_identities[constructor].id(),
                    ),
                    binders: binder_group(
                        &self.inputs.type_inputs.structs[declaration.owner]
                            .type_params
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>(),
                    )?,
                })
            }
            LexicalDefinitionRoot::VariantConstructor(variant) => {
                let enumeration = variant.enumeration();
                if local_index(enumeration) >= self.inputs.type_inputs.enums.len()
                    || variant.local_index() as usize
                        >= self.inputs.type_inputs.enums[enumeration].variants.len()
                {
                    return Err(Detail::InvalidDefinitionRoot);
                }
                Ok(CallbackSiteContext {
                    parent: LexicalCallableParent::variant_constructor(
                        self.inputs.enum_member_identities[variant].id(),
                    ),
                    binders: binder_group(
                        &self.inputs.type_inputs.enums[enumeration]
                            .type_params
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>(),
                    )?,
                })
            }
        }
    }

    fn immediate_parent(
        &self,
        root: LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
    ) -> Result<Option<FunctionId>, Detail> {
        let mut candidate = None;
        for (_, declaration) in self.inputs.local_functions.iter() {
            let Some((source_function, source_root)) = declaration.source() else {
                continue;
            };
            consider_parent(
                root,
                path,
                source_function,
                source_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        for (_, declaration) in self.inputs.lambdas.iter() {
            consider_parent(
                root,
                path,
                declaration.function,
                declaration.definition_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        for (_, declaration) in self.inputs.anonymous_functions.iter() {
            consider_parent(
                root,
                path,
                declaration.function,
                declaration.definition_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        Ok(candidate.map(|(_, function)| function))
    }

    fn local_function_site(
        &self,
        function: FunctionId,
    ) -> Result<Option<(LexicalDefinitionRoot, StructuralDefinitionPath)>, Detail> {
        unique_function_site(
            self.inputs
                .local_functions
                .iter()
                .filter_map(|(_, declaration)| {
                    let (source, root) = declaration.source()?;
                    (source == function).then_some((root, declaration.definition_path.clone()))
                }),
        )
    }

    fn generated_function_site(
        &self,
        function: FunctionId,
    ) -> Result<Option<(LexicalDefinitionRoot, StructuralDefinitionPath)>, Detail> {
        unique_function_site(
            self.inputs
                .lambdas
                .iter()
                .filter_map(|(_, declaration)| {
                    (declaration.function == function).then_some((
                        declaration.definition_root,
                        declaration.definition_path.clone(),
                    ))
                })
                .chain(
                    self.inputs
                        .anonymous_functions
                        .iter()
                        .filter_map(|(_, declaration)| {
                            (declaration.function == function).then_some((
                                declaration.definition_root,
                                declaration.definition_path.clone(),
                            ))
                        }),
                ),
        )
    }
}

fn unique_function_site(
    sites: impl Iterator<Item = (LexicalDefinitionRoot, StructuralDefinitionPath)>,
) -> Result<Option<(LexicalDefinitionRoot, StructuralDefinitionPath)>, Detail> {
    let mut found = None;
    for site in sites {
        if found.is_some_and(|found| found != site) {
            return Err(Detail::AmbiguousLexicalParent);
        }
        found = Some(site);
    }
    Ok(found)
}

fn consider_parent(
    root: LexicalDefinitionRoot,
    path: &StructuralDefinitionPath,
    possible_parent: FunctionId,
    possible_root: LexicalDefinitionRoot,
    possible_path: &StructuralDefinitionPath,
    candidate: &mut Option<(usize, FunctionId)>,
) -> Result<(), Detail> {
    let segments = path.segments();
    let possible_segments = possible_path.segments();
    if possible_root != root
        || possible_segments.len() >= segments.len()
        || !segments.starts_with(possible_segments)
    {
        return Ok(());
    }
    match candidate {
        Some((length, parent)) if *length == possible_segments.len() => {
            if *parent != possible_parent {
                return Err(Detail::AmbiguousLexicalParent);
            }
        }
        Some((length, _)) if *length > possible_segments.len() => {}
        _ => *candidate = Some((possible_segments.len(), possible_parent)),
    }
    Ok(())
}

fn function_type_parameters(function: &Function) -> Vec<crate::TypeParamId> {
    function
        .type_params()
        .into_iter()
        .map(|parameter| parameter.id)
        .collect()
}

fn binder_group(parameters: &[crate::TypeParamId]) -> Result<Vec<HirSignatureBinder>, Detail> {
    parameters
        .iter()
        .copied()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(HirSignatureBinder {
                parameter,
                depth: 0,
                index: u32::try_from(index).map_err(|_| Detail::TooManyBinders)?,
            })
        })
        .collect()
}

fn append_binder_group(
    outer: &mut Vec<HirSignatureBinder>,
    inner: &[crate::TypeParamId],
) -> Result<(), Detail> {
    if inner.is_empty() {
        return Ok(());
    }
    for binder in outer.iter_mut() {
        binder.depth = binder
            .depth
            .checked_add(1)
            .ok_or(Detail::BinderDepthOverflow)?;
    }
    outer.extend(binder_group(inner)?);
    Ok(())
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
