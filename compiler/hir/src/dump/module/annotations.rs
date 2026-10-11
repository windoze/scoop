use super::*;
use std::fmt::Write;

pub(super) fn dump_annotations(module: &Module, out: &mut String) {
    for declaration in &module.annotations.declarations {
        let scoop_identity::DeclarationName::Named(name) = declaration.identity.key().name() else {
            unreachable!("annotation declarations have names");
        };
        write!(out, "  annotation {name} {}(", declaration.identity.id())
            .expect("writing to String");
        for (index, parameter) in declaration.parameters.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            write!(
                out,
                "{}: {}",
                parameter.name,
                match parameter.value_type {
                    crate::AnnotationParameterType::Value(ty) => type_name(module, ty),
                    crate::AnnotationParameterType::Annotation(id) => format!("annotation<{id}>"),
                    crate::AnnotationParameterType::AnnotationArray { element, .. } =>
                        format!("Array<annotation<{element}>>"),
                }
            )
            .expect("writing to String");
            if let Some(default) = &parameter.default {
                write!(out, " = {default:?}").expect("writing to String");
            }
        }
        out.push_str(")\n");
    }
    for target in &module.annotations.targets {
        writeln!(out, "  annotations {}", target_name(module, target.target))
            .expect("writing to String");
        for application in &target.annotations {
            writeln!(
                out,
                "    @{} {:?}",
                application.annotation, application.arguments
            )
            .expect("writing to String");
        }
    }
}

fn target_name(module: &Module, target: SourceAnnotationTarget) -> String {
    match target {
        SourceAnnotationTarget::Nominal(owner) => nominal_name(module, owner),
        SourceAnnotationTarget::Field(field) => {
            let owner = &module.structs[field.structure()];
            format!(
                "{}.{}",
                nominal_name(module, NominalOwner::Struct(field.structure())),
                owner.semantic_fields()[field.local_index() as usize].name
            )
        }
        SourceAnnotationTarget::Variant(variant) => variant_name(module, variant),
        SourceAnnotationTarget::VariantField(field) => {
            let variant = field.variant();
            let declaration =
                &module.enums[variant.enumeration()].variants[variant.local_index() as usize];
            format!(
                "{}.{}",
                variant_name(module, variant),
                declaration.fields[field.local_index() as usize].name
            )
        }
        SourceAnnotationTarget::Property(id) => {
            let property = &module.properties[id];
            let owner = match property.owner {
                PropertyOwner::Class(id) => NominalOwner::Class(id),
                PropertyOwner::Interface(id) => NominalOwner::Interface(id),
                PropertyOwner::Object(id) => NominalOwner::Object(id),
                PropertyOwner::Struct(id) => NominalOwner::Struct(id),
                PropertyOwner::Enum(id) => NominalOwner::Enum(id),
                PropertyOwner::TopLevel | PropertyOwner::Extension(_) => {
                    return property.name.clone();
                }
            };
            format!("{}.{}", nominal_name(module, owner), property.name)
        }
        SourceAnnotationTarget::Annotation(id) => format!("annotation {id}"),
        SourceAnnotationTarget::Callable(id) => format!("callable {id:?}"),
        SourceAnnotationTarget::Parameter { callable, index } => {
            format!("parameter {callable:?}[{index}]")
        }
        SourceAnnotationTarget::Receiver(receiver) => format!("receiver {receiver:?}"),
        SourceAnnotationTarget::TypeAlias(id) => {
            format!("typealias {}", module.type_aliases[id].name)
        }
        SourceAnnotationTarget::ClassField(id) => format!("field {id:?}"),
        SourceAnnotationTarget::PropertyStorage { property, role } => {
            format!("storage {} {role:?}", module.properties[property].name)
        }
    }
}

fn variant_name(module: &Module, variant: EnumVariantRef) -> String {
    format!(
        "{}.{}",
        nominal_name(module, NominalOwner::Enum(variant.enumeration())),
        module.enums[variant.enumeration()].variants[variant.local_index() as usize].name
    )
}

fn nominal_name(module: &Module, owner: NominalOwner) -> String {
    let (name, parent) = match owner {
        NominalOwner::Struct(id) => (&module.structs[id].name, module.structs[id].owner),
        NominalOwner::Enum(id) => (&module.enums[id].name, module.enums[id].owner),
        NominalOwner::Class(id) => (&module.classes[id].name, module.classes[id].owner),
        NominalOwner::Interface(id) => (&module.interfaces[id].name, module.interfaces[id].owner),
        NominalOwner::Object(id) => (&module.objects[id].name, module.objects[id].owner),
    };
    nominal_declaration_name(module, name, parent)
}
