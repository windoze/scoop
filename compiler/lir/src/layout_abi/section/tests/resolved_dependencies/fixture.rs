use super::*;

pub(super) struct Fixture {
    pub(super) local: LayoutAbiExportConstituentsV1,
    pub(super) middle: LayoutAbiExportConstituentsV1,
    pub(super) leaf: LayoutAbiExportConstituentsV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let (leaf, foundation) = empty_struct(cone("owned-lir-leaf"), "Leaf");
        let leaf = exports(&foundation, vec![leaf.into()]);
        let (middle, foundation) = struct_with_field(
            cone("owned-lir-middle"),
            "Middle",
            "leaf",
            &leaf.layouts().records()[0],
        );
        Self {
            local: empty_exports(cone("owned-lir-consumer")),
            middle: exports(&foundation, vec![middle.into()]),
            leaf,
        }
    }

    pub(super) fn leaf_use(&self) -> LayoutAbiDependencyV1 {
        dependency(&self.leaf)
    }

    pub(super) fn middle_use(&self) -> LayoutAbiDependencyV1 {
        dependency(&self.middle)
    }

    pub(super) fn relations(&self) -> Vec<LayoutAbiDependencyV1> {
        let mut result = vec![self.leaf_use(), self.middle_use()];
        result.sort_unstable();
        result
    }

    pub(super) fn dependencies(&self) -> [&LayoutAbiExportConstituentsV1; 2] {
        [&self.middle, &self.leaf]
    }

    pub(super) fn resolve(
        &self,
        semantic: &[LayoutAbiDependencyV1],
    ) -> Result<DependencyResolvedCrossConeLayoutAbiSectionV1, LayoutAbiSectionError> {
        self.resolve_with(&self.local, semantic)
    }

    pub(super) fn resolve_with(
        &self,
        local: &LayoutAbiExportConstituentsV1,
        semantic: &[LayoutAbiDependencyV1],
    ) -> Result<DependencyResolvedCrossConeLayoutAbiSectionV1, LayoutAbiSectionError> {
        let wire = Selection {
            exports: local,
            semantic,
        };
        let bytes = encode(&wire).unwrap();
        let decoded: DecodedCrossConeLayoutAbiSectionV1 = decode_canonical(&bytes).unwrap();
        let exports = decoded
            .validate_layouts(local.layouts())
            .unwrap()
            .validate_callables(local.callables())
            .unwrap()
            .validate_dispatch(local.dispatch())
            .unwrap()
            .validate_descriptors(local.descriptors())
            .unwrap()
            .validate_shape_support(local.shape_support(), local.direct_callables())
            .unwrap();
        assert_eq!(encode(&exports).unwrap(), bytes);
        let mut pending = PendingIdentityValidation::new();
        for table in [&self.local, &self.middle, &self.leaf] {
            pending.register_authority(table.provider()).unwrap();
            for record in table.layouts().records() {
                pending
                    .register_authority(record.identity().layout())
                    .unwrap();
            }
        }
        exports.resolve_dependencies(&mut pending.finish().unwrap())
    }
}

fn dependency(exports: &LayoutAbiExportConstituentsV1) -> LayoutAbiDependencyV1 {
    LayoutAbiDependencyV1::new(
        exports.provider(),
        LayoutAbiSemanticTargetV1::Layout(exports.layouts().records()[0].identity().layout()),
    )
}

struct Selection<'a> {
    exports: &'a LayoutAbiExportConstituentsV1,
    semantic: &'a [LayoutAbiDependencyV1],
}

impl WireEncode for Selection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        field(encoder, 1, self.exports.layouts())?;
        field(encoder, 2, self.exports.descriptors())?;
        field(encoder, 3, self.exports.dispatch())?;
        field(encoder, 4, self.exports.callables())?;
        field(encoder, 5, self.exports.shape_support())?;
        encoder.field(6)?;
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.semantic.len() as u64)?;
        for relation in self.semantic {
            relation.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(0)
    }
}

fn field(
    encoder: &mut Encoder,
    index: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(index)?;
    value.encode(encoder)
}
