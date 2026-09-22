use super::*;

impl FunctionIdentityBuilder<'_> {
    pub(super) fn context_for_site(
        &mut self,
        function: hir::FunctionId,
        root: hir::LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
    ) -> Result<CallableContext, PersistentFunctionIdentityError> {
        if let Some(parent) = self.immediate_parent(function, root, path)? {
            self.resolve(parent)?;
            return self.function_context(function, parent);
        }
        self.root_context(function, root)
    }

    pub(super) fn function_context(
        &mut self,
        function: hir::FunctionId,
        parent: hir::FunctionId,
    ) -> Result<CallableContext, PersistentFunctionIdentityError> {
        let identity = self.resolve(parent)?;
        match identity {
            hir::HirFunctionIdentity::Source(identity) => {
                let mut owners = identity.declaration().owners().owners().to_vec();
                owners.push(identity.definition_owner());
                Ok(CallableContext {
                    parent: identity.lexical_parent(),
                    owners,
                    binders: self.source_function_binders(function, parent)?,
                })
            }
            hir::HirFunctionIdentity::PropertyAccessor(accessor) => {
                self.accessor_context(function, accessor)
            }
            hir::HirFunctionIdentity::LexicalGenerated(record) => {
                let site = self
                    .lexical_site(parent)?
                    .ok_or_else(|| self.failure(function, Detail::InvalidLexicalRoot))?;
                let mut context = self.context_for_site(parent, site.root, &site.path)?;
                context.parent =
                    LexicalCallableParent::from_generated_key(record.key()).map_err(|error| {
                        self.failure(
                            function,
                            Detail::InvalidIdentity(hir::HirFunctionIdentityError::LexicalParent(
                                error,
                            )),
                        )
                    })?;
                context
                    .owners
                    .push(DefinitionOwnerAtom::GeneratedCallable(record.id()));
                Ok(context)
            }
            hir::HirFunctionIdentity::Initialization { unit, record, .. } => {
                let context = self.initialization_context(function, parent, unit)?;
                let parent =
                    LexicalCallableParent::from_generated_key(record.key()).map_err(|error| {
                        self.failure(
                            function,
                            Detail::InvalidIdentity(hir::HirFunctionIdentityError::LexicalParent(
                                error,
                            )),
                        )
                    })?;
                if context.parent == parent {
                    Ok(context)
                } else {
                    Err(self.failure(function, Detail::InvalidLexicalRoot))
                }
            }
            hir::HirFunctionIdentity::DerivedEquality(_) => {
                Err(self.failure(function, Detail::InvalidLexicalRoot))
            }
        }
    }

    pub(super) fn root_context(
        &mut self,
        function: hir::FunctionId,
        root: hir::LexicalDefinitionRoot,
    ) -> Result<CallableContext, PersistentFunctionIdentityError> {
        match root {
            hir::LexicalDefinitionRoot::Function(root) => {
                self.resolve(root)?;
                self.function_context(function, root)
            }
            hir::LexicalDefinitionRoot::ClassConstructor(root) => {
                if local_index(root) >= self.lowerer.class_constructors.len() {
                    return Err(self.failure(function, Detail::InvalidLexicalRoot));
                }
                let identity = &self.constructors[root];
                let constructor = &self.lowerer.class_constructors[root];
                let binders = self.binder_group(
                    function,
                    &self.lowerer.classes[constructor.owner].type_params,
                )?;
                if let Some(record) = identity.source_record() {
                    let mut owners = record.key().owners().owners().to_vec();
                    owners.push(DefinitionOwnerAtom::Constructor(record.id()));
                    Ok(CallableContext {
                        parent: LexicalCallableParent::constructor(record.id()),
                        owners,
                        binders,
                    })
                } else if let Some(record) = identity.generated_record() {
                    let parent = LexicalCallableParent::from_generated_key(record.key())
                        .map_err(|_| self.failure(function, Detail::InvalidLexicalRoot))?;
                    Ok(CallableContext {
                        parent,
                        owners: vec![DefinitionOwnerAtom::GeneratedCallable(record.id())],
                        binders,
                    })
                } else {
                    Err(self.failure(function, Detail::InvalidLexicalRoot))
                }
            }
            hir::LexicalDefinitionRoot::StructConstructor(root) => {
                if local_index(root) >= self.lowerer.struct_constructors.len() {
                    return Err(self.failure(function, Detail::InvalidLexicalRoot));
                }
                let record = &self.constructors[root];
                let constructor = &self.lowerer.struct_constructors[root];
                let mut owners = record.key().owners().owners().to_vec();
                owners.push(DefinitionOwnerAtom::Constructor(record.id()));
                Ok(CallableContext {
                    parent: LexicalCallableParent::constructor(record.id()),
                    owners,
                    binders: self.binder_group(
                        function,
                        &self.lowerer.structs[constructor.owner].type_params,
                    )?,
                })
            }
            hir::LexicalDefinitionRoot::VariantConstructor(root) => {
                let enumeration = root.enumeration();
                if local_index(enumeration) >= self.lowerer.enums.len()
                    || root.local_index() as usize >= self.lowerer.enums[enumeration].variants.len()
                {
                    return Err(self.failure(function, Detail::InvalidLexicalRoot));
                }
                let nominal = self.nominals[enumeration]
                    .source()
                    .ok_or_else(|| self.failure(function, Detail::GeneratedDeclarationOwner))?;
                let variant = &self.enum_members[root];
                let mut owners = nominal.declaration().owners().owners().to_vec();
                owners.push(nominal.definition_owner());
                owners.push(DefinitionOwnerAtom::EnumVariant(variant.id()));
                Ok(CallableContext {
                    parent: LexicalCallableParent::variant_constructor(variant.id()),
                    owners,
                    binders: self
                        .binder_group(function, &self.lowerer.enums[enumeration].type_params)?,
                })
            }
        }
    }

    pub(super) fn accessor_context(
        &self,
        function: hir::FunctionId,
        accessor: hir::HirPropertyAccessorFunction,
    ) -> Result<CallableContext, PersistentFunctionIdentityError> {
        let (property, persistent) = match accessor {
            hir::HirPropertyAccessorFunction::Getter(getter) => {
                let identity = self
                    .accessors
                    .get_getter(getter)
                    .ok_or_else(|| self.failure(function, Detail::InvalidLexicalRoot))?;
                (identity.property(), identity.id())
            }
            hir::HirPropertyAccessorFunction::Setter(setter) => {
                let identity = self
                    .accessors
                    .get_setter(setter)
                    .ok_or_else(|| self.failure(function, Detail::InvalidLexicalRoot))?;
                (identity.property(), identity.id())
            }
        };
        let property = &self.properties[property];
        let mut owners = property.declaration().owners().owners().to_vec();
        owners.push(DefinitionOwnerAtom::PropertyAccessor(persistent));
        let signature = self
            .lowerer
            .signatures
            .get(&function)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        Ok(CallableContext {
            parent: LexicalCallableParent::accessor(persistent),
            owners,
            binders: self.binder_group(function, &signature.type_params)?,
        })
    }

    pub(super) fn initialization_context(
        &self,
        function: hir::FunctionId,
        initialization_function: hir::FunctionId,
        unit: hir::InitializationUnitId,
    ) -> Result<CallableContext, PersistentFunctionIdentityError> {
        let declaration = &self.lowerer.initialization_units[unit];
        let mut owners = match declaration.kind {
            hir::InitializationUnitKind::EagerTopLevel { property, .. } => {
                let identity = &self.properties[property];
                let mut owners = identity.declaration().owners().owners().to_vec();
                owners.push(identity.definition_owner());
                owners
            }
            hir::InitializationUnitKind::LazySingleton { value, .. } => {
                let object = self.lowerer.singleton_values[value].declaration;
                let identity = self.nominals[object]
                    .source()
                    .ok_or_else(|| self.failure(function, Detail::GeneratedDeclarationOwner))?;
                let mut owners = identity.declaration().owners().owners().to_vec();
                owners.push(identity.definition_owner());
                owners
            }
        };
        let role = if declaration.initializer == initialization_function {
            InitializationCallableRole::Initializer
        } else if declaration.ensure == initialization_function {
            InitializationCallableRole::Ensure
        } else {
            return Err(self.failure(function, Detail::InvalidLexicalRoot));
        };
        let key = GeneratedCallableKey::Initialization {
            unit: self.initialization_units[unit].id(),
            role,
        };
        let parent = LexicalCallableParent::from_generated_key(&key)
            .map_err(|_| self.failure(function, Detail::InvalidLexicalRoot))?;
        let record = scoop_identity::PersistentGeneratedCallableId::from_key(&key)
            .map_err(|_| self.failure(function, Detail::InvalidLexicalRoot))?;
        owners.push(DefinitionOwnerAtom::GeneratedCallable(record));
        let signature = self
            .lowerer
            .signatures
            .get(&function)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        Ok(CallableContext {
            parent,
            owners,
            binders: self.binder_group(function, &signature.type_params)?,
        })
    }
}
