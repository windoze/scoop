use super::*;

impl Fixture {
    /// Every frozen tag and typed child is specified independently of encoding.
    pub fn cases(&self) -> Vec<(SelectedTypeUseV1, Vec<u8>)> {
        use InheritanceCallableDeclarationV1 as Member;
        use SelectedDirectInheritanceEdgeV1 as Edge;
        use SelectedTypeConstructionV1 as Construction;
        use SelectedTypeUseV1 as Use;
        vec![
            (
                Use::Signature { exact: self.owner },
                sum(1, self.owner.as_array(), None),
            ),
            (
                Use::Representation { exact: self.owner },
                sum(2, self.owner.as_array(), None),
            ),
            (
                Use::Construct {
                    exact: self.owner,
                    declaration: Construction::Constructor(self.constructor),
                },
                sum(
                    3,
                    self.owner.as_array(),
                    Some(sum(1, self.constructor.as_array(), None)),
                ),
            ),
            (
                Use::Construct {
                    exact: self.enumeration,
                    declaration: Construction::EnumVariant(self.variant),
                },
                sum(
                    3,
                    self.enumeration.as_array(),
                    Some(sum(2, self.variant.as_array(), None)),
                ),
            ),
            (
                Use::MemberCall {
                    receiver: self.owner,
                    declaration: Member::Function(self.function),
                },
                sum(
                    4,
                    self.owner.as_array(),
                    Some(sum(1, self.function.as_array(), None)),
                ),
            ),
            (
                Use::MemberCall {
                    receiver: self.owner,
                    declaration: Member::Getter(self.getter),
                },
                sum(
                    4,
                    self.owner.as_array(),
                    Some(sum(2, self.getter.as_array(), None)),
                ),
            ),
            (
                Use::MemberCall {
                    receiver: self.owner,
                    declaration: Member::Setter(self.setter),
                },
                sum(
                    4,
                    self.owner.as_array(),
                    Some(sum(3, self.setter.as_array(), None)),
                ),
            ),
            (
                Use::SlotCall {
                    receiver: self.owner,
                    slot: self.slot,
                },
                sum(
                    5,
                    self.owner.as_array(),
                    Some(id_wire(self.slot.as_array())),
                ),
            ),
            (
                Use::TypeTest { exact: self.owner },
                sum(6, self.owner.as_array(), None),
            ),
            (
                Use::SingletonValue {
                    exact: self.object_exact,
                    value: self.object,
                },
                sum(
                    7,
                    self.object_exact.as_array(),
                    Some(id_wire(self.object.as_array())),
                ),
            ),
            (
                Use::Inheritance {
                    derived: self.derived,
                    edge: Edge::ClassBase { exact: self.owner },
                },
                sum(
                    8,
                    self.derived.as_array(),
                    Some(sum(1, self.owner.as_array(), None)),
                ),
            ),
            (
                Use::Inheritance {
                    derived: self.derived,
                    edge: Edge::Interface {
                        exact: self.interface,
                    },
                },
                sum(
                    8,
                    self.derived.as_array(),
                    Some(sum(2, self.interface.as_array(), None)),
                ),
            ),
            (
                Use::ShapeSupport { exact: self.owner },
                sum(9, self.owner.as_array(), None),
            ),
        ]
    }
}
