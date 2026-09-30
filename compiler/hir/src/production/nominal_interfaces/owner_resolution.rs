use scoop_identity::NominalDeclarationOwner;

pub(in crate::production) fn from_type(
    export: &crate::ExportHir,
    ty: crate::TypeId,
) -> Option<NominalDeclarationOwner> {
    let ty = super::arena_get(&export.types, ty)?;
    let identity = match ty {
        crate::Type::ImportedClass(structure) => {
            return Some(structure.declaration.owner());
        }
        crate::Type::ImportedInterface(structure) => {
            return Some(structure.declaration.owner());
        }
        crate::Type::Unit => {
            return Some(core_builtin_owner(
                export,
                scoop_identity::CoreBuiltinNominal::Unit,
            ));
        }
        crate::Type::Integer(kind) => {
            return intrinsic_concrete_owner(export, IntrinsicConcreteType::Integer(*kind));
        }
        crate::Type::Boolean => {
            return intrinsic_concrete_owner(export, IntrinsicConcreteType::Boolean);
        }
        crate::Type::String => {
            return intrinsic_concrete_owner(export, IntrinsicConcreteType::String);
        }
        crate::Type::Any => {
            return Some(core_builtin_owner(
                export,
                scoop_identity::CoreBuiltinNominal::Any,
            ));
        }
        crate::Type::Struct(application) => {
            return Some(super::arena_get(&export.struct_applications, *application)?.template);
        }
        crate::Type::Enum(application) => {
            return Some(super::arena_get(&export.enum_applications, *application)?.template);
        }
        crate::Type::Interface(application) => {
            return Some(super::arena_get(&export.interface_applications, *application)?.template);
        }
        crate::Type::Class(application) => {
            let application = super::arena_get(&export.class_applications, *application)?;
            class_or_object_identity(
                export,
                export.nominal_identities.class_id(application.template)?,
            )?
        }
        crate::Type::Ptr(_) => {
            return intrinsic_generic_owner(export, IntrinsicGenericType::Pointer);
        }
        crate::Type::FunPtr(_) => {
            return intrinsic_generic_owner(export, IntrinsicGenericType::FunctionPointer);
        }
        crate::Type::Tuple(_) | crate::Type::Function(_) | crate::Type::Param(_) => return None,
    };
    identity.source().map(super::source_nominal_id)
}

pub(in crate::production) fn from_property(
    export: &crate::ExportHir,
    owner: crate::PropertyOwner,
) -> Option<NominalDeclarationOwner> {
    let identity = match owner {
        crate::PropertyOwner::Class(id) => export.nominal_identities.get_class(id)?,
        crate::PropertyOwner::Struct(id) => export.nominal_identities.get_struct(id)?,
        crate::PropertyOwner::Enum(id) => export.nominal_identities.get_enum(id)?,
        crate::PropertyOwner::Interface(id) => export.nominal_identities.get_interface(id)?,
        crate::PropertyOwner::Object(id) => export.nominal_identities.get_object(id)?,
        crate::PropertyOwner::TopLevel | crate::PropertyOwner::Extension(_) => return None,
    };
    identity.source().map(super::source_nominal_id)
}

fn class_or_object_identity(
    export: &crate::ExportHir,
    class: crate::ClassId,
) -> Option<&crate::HirNominalIdentity> {
    let mut objects = export
        .objects
        .iter()
        .filter_map(|(id, declaration)| (declaration.backing_class == class).then_some(id));
    match (objects.next(), objects.next()) {
        (Some(object), None) => export.nominal_identities.get_object(object),
        (None, None) => export.nominal_identities.get_class(class),
        _ => None,
    }
}

fn core_builtin_owner(
    export: &crate::ExportHir,
    builtin: scoop_identity::CoreBuiltinNominal,
) -> NominalDeclarationOwner {
    NominalDeclarationOwner::Concrete(export.nominal_identities.core_builtin(builtin).id())
}

#[derive(Clone, Copy)]
enum IntrinsicGenericType {
    Pointer,
    FunctionPointer,
}

fn intrinsic_generic_owner(
    export: &crate::ExportHir,
    intrinsic: IntrinsicGenericType,
) -> Option<NominalDeclarationOwner> {
    let persistent = match &export.core_protocols {
        crate::CoreProtocols::Defined(protocols) => {
            let owner = match intrinsic {
                IntrinsicGenericType::Pointer => protocols.fundamental_types.ptr,
                IntrinsicGenericType::FunctionPointer => protocols.fundamental_types.fun_ptr,
            };
            export
                .nominal_identities
                .get_struct(owner)?
                .source()?
                .generic_id()?
        }
        crate::CoreProtocols::Imported(protocols) => match intrinsic {
            IntrinsicGenericType::Pointer => protocols.fundamental_types().ptr().persistent(),
            IntrinsicGenericType::FunctionPointer => {
                protocols.fundamental_types().fun_ptr().persistent()
            }
        },
    };
    Some(NominalDeclarationOwner::GenericTemplate(persistent))
}

#[derive(Clone, Copy)]
enum IntrinsicConcreteType {
    Integer(crate::IntegerKind),
    Boolean,
    String,
}

fn intrinsic_concrete_owner(
    export: &crate::ExportHir,
    intrinsic: IntrinsicConcreteType,
) -> Option<NominalDeclarationOwner> {
    let persistent = match &export.core_protocols {
        crate::CoreProtocols::Defined(protocols) => {
            let identity = match intrinsic {
                IntrinsicConcreteType::Integer(kind) => export
                    .nominal_identities
                    .get_struct(protocols.fundamental_types.integers.owner(kind)),
                IntrinsicConcreteType::Boolean => export
                    .nominal_identities
                    .get_struct(protocols.fundamental_types.boolean),
                IntrinsicConcreteType::String => export
                    .nominal_identities
                    .get_class(protocols.fundamental_types.string),
            }?;
            identity.source()?.concrete_id()?
        }
        crate::CoreProtocols::Imported(protocols) => match intrinsic {
            IntrinsicConcreteType::Integer(kind) => {
                protocols.fundamental_types().integer(kind).persistent()
            }
            IntrinsicConcreteType::Boolean => protocols.fundamental_types().boolean().persistent(),
            IntrinsicConcreteType::String => protocols.fundamental_types().string().persistent(),
        },
    };
    Some(NominalDeclarationOwner::Concrete(persistent))
}
