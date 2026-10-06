use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn prepare_tuple_encoding(&mut self, owner: concrete::TypeId) {
        if self.tuple_interface_implementations.contains_key(&owner) {
            return;
        }
        let concrete::TypeKind::Tuple(elements) = self.types[owner].kind.clone() else {
            unreachable!("tuple encoding is requested for a tuple type")
        };
        let Some(template) = self
            .source
            .tuple_encoding_templates
            .values()
            .find(|template| {
                self.source.functions[template.function].type_param_count() == elements.len()
            })
            .cloned()
        else {
            return;
        };
        let export::Type::Interface(interface) = self.source.types[template.interface] else {
            unreachable!("tuple encoding names the actual Encodable interface")
        };
        let source_interface = self.source.interface_applications[interface].template;
        if !elements
            .iter()
            .all(|element| self.element_has_encoding(*element, source_interface))
        {
            return;
        }
        let interface = self.lower_interface_type(template.interface, &[]);
        let source_slot = self.interface_reference_slot(template.member);
        let slot = self.interface_slot_by_source[&(interface, source_slot)];
        let function = self.request_method(
            template.function,
            concrete::MethodOwner::TypeOwned(owner),
            elements,
        );
        self.tuple_interface_implementations.insert(
            owner,
            concrete::InterfaceImplementation {
                interface,
                methods: vec![concrete::InterfaceMethodImplementation {
                    slot,
                    target: concrete::InterfaceImplementationTarget::Method(function),
                }],
            },
        );
    }
}
