//! Complete declaration metadata for directly assembled MIR test inputs.

mod finish;

use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SourceDeclarationKey, SourceDeclarationSite,
};

impl Harness {
    pub(super) fn use_identity_instances(
        &mut self,
        function: hir::FunctionId,
        types: &[hir::TypeId],
    ) {
        let mut locals = Arena::new();
        let mut parameters = Vec::new();
        let mut statements = Vec::new();
        for (index, ty) in types.iter().copied().enumerate() {
            let name = format!("value{index}");
            let value = locals.alloc(local(&name, ty));
            parameters.push(param(&name, ty, value));
            let application = self.instantiate(function, vec![ty]);
            statements.push(expr_stmt(generic_call(
                application,
                vec![local_ref(value, ty)],
                ty,
            )));
        }
        let caller = self.user_fn_full(
            "useIdentityInstances",
            Vec::new(),
            parameters,
            self.unit,
            hir::Body { locals, statements },
        );
        self.functions[caller].is_suspend = self.functions[function].is_suspend;
    }

    pub(super) fn test_function_identity(
        &self,
        inputs: hir::HirTypeIdentityInputs<'_>,
        function: hir::FunctionId,
        entry: hir::FunctionId,
    ) -> hir::HirFunctionIdentity {
        let declaration = &self.functions[function];
        let owner = declaration
            .method
            .map(|method| self.test_method_owner(inputs, method.owner));
        let owners = owner
            .map(|(identity, _)| identity.source().unwrap().definition_owner())
            .into_iter()
            .collect();
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(owners),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let name = if function == entry {
            "main".to_string()
        } else {
            format!("fixture_function_{}", function.into_raw().into_u32())
        };
        let (own, outer): (Vec<hir::TypeParamDecl>, Vec<hir::TypeParamDecl>) = match &declaration
            .genericity
        {
            hir::FunctionGenericity::Plain => (
                Vec::new(),
                owner.map_or_else(Vec::new, |(_, parameters)| parameters.to_vec()),
            ),
            hir::FunctionGenericity::Generic { parameters, .. } => (parameters.clone(), Vec::new()),
            hir::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => (Vec::new(), owner_parameters.clone()),
            hir::FunctionGenericity::GenericMethod {
                method_parameters,
                owner_parameters,
                ..
            } => (
                method_parameters.iter().cloned().collect(),
                owner_parameters.clone(),
            ),
        };
        let binders = own
            .iter()
            .enumerate()
            .map(|(index, parameter)| (index, parameter, 0))
            .chain(
                outer
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| (index, parameter, u32::from(!own.is_empty()))),
            )
            .map(|(index, parameter, depth)| hir::HirSignatureBinder {
                parameter: parameter.id,
                depth,
                index: u32::try_from(index).unwrap(),
            })
            .collect::<Vec<_>>();
        let mapper = hir::HirSignatureTypeMapper::new(inputs);
        let parameters = declaration
            .params
            .iter()
            .skip(usize::from(declaration.method.is_some()))
            .map(|parameter| mapper.map(parameter.ty, &binders).unwrap())
            .collect();
        hir::HirFunctionIdentity::source(
            hir::HirSourceFunctionIdentity::from_declaration(SourceDeclarationKey::function(
                site,
                CanonicalIdentifier::new(&name).unwrap(),
                u32::try_from(own.len()).unwrap(),
                None,
                parameters,
            ))
            .unwrap(),
        )
    }

    fn test_method_owner<'a>(
        &'a self,
        inputs: hir::HirTypeIdentityInputs<'a>,
        ty: hir::TypeId,
    ) -> (&'a hir::HirNominalIdentity, &'a [hir::TypeParamDecl]) {
        match self.types[ty] {
            hir::Type::Struct(application) => {
                let owner = self.struct_applications[application].template;
                (
                    &inputs.nominal_identities[owner],
                    &self.structs[owner].type_params,
                )
            }
            hir::Type::Class(application) => {
                let owner = self.class_applications[application].template;
                (
                    &inputs.nominal_identities[owner],
                    &self.classes[owner].type_params,
                )
            }
            hir::Type::Enum(application) => {
                let owner = self.enum_applications[application].template;
                (
                    &inputs.nominal_identities[owner],
                    &self.enums[owner].type_params,
                )
            }
            hir::Type::Interface(application) => {
                let owner = self.interface_applications[application].template;
                (
                    &inputs.nominal_identities[owner],
                    &self.interfaces[owner].type_params,
                )
            }
            hir::Type::Integer(kind) => {
                let hir::HirCoreTypeIdentityAuthority::Defined(core) = inputs.core_types else {
                    panic!("test protocols are local")
                };
                (&inputs.nominal_identities[core.integers.owner(kind)], &[])
            }
            hir::Type::Boolean => {
                let hir::HirCoreTypeIdentityAuthority::Defined(core) = inputs.core_types else {
                    panic!("test protocols are local")
                };
                (&inputs.nominal_identities[core.boolean], &[])
            }
            hir::Type::String => {
                let hir::HirCoreTypeIdentityAuthority::Defined(core) = inputs.core_types else {
                    panic!("test protocols are local")
                };
                (&inputs.nominal_identities[core.string], &[])
            }
            _ => panic!("source fixture methods have nominal owners"),
        }
    }

    pub(super) fn test_parameter_interfaces(&self) -> Vec<hir::ExportParameterInterface> {
        let parameter = |name: &str, ty| hir::ExportValueParameter {
            name: name.to_owned(),
            calling: hir::ExportParameterCalling::Required { value_type: ty },
            origin: definition_origin(),
        };
        let mut interfaces = Vec::new();
        for (id, function) in self.functions.iter() {
            let parameters = function
                .params
                .iter()
                .skip(usize::from(function.method.is_some()))
                .map(|value| parameter(&value.name, value.ty))
                .collect();
            interfaces.push(hir::ExportParameterInterface {
                owner: hir::ExportParameterOwner::Function(id),
                parameters,
            });
        }
        for (id, constructor) in self.struct_constructors.iter() {
            interfaces.push(hir::ExportParameterInterface {
                owner: hir::ExportParameterOwner::StructConstructor(id),
                parameters: constructor
                    .parameters
                    .iter()
                    .map(|value| parameter(&value.name, value.ty))
                    .collect(),
            });
        }
        for (id, constructor) in self.class_constructors.iter() {
            interfaces.push(hir::ExportParameterInterface {
                owner: hir::ExportParameterOwner::ClassConstructor(id),
                parameters: constructor
                    .parameters
                    .iter()
                    .map(|value| parameter(&value.name, value.ty))
                    .collect(),
            });
        }
        for (id, enumeration) in self.enums.iter() {
            for (index, variant) in enumeration.variants.iter().enumerate() {
                let variant_id =
                    hir::EnumVariantRef::checked(&self.enums, id, u32::try_from(index).unwrap())
                        .unwrap();
                interfaces.push(hir::ExportParameterInterface {
                    owner: hir::ExportParameterOwner::VariantConstructor(variant_id),
                    parameters: variant
                        .fields
                        .iter()
                        .map(|value| parameter(&value.name, value.ty))
                        .collect(),
                });
            }
        }
        interfaces
    }
}
