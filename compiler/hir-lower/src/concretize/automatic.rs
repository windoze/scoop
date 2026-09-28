//! Automatic declaration roots share the nominal representation closure.

use super::*;

pub(super) struct AutomaticNominalRoots(export::NominalMaterializationClosure);

impl AutomaticNominalRoots {
    pub(super) fn new(
        source: &export::Module,
    ) -> Result<Self, export::PublicNominalShapeProjectionError> {
        export::NominalMaterializationClosure::from_current_declarations(source).map(Self)
    }

    fn permits(&self, source: &export::Module, identity: &export::HirNominalIdentity) -> bool {
        match identity {
            export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Concrete(
                record,
            )) => record.key().origin() != source.cone || self.0.contains(record.id()),
            export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Generic(_)) => {
                false
            }
            export::HirNominalIdentity::Generated(_) => true,
        }
    }
}

impl Concretizer<'_> {
    pub(super) fn automatic_nominal(&self, identity: &export::HirNominalIdentity) -> bool {
        self.automatic.permits(self.source, identity)
    }

    pub(super) fn automatic_class(&self, class: export::ClassId) -> bool {
        let identity = self
            .object_by_backing_class
            .get(&class)
            .map_or(&self.source.nominal_identities[class], |object| {
                &self.source.nominal_identities[*object]
            });
        self.automatic_nominal(identity)
    }

    pub(super) fn automatic_type(&self, ty: export::TypeId) -> bool {
        match &self.source.types[ty] {
            export::Type::ImportedStruct(_)
            | export::Type::ImportedEnum(_)
            | export::Type::ImportedClass(_)
            | export::Type::ImportedInterface(_)
            | export::Type::Unit
            | export::Type::Integer(_)
            | export::Type::Boolean
            | export::Type::String
            | export::Type::Any => true,
            export::Type::Param(_) => false,
            export::Type::Struct(application) => {
                let application = &self.source.struct_applications[*application];
                (!application.arguments.is_empty()
                    || self
                        .automatic_nominal(&self.source.nominal_identities[application.template]))
                    && application
                        .arguments
                        .iter()
                        .all(|ty| self.automatic_type(*ty))
            }
            export::Type::Class(application) => {
                let application = &self.source.class_applications[*application];
                (!application.arguments.is_empty() || self.automatic_class(application.template))
                    && application
                        .arguments
                        .iter()
                        .all(|ty| self.automatic_type(*ty))
            }
            export::Type::Interface(application) => {
                let application = &self.source.interface_applications[*application];
                (!application.arguments.is_empty()
                    || self
                        .automatic_nominal(&self.source.nominal_identities[application.template]))
                    && application
                        .arguments
                        .iter()
                        .all(|ty| self.automatic_type(*ty))
            }
            export::Type::Enum(application) => {
                let application = &self.source.enum_applications[*application];
                (!application.arguments.is_empty()
                    || self
                        .automatic_nominal(&self.source.nominal_identities[application.template]))
                    && application
                        .arguments
                        .iter()
                        .all(|ty| self.automatic_type(*ty))
            }
            export::Type::Tuple(elements) => elements.iter().all(|ty| self.automatic_type(*ty)),
            export::Type::Ptr(pointee) => self.automatic_type(*pointee),
            export::Type::Function(function) | export::Type::FunPtr(function) => {
                let function = &self.source.function_types[*function];
                function
                    .parameter_types
                    .iter()
                    .all(|ty| self.automatic_type(*ty))
                    && self.automatic_type(function.return_type)
            }
        }
    }

    pub(super) fn automatic_method(&self, function: export::FunctionId) -> bool {
        let function = &self.source.functions[function];
        let receiver = function.method.expect("nominal members have a receiver");
        // Signatures may request a source-only owner's representation. Direct
        // members need an actual call; dispatch members belong to its tables.
        if !self.automatic_type(receiver.owner) {
            return !matches!(receiver.dispatch, export::MethodDispatch::Direct);
        }
        !function.is_suspend
            && function
                .params
                .iter()
                .all(|param| self.automatic_type(param.ty))
            && self.automatic_type(function.return_ty)
    }

    pub(super) fn automatic_class_constructor(
        &self,
        constructor: export::ClassConstructorId,
    ) -> bool {
        let constructor = &self.source.class_constructors[constructor];
        !self.automatic_class(constructor.owner)
            || constructor
                .parameters
                .iter()
                .all(|parameter| self.automatic_type(parameter.ty))
    }

    pub(super) fn automatic_struct_constructor(
        &self,
        constructor: export::StructConstructorId,
    ) -> bool {
        let constructor = &self.source.struct_constructors[constructor];
        !self.automatic_nominal(&self.source.nominal_identities[constructor.owner])
            || constructor
                .parameters
                .iter()
                .all(|parameter| self.automatic_type(parameter.ty))
    }
}
