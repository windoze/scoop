use super::AnnotationProductionError;
use crate::{
    AnnotationCallableV1 as Callable, AnnotationTargetV1 as Target, ExportHir, HirFunctionIdentity,
    HirSourceFunctionIdentity, SourceAnnotationCallable as SourceCallable,
    SourceAnnotationReceiver, SourceAnnotationTarget as Source,
};
use scoop_identity::{CallableTemplateOrigin as Declaration, PropertyOwner};

pub(super) fn project(
    export: &ExportHir,
    target: Source,
) -> Result<Target, AnnotationProductionError> {
    Ok(match target {
        Source::Nominal(owner) => Target::Nominal(match owner {
            crate::NominalOwner::Struct(id) => export.nominal_identities[id].declaration_id(),
            crate::NominalOwner::Enum(id) => export.nominal_identities[id].declaration_id(),
            crate::NominalOwner::Class(id) => export.nominal_identities[id].declaration_id(),
            crate::NominalOwner::Interface(id) => export.nominal_identities[id].declaration_id(),
            crate::NominalOwner::Object(id) => export.nominal_identities[id].declaration_id(),
        }),
        Source::Field(field) => Target::Field(export.field_identities[field].id()),
        Source::ClassField(field) => Target::Field(export.field_identities[field].id()),
        Source::Variant(variant) => Target::Variant(export.enum_member_identities[variant].id()),
        Source::VariantField(field) => {
            Target::VariantField(export.enum_member_identities[field].id())
        }
        Source::Property(property) => match export.property_identities[property].property_owner() {
            PropertyOwner::Property(id) => Target::Property(id),
            PropertyOwner::ExtensionProperty(id) => Target::ExtensionProperty(id),
        },
        Source::Annotation(id) => Target::Annotation(id),
        Source::Callable(id) => Target::Callable(callable(export, id)?),
        Source::Parameter {
            callable: id,
            index,
        } => Target::Parameter {
            callable: callable(export, id)?,
            index,
        },
        Source::Receiver(SourceAnnotationReceiver::Callable(id)) => {
            Target::CallableReceiver(callable(export, id)?)
        }
        Source::Receiver(SourceAnnotationReceiver::Property(id)) => {
            Target::PropertyReceiver(export.property_identities[id].property_owner())
        }
        Source::TypeAlias(id) => Target::TypeAlias(export.type_alias_identities[id].id()),
        Source::PropertyStorage { property, role } => Target::PropertyStorage {
            property: export.property_identities[property].property_owner(),
            role,
        },
    })
}

fn callable(export: &ExportHir, id: SourceCallable) -> Result<Callable, AnnotationProductionError> {
    let source = match id {
        SourceCallable::Function(id) => match &export.function_identities[id] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                Declaration::Function(record.id())
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => {
                Declaration::GenericFunction(record.id())
            }
            HirFunctionIdentity::LexicalGenerated(record) => {
                return Ok(Callable::Local(record.id()));
            }
            HirFunctionIdentity::PropertyAccessor(crate::HirPropertyAccessorFunction::Getter(
                id,
            )) => Declaration::Accessor(export.property_accessor_identities[*id].id()),
            HirFunctionIdentity::PropertyAccessor(crate::HirPropertyAccessorFunction::Setter(
                id,
            )) => Declaration::Accessor(export.property_accessor_identities[*id].id()),
            _ => return Err(AnnotationProductionError::InvalidCallable),
        },
        SourceCallable::Getter(id) => {
            Declaration::Accessor(export.property_accessor_identities[id].id())
        }
        SourceCallable::Setter(id) => {
            Declaration::Accessor(export.property_accessor_identities[id].id())
        }
        SourceCallable::StructConstructor(id) => {
            Declaration::Constructor(export.constructor_identities[id].id())
        }
        SourceCallable::ClassConstructor(id) => Declaration::Constructor(
            export.constructor_identities[id]
                .source_record()
                .ok_or(AnnotationProductionError::InvalidCallable)?
                .id(),
        ),
        SourceCallable::VariantConstructor(id) => {
            Declaration::VariantConstructor(export.enum_member_identities[id].id())
        }
    };
    Ok(Callable::Declaration(source))
}
