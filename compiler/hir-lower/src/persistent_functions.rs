use scoop_hir as hir;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    GeneratedCallableKey, InitializationCallableRole, LexicalCallableParent, LexicalCallableRole,
    PackagePath, SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
};

use crate::persistent_types::{SignatureBinder, SignatureTypeMapper};
use crate::{Lowerer, Owner, SourceFunctionDeclaration, namespace::TopLevelLookupLayer};

mod error;
pub(crate) use error::PersistentFunctionIdentityError;
use error::PersistentFunctionIdentityErrorDetail as Detail;
mod context;
mod derived;
mod topology;

#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    nominals: &hir::HirNominalIdentities,
    properties: &hir::HirPropertyIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    initialization_units: &hir::HirInitializationUnitIdentities,
    types: &hir::HirTypeIdentities,
    constructors: &hir::HirConstructorIdentities,
    enum_members: &hir::HirEnumMemberIdentities,
    core_types: hir::HirCoreTypeIdentityAuthority<'_>,
) -> Result<hir::HirFunctionIdentities, PersistentFunctionIdentityError> {
    FunctionIdentityBuilder {
        lowerer,
        nominals,
        properties,
        accessors,
        initialization_units,
        types,
        constructors,
        enum_members,
        type_mapper: SignatureTypeMapper::new(lowerer, nominals, core_types),
        identities: vec![None; lowerer.functions.len()],
        visiting: vec![false; lowerer.functions.len()],
    }
    .build()
}

#[derive(Clone)]
struct CallableContext {
    parent: LexicalCallableParent,
    owners: Vec<DefinitionOwnerAtom>,
    binders: Vec<SignatureBinder>,
}

#[derive(Clone)]
struct LexicalSite {
    root: hir::LexicalDefinitionRoot,
    path: StructuralDefinitionPath,
    role: LexicalCallableRole,
}

struct FunctionIdentityBuilder<'a> {
    lowerer: &'a Lowerer,
    nominals: &'a hir::HirNominalIdentities,
    properties: &'a hir::HirPropertyIdentities,
    accessors: &'a hir::HirPropertyAccessorIdentities,
    initialization_units: &'a hir::HirInitializationUnitIdentities,
    types: &'a hir::HirTypeIdentities,
    constructors: &'a hir::HirConstructorIdentities,
    enum_members: &'a hir::HirEnumMemberIdentities,
    type_mapper: SignatureTypeMapper<'a>,
    identities: Vec<Option<hir::HirFunctionIdentity>>,
    visiting: Vec<bool>,
}

