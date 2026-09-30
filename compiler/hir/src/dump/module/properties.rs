use super::*;

pub(super) fn dump_property(module: &Module, id: PropertyId, indent: usize, out: &mut String) {
    let property = &module.properties[id];
    let modifier = match property.modifier {
        MethodModifier::Final => "",
        MethodModifier::Open => "open ",
        MethodModifier::Abstract => "abstract ",
    };
    let override_ = if property.is_override {
        "override "
    } else {
        ""
    };
    let mutability = if property.capability.setter().is_some() {
        "var"
    } else {
        "val"
    };
    let getter = property.capability.getter();
    let getter = format!(
        "getter{}={}",
        getter.into_raw(),
        dump_accessor_implementation(module, module.property_getters[getter].implementation)
    );
    let setter = property
        .capability
        .setter()
        .map_or_else(String::new, |setter| {
            format!(
                " setter{}={}",
                setter.into_raw(),
                dump_accessor_implementation(
                    module,
                    module.property_setters[setter].implementation
                )
            )
        });
    let representation = match &property.representation {
        PropertyRepresentation::Stored(stored) => match stored.backing {
            PropertyBacking::TopLevelGlobal {
                storage,
                initialization,
            } => {
                let initialization = match initialization {
                    TopLevelInitialization::Image => "image".to_string(),
                    TopLevelInitialization::Runtime(unit) => {
                        format!("init{}", unit.into_raw())
                    }
                };
                format!("stored global{} {initialization}", storage.into_raw())
            }
            PropertyBacking::ClassField { field, initializer } => format!(
                "stored field{} init={}",
                field.into_raw(),
                match initializer {
                    ClassPropertyInitializer::PrimaryParameter(parameter) => {
                        format!("parameter{}", parameter.into_raw())
                    }
                    ClassPropertyInitializer::Expression => "expression".to_string(),
                    ClassPropertyInitializer::SyntheticNone => "synthetic-none".to_string(),
                }
            ),
            PropertyBacking::StructField { owner, index } => {
                format!("stored struct{}-field{index}", owner.into_raw())
            }
        },
        PropertyRepresentation::AccessorOnly => "accessor-only".to_string(),
        PropertyRepresentation::GenericDelegated { template } => {
            let delegate = &module.generic_delegate_templates[*template];
            format!(
                "generic-delegated template{} type={} init{}",
                template.into_raw(),
                type_name(module, delegate.ty),
                delegate.initialization.into_raw()
            )
        }
        PropertyRepresentation::Delegated { storage } => {
            let delegate = &module.delegate_storages[*storage];
            let location = match delegate.location {
                DelegateStorageLocation::ClassField(field) => {
                    format!("class-field{}", field.into_raw())
                }
                DelegateStorageLocation::ManagedGlobal(global) => {
                    format!("managed-global{}", global.into_raw())
                }
            };
            format!(
                "delegated storage{} type={} location={location}",
                storage.into_raw(),
                type_name(module, delegate.ty)
            )
        }
        PropertyRepresentation::Const { value } => format!("const {value:?}"),
        PropertyRepresentation::NativeStorage { storage } => {
            format!("native-storage global{}", storage.into_raw())
        }
    };
    let overrides = if property.overrides.is_empty() {
        String::new()
    } else {
        format!(
            " overrides=[{}]",
            property
                .overrides
                .iter()
                .map(|property| match property {
                    PropertyReference::Local(property) => property.into_raw().to_string(),
                    PropertyReference::Imported { owner, declaration } => {
                        format!("imported({},{declaration})", type_name(module, *owner))
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let (type_params, receiver, property_ty) = match property.owner {
        PropertyOwner::Extension(extension) => {
            let extension = &module.extension_properties[extension];
            let params = if extension.type_params.is_empty() {
                String::new()
            } else {
                format!("{} ", dump_type_params(module, &extension.type_params))
            };
            (
                params,
                format!(
                    "{}.",
                    type_name_with_params(module, extension.receiver_ty, &extension.type_params)
                ),
                type_name_with_params(module, property.ty, &extension.type_params),
            )
        }
        _ => (String::new(), String::new(), type_name(module, property.ty)),
    };
    out.push_str(&format!(
        "{}property{} {modifier}{override_}{mutability} {type_params}{receiver}{}: {property_ty} {getter}{setter} <{representation}>{overrides}\n",
        "  ".repeat(indent),
        id.into_raw(),
        property.name,
    ));
}

fn dump_accessor_implementation(
    module: &Module,
    implementation: PropertyAccessorImplementation,
) -> String {
    match implementation {
        PropertyAccessorImplementation::Storage => "storage".to_string(),
        PropertyAccessorImplementation::Constant => "constant".to_string(),
        PropertyAccessorImplementation::Body(function) => {
            format!("body({})", module.functions[function].name)
        }
        PropertyAccessorImplementation::AbstractSlot(function) => {
            format!("abstract({})", module.functions[function].name)
        }
    }
}
