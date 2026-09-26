use scoop_lir as lir;

/// Target-dependent physical facts consumed while constructing LIR.
///
/// The complete profile remains the single authority. Derived runtime ABI
/// prefixes are computed from its scalar and pointer layouts rather than kept
/// as a second table of target constants in this stage.
#[derive(Clone, Copy)]
pub(crate) struct LoweringContext {
    target_profile: lir::LirTargetProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalLayout {
    pub(crate) size: u64,
    pub(crate) align: u64,
}

impl PhysicalLayout {
    fn from_scalar(layout: lir::ScalarLayout) -> Self {
        Self {
            size: layout.size_bytes(),
            align: layout.alignment_bytes(),
        }
    }
}

impl LoweringContext {
    pub(crate) const fn new(target_profile: lir::LirTargetProfile) -> Self {
        Self { target_profile }
    }

    pub(crate) const fn target_profile(self) -> lir::LirTargetProfile {
        self.target_profile
    }

    pub(crate) fn scalar_layout(self, kind: lir::BackendScalarKind) -> PhysicalLayout {
        PhysicalLayout::from_scalar(self.target_profile.scalar_layout(kind))
    }

    /// Natural target layout of one exact-width source integer.
    pub(crate) fn integer_layout(self, kind: lir::IntegerKind) -> PhysicalLayout {
        self.scalar_layout(kind.width().backend_scalar_kind())
    }

    /// Every current internal machine-scalar domain still lowers to I64.
    /// Keeping this choice explicit prevents it from being inferred from a
    /// coincidentally pointer-sized target word.
    pub(crate) fn machine_scalar_layout(self) -> PhysicalLayout {
        self.scalar_layout(lir::BackendScalarKind::I64)
    }

    pub(crate) fn pointer_layout(self, kind: lir::PointerKind) -> PhysicalLayout {
        let layout = match kind {
            lir::PointerKind::Managed => self.target_profile.managed_pointer_layout(),
            lir::PointerKind::Raw => self.target_profile.data_pointer().layout(),
            lir::PointerKind::Code => self.target_profile.code_pointer().layout(),
            lir::PointerKind::Metadata => self.target_profile.metadata_pointer_layout(),
        };
        debug_assert_eq!(layout, self.target_profile.pointer_layout(kind));
        PhysicalLayout::from_scalar(layout)
    }

    pub(crate) fn aggregate_layout(
        self,
        fields: impl IntoIterator<Item = PhysicalLayout>,
    ) -> (Vec<u64>, PhysicalLayout) {
        let mut offsets = Vec::new();
        let mut size = 0u64;
        let mut align = 1u64;
        for field in fields {
            size = size.next_multiple_of(field.align);
            offsets.push(size);
            size += field.size;
            align = align.max(field.align);
        }
        (
            offsets,
            PhysicalLayout {
                size: size.next_multiple_of(align),
                align,
            },
        )
    }

    /// Runtime object header `{ metadata-ptr td, i64 gc-word }`.
    pub(crate) fn object_header_layout(self) -> PhysicalLayout {
        self.aggregate_layout([
            self.pointer_layout(lir::PointerKind::Metadata),
            self.scalar_layout(lir::BackendScalarKind::I64),
        ])
        .1
    }

    pub(crate) fn object_type_descriptor_offset(self) -> u64 {
        self.aggregate_layout([
            self.pointer_layout(lir::PointerKind::Metadata),
            self.scalar_layout(lir::BackendScalarKind::I64),
        ])
        .0[0]
    }

    /// Runtime String fixed prefix `{ object-header, i64 length }`.
    pub(crate) fn string_layout(self) -> PhysicalLayout {
        self.aggregate_layout([
            self.object_header_layout(),
            self.scalar_layout(lir::BackendScalarKind::I64),
        ])
        .1
    }

    /// Closure fixed prefix `{ object-header, code-ptr invoke }`.
    pub(crate) fn closure_prefix(self) -> (u64, PhysicalLayout) {
        let (offsets, layout) = self.aggregate_layout([
            self.object_header_layout(),
            self.pointer_layout(lir::PointerKind::Code),
        ]);
        (offsets[1], layout)
    }

    /// Closure dispatch is represented in LIR as an index into LLVM's
    /// default-address-space pointer slots rooted at the object address.
    pub(crate) fn closure_invoke_dispatch_slot(self) -> u32 {
        let (invoke_offset, _) = self.closure_prefix();
        let pointer_stride = self.pointer_layout(lir::PointerKind::Raw).size;
        assert_eq!(
            invoke_offset % pointer_stride,
            0,
            "the qualified closure invoke field must start on a pointer slot"
        );
        u32::try_from(invoke_offset / pointer_stride)
            .expect("the qualified closure invoke slot fits the LIR slot domain")
    }

    /// Current runtime metadata descriptor prefix: type id, complete instance
    /// shape, object scan, parent, and vtable (88 bytes on the 64-bit runtime).
    pub(crate) fn type_descriptor_vtable_offset(self) -> u64 {
        let i32_layout = self.scalar_layout(lir::BackendScalarKind::I32);
        let i64_layout = self.scalar_layout(lir::BackendScalarKind::I64);
        let metadata_pointer = self.pointer_layout(lir::PointerKind::Metadata);
        let instance_shape = self
            .aggregate_layout([
                i32_layout,
                i32_layout,
                i64_layout,
                i64_layout,
                i64_layout,
                i64_layout,
                i64_layout,
                i64_layout,
                metadata_pointer,
            ])
            .1;
        self.aggregate_layout([
            i64_layout,
            instance_shape,
            metadata_pointer,
            metadata_pointer,
            metadata_pointer,
        ])
        .0[4]
    }

    /// LLVM landing-pad record `{ raw-ptr exception, i32 selector }`.
    pub(crate) fn exception_record_layout(self) -> PhysicalLayout {
        self.aggregate_layout([
            self.pointer_layout(lir::PointerKind::Raw),
            self.scalar_layout(lir::BackendScalarKind::I32),
        ])
        .1
    }
}