impl FunctionIdentityBuilder<'_> {
    fn build(mut self) -> Result<hir::HirFunctionIdentities, PersistentFunctionIdentityError> {
        let functions = self
            .lowerer
            .functions
            .iter()
            .map(|(function, _)| function)
            .collect::<Vec<_>>();
        for function in functions {
            self.resolve(function)?;
        }
        let pending_identities = std::mem::take(&mut self.identities);
        let identities = pending_identities
            .into_iter()
            .enumerate()
            .map(|(index, identity)| {
                identity.ok_or_else(|| {
                    self.failure(
                        hir::FunctionId::from_raw((index as u32).into()),
                        Detail::InvalidFunctionOrigin,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        hir::HirFunctionIdentities::checked(
            hir::HirFunctionIdentityInputs {
                functions: self.lowerer.functions.as_arena(),
                lambdas: &self.lowerer.lambdas,
                anonymous_functions: &self.lowerer.anonymous_functions,
                local_functions: &self.lowerer.local_functions,
                property_getters: &self.lowerer.property_getters,
                property_setters: &self.lowerer.property_setters,
                property_accessor_identities: self.accessors,
                initialization_units: &self.lowerer.initialization_units,
                initialization_unit_identities: self.initialization_units,
                derived_equality_applications: &self.lowerer.derived_equality_applications,
                structs: &self.lowerer.structs,
                enums: &self.lowerer.enums,
                type_identities: self.types,
                struct_constructors: &self.lowerer.struct_constructors,
                class_constructors: &self.lowerer.class_constructors,
                constructor_identities: self.constructors,
                enum_member_identities: self.enum_members,
            },
            identities,
        )
        .map_err(|error| self.failure_for_relation(error))
    }

    fn resolve(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<hir::HirFunctionIdentity, PersistentFunctionIdentityError> {
        let index = local_index(function);
        if let Some(identity) = &self.identities[index] {
            return Ok(identity.clone());
        }
        if std::mem::replace(&mut self.visiting[index], true) {
            return Err(self.failure(function, Detail::CyclicLexicalParent));
        }

        let source = self.lowerer.source_function_declarations.get(&function);
        let accessor = self.accessor(function)?;
        let lexical = self.lexical_site(function)?;
        let initialization = self.initialization(function)?;
        let derived = matches!(
            self.lowerer.functions[function].kind,
            hir::FunctionKind::DerivedEquality
        );
        let kinds = usize::from(source.is_some())
            + usize::from(accessor.is_some())
            + usize::from(lexical.is_some())
            + usize::from(initialization.is_some())
            + usize::from(derived);
        if kinds != 1 {
            return Err(self.failure(function, Detail::InvalidFunctionOrigin));
        }

        let identity = if let Some(source) = source.cloned() {
            hir::HirFunctionIdentity::source(self.source_identity(function, &source)?)
        } else if let Some(accessor) = accessor {
            hir::HirFunctionIdentity::property_accessor(accessor)
        } else if let Some(site) = lexical {
            let parent = self
                .context_for_site(function, site.root, &site.path)?
                .parent;
            hir::HirFunctionIdentity::lexical_generated(parent, site.role, site.path)
                .map_err(|error| self.failure(function, Detail::InvalidIdentity(error)))?
        } else if let Some((unit, role)) = initialization {
            hir::HirFunctionIdentity::initialization(
                unit,
                self.initialization_units[unit].id(),
                role,
            )
            .map_err(|error| self.failure(function, Detail::InvalidIdentity(error)))?
        } else {
            self.derived_identity(function)?
        };
        self.visiting[index] = false;
        self.identities[index] = Some(identity.clone());
        Ok(identity)
    }

    fn source_identity(
        &mut self,
        function: hir::FunctionId,
        source_declaration: &SourceFunctionDeclaration,
    ) -> Result<hir::HirSourceFunctionIdentity, PersistentFunctionIdentityError> {
        let signature = self
            .lowerer
            .signatures
            .get(&function)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        let own_count = signature
            .type_params
            .len()
            .checked_sub(signature.owner_type_param_count)
            .ok_or_else(|| self.failure(function, Detail::InvalidBinderInheritance))?;
        let own_count_u32 = u32::try_from(own_count)
            .map_err(|_| self.failure(function, Detail::TooManyTypeParameters))?;
        let file = self.lowerer.function_files[&function];
        let source = self.source(function, file)?;
        let package = self.package(function, file)?;

        let (owners, scope, binders) = if let Some(local) = self
            .lowerer
            .local_function_by_function
            .get(&function)
            .copied()
        {
            let local = &self.lowerer.local_functions[local];
            let context = self.context_for_site(
                function,
                local
                    .source()
                    .expect("source identity has a current declaration")
                    .1,
                &local.definition_path,
            )?;
            let inherited = &signature.type_params[..signature.owner_type_param_count];
            if inherited.len() != context.binders.len()
                || inherited
                    .iter()
                    .zip(&context.binders)
                    .any(|(parameter, binder)| parameter.id != binder.parameter)
            {
                return Err(self.failure(function, Detail::InvalidBinderInheritance));
            }
            let binders = self.append_binder_group(
                function,
                context.binders,
                &signature.type_params[signature.owner_type_param_count..],
            )?;
            (
                context.owners,
                DeclarationScope::LexicalScoped {
                    source: source.clone(),
                    path: local.definition_path.clone(),
                },
                binders,
            )
        } else {
            let owners = self.direct_source_owners(function, &source, &package)?;
            let scope = if !self.lowerer.function_owner.contains_key(&function)
                && self.lowerer.functions[function].access.declared
                    == hir::DeclaredVisibility::Private
            {
                DeclarationScope::SourceScoped(source.clone())
            } else {
                DeclarationScope::ConeWide
            };
            let owner_group = self.binder_group(
                function,
                &signature.type_params[..signature.owner_type_param_count],
            )?;
            let binders = self.append_binder_group(
                function,
                owner_group,
                &signature.type_params[signature.owner_type_param_count..],
            )?;
            (owners, scope, binders)
        };

        let site = SourceDeclarationSite::new(
            source.cone(),
            package,
            DefinitionOwnerChain::from_outer_to_inner(owners),
            scope,
        )
        .map_err(|error| self.failure(function, Detail::InvalidSite(error)))?;
        let name = CanonicalIdentifier::new(&source_declaration.name)
            .map_err(|error| self.failure(function, Detail::InvalidName(error)))?;
        let receiver = self
            .lowerer
            .extension_receivers
            .get(&function)
            .copied()
            .map(|ty| self.type_mapper.map(ty, &binders))
            .transpose()
            .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))?;
        let parameters = signature
            .params
            .iter()
            .map(|parameter| self.type_mapper.map(parameter.ty, &binders))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))?;
        hir::HirSourceFunctionIdentity::from_declaration(SourceDeclarationKey::function(
            site,
            name,
            own_count_u32,
            receiver,
            parameters,
        ))
        .map_err(|error| self.failure(function, Detail::InvalidIdentity(error)))
    }

    fn source_function_binders(
        &mut self,
        function: hir::FunctionId,
        source: hir::FunctionId,
    ) -> Result<Vec<SignatureBinder>, PersistentFunctionIdentityError> {
        let signature = self
            .lowerer
            .signatures
            .get(&source)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        if let Some(local) = self
            .lowerer
            .local_function_by_function
            .get(&source)
            .copied()
        {
            let local = &self.lowerer.local_functions[local];
            let context = self.context_for_site(
                source,
                local
                    .source()
                    .expect("source identity has a current declaration")
                    .1,
                &local.definition_path,
            )?;
            let inherited = &signature.type_params[..signature.owner_type_param_count];
            if inherited.len() != context.binders.len()
                || inherited
                    .iter()
                    .zip(&context.binders)
                    .any(|(parameter, binder)| parameter.id != binder.parameter)
            {
                return Err(self.failure(function, Detail::InvalidBinderInheritance));
            }
            self.append_binder_group(
                function,
                context.binders,
                &signature.type_params[signature.owner_type_param_count..],
            )
        } else {
            let owner = self.binder_group(
                function,
                &signature.type_params[..signature.owner_type_param_count],
            )?;
            self.append_binder_group(
                function,
                owner,
                &signature.type_params[signature.owner_type_param_count..],
            )
        }
    }

    fn binder_group(
        &self,
        function: hir::FunctionId,
        parameters: &[hir::TypeParamDecl],
    ) -> Result<Vec<SignatureBinder>, PersistentFunctionIdentityError> {
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                Ok(SignatureBinder {
                    parameter: parameter.id,
                    depth: 0,
                    index: u32::try_from(index)
                        .map_err(|_| self.failure(function, Detail::TooManyTypeParameters))?,
                })
            })
            .collect()
    }

    fn append_binder_group(
        &self,
        function: hir::FunctionId,
        mut outer: Vec<SignatureBinder>,
        inner: &[hir::TypeParamDecl],
    ) -> Result<Vec<SignatureBinder>, PersistentFunctionIdentityError> {
        if inner.is_empty() {
            return Ok(outer);
        }
        for binder in &mut outer {
            binder.depth = binder
                .depth
                .checked_add(1)
                .ok_or_else(|| self.failure(function, Detail::BinderDepthOverflow))?;
        }
        outer.extend(self.binder_group(function, inner)?);
        Ok(outer)
    }

    fn direct_source_owners(
        &self,
        function: hir::FunctionId,
        source: &scoop_identity::SourceIdentity,
        package: &PackagePath,
    ) -> Result<Vec<DefinitionOwnerAtom>, PersistentFunctionIdentityError> {
        let Some(owner) = self.lowerer.function_owner.get(&function).copied() else {
            return Ok(Vec::new());
        };
        let identity = match owner {
            Owner::Class(owner) => &self.nominals[owner],
            Owner::Struct(owner) => &self.nominals[owner],
            Owner::Enum(owner) => &self.nominals[owner],
            Owner::Interface(owner) => &self.nominals[owner],
            Owner::Object(owner) => &self.nominals[owner],
        };
        let identity = identity
            .source()
            .ok_or_else(|| self.failure(function, Detail::GeneratedDeclarationOwner))?;
        if identity.declaration().origin() != source.cone()
            || identity.declaration().package() != package
        {
            return Err(self.failure(function, Detail::OwnerSiteMismatch));
        }
        let mut owners = identity.declaration().owners().owners().to_vec();
        owners.push(identity.definition_owner());
        Ok(owners)
    }

    fn source(
        &self,
        function: hir::FunctionId,
        file: usize,
    ) -> Result<scoop_identity::SourceIdentity, PersistentFunctionIdentityError> {
        self.lowerer
            .intrinsic_sources
            .get(file)
            .map(|source| source.identity.clone())
            .ok_or_else(|| self.failure(function, Detail::MissingSourceFile))
    }

    fn package(
        &self,
        function: hir::FunctionId,
        file: usize,
    ) -> Result<PackagePath, PersistentFunctionIdentityError> {
        match self.lowerer.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => Ok(PackagePath::root()),
            TopLevelLookupLayer::CurrentPackage(package) => self
                .lowerer
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(CanonicalIdentifier::new)
                .collect::<Result<Vec<_>, _>>()
                .map(PackagePath::from_segments)
                .map_err(|error| self.failure(function, Detail::InvalidName(error))),
        }
    }

    fn failure(
        &self,
        function: hir::FunctionId,
        detail: Detail,
    ) -> PersistentFunctionIdentityError {
        let declaration = &self.lowerer.functions[function];
        PersistentFunctionIdentityError {
            file: self.lowerer.function_files[&function],
            span: declaration.span,
            detail,
        }
    }

    fn failure_for_relation(
        &self,
        error: hir::HirFunctionIdentityError,
    ) -> PersistentFunctionIdentityError {
        PersistentFunctionIdentityError {
            file: 0,
            span: scoop_ast::Span { start: 0, end: 0 },
            detail: Detail::InvalidIdentity(error),
        }
    }
}

fn local_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
