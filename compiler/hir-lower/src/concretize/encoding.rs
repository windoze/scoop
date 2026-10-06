//! Apply the checked container condition before requesting methods and itables.

use super::*;

mod tuples;

impl Concretizer<'_> {
    pub(super) fn request_encoding_methods(
        &mut self,
        methods: &[export::FunctionId],
        owner: concrete::MethodOwner,
        disabled: Option<&export::ElementEncoding>,
    ) -> Vec<concrete::FunctionId> {
        let Some(disabled) = disabled else {
            return self.request_concrete_methods(methods, owner);
        };
        let methods = methods
            .iter()
            .copied()
            .filter(|function| {
                !disabled.implementation.methods.iter().any(|method| {
                matches!(method.target, export::InterfaceImplementationTarget::Method(application)
                    if self.source.method_applications[application].function == *function)
            })
            })
            .collect::<Vec<_>>();
        self.request_concrete_methods(&methods, owner)
    }

    pub(super) fn concrete_encoding_applies(
        &mut self,
        encoding: &export::ElementEncoding,
        substitution: &[concrete::TypeId],
    ) -> bool {
        let export::Type::Interface(interface) =
            self.source.types[encoding.implementation.interface]
        else {
            unreachable!("the checked encoding condition names an interface")
        };
        let interface = self.source.interface_applications[interface].template;
        let element = self.lower_type(encoding.element, substitution);
        self.element_has_encoding(element, interface)
    }

    fn element_has_encoding(
        &mut self,
        element: concrete::TypeId,
        interface: export::SourceNominalId,
    ) -> bool {
        // Recursive concrete declarations may still be under construction.
        // Their complete checked source conformances already determine this condition.
        let (implementations, conditional) = match self.types[element].kind {
            concrete::TypeKind::Tuple(ref elements) => {
                let elements = elements.clone();
                return !elements.is_empty()
                    && elements
                        .into_iter()
                        .all(|element| self.element_has_encoding(element, interface));
            }
            concrete::TypeKind::Class(id) => {
                let value = &self.classes[id];
                let source = self.source.class_definition(value.origin.declaration_id());
                (
                    &source.interface_implementations,
                    source
                        .element_encoding
                        .as_ref()
                        .map(|encoding| (encoding, value.type_arguments.clone())),
                )
            }
            concrete::TypeKind::Enum(id) => {
                let value = &self.enums[id];
                let source = self.source.enum_definition(value.origin.declaration_id());
                (
                    &source.interface_implementations,
                    source
                        .element_encoding
                        .as_ref()
                        .map(|encoding| (encoding, value.type_arguments.clone())),
                )
            }
            concrete::TypeKind::Struct(id) => {
                let source = self
                    .source
                    .struct_definition(self.structs[id].origin.declaration_id());
                (&source.interface_implementations, None)
            }
            concrete::TypeKind::Interface(id) => {
                return self.interface_inherits_encoding(
                    self.interfaces[id].origin.declaration_id(),
                    interface,
                );
            }
            concrete::TypeKind::Unit => (
                self.scalar_encoding_implementations(export::IntrinsicTypeKind::Unit),
                None,
            ),
            concrete::TypeKind::Integer(kind) => (
                self.scalar_encoding_implementations(export::IntrinsicTypeKind::Integer(kind)),
                None,
            ),
            concrete::TypeKind::Boolean => (
                self.scalar_encoding_implementations(export::IntrinsicTypeKind::Boolean),
                None,
            ),
            concrete::TypeKind::String => (
                self.scalar_encoding_implementations(export::IntrinsicTypeKind::String),
                None,
            ),
            _ => return false,
        };
        implementations.iter().any(|implementation| {
            let export::Type::Interface(application) = self.source.types[implementation.interface]
            else {
                unreachable!("checked conformances name interfaces")
            };
            self.source.interface_applications[application].template == interface
        }) || conditional.is_some_and(|(encoding, arguments)| {
            self.concrete_encoding_applies(encoding, &arguments)
        })
    }

    fn interface_inherits_encoding(
        &self,
        source: export::SourceNominalId,
        target: export::SourceNominalId,
    ) -> bool {
        source == target
            || self
                .source
                .interface_definition(source)
                .parents
                .iter()
                .any(|parent| {
                    let export::Type::Interface(parent) = self.source.types[*parent] else {
                        unreachable!("checked interface parents are interfaces")
                    };
                    self.interface_inherits_encoding(
                        self.source.interface_applications[parent].template,
                        target,
                    )
                })
    }
}

impl<'input> Concretizer<'input> {
    fn scalar_encoding_implementations(
        &self,
        family: export::IntrinsicTypeKind,
    ) -> &'input Vec<export::InterfaceImplementation> {
        match self.core {
            export::CoreProtocols::Defined(protocols) => {
                let types = protocols.fundamental_types;
                let owner = match family {
                    export::IntrinsicTypeKind::Unit => types.unit,
                    export::IntrinsicTypeKind::Integer(kind) => types.integers.owner(kind),
                    export::IntrinsicTypeKind::Boolean => types.boolean,
                    export::IntrinsicTypeKind::String => {
                        return &self.source.classes[types.string].interface_implementations;
                    }
                    _ => unreachable!("only scalar encoding needs representation lookup"),
                };
                &self.source.structs[owner].interface_implementations
            }
            export::CoreProtocols::Imported(_) => {
                &self.source.imported_intrinsic_types[&family].interface_implementations
            }
        }
    }
}
