use crate::{ArrayElementStorageV1, LirTargetProfile, TypeInstanceShapeError, TypeInstanceShapeV1};

/// A checked array element and its complete managed instance layout.
/// The constructor alone binds both views; codegen never computes offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayLayoutV1 {
    storage: ArrayElementStorageV1,
    instance: TypeInstanceShapeV1,
    maximum_count: u64,
}

impl ArrayLayoutV1 {
    pub fn new(
        target: LirTargetProfile,
        storage: ArrayElementStorageV1,
    ) -> Result<Self, TypeInstanceShapeError> {
        let instance = TypeInstanceShapeV1::inline_array(target, storage.clone())?;
        let maximum_count = match &storage {
            ArrayElementStorageV1::ZeroSized { .. } => i64::MAX as u64,
            ArrayElementStorageV1::Inline { stride, .. } => {
                let maximum = target.contract().maximum_managed_object_size()
                    & !(instance.instance_alignment() - 1);
                ((maximum - instance.inline_offset()) / stride.get()).min(i64::MAX as u64)
            }
        };
        Ok(Self {
            storage,
            instance,
            maximum_count,
        })
    }

    pub const fn maximum_count(&self) -> u64 {
        self.maximum_count
    }

    pub fn allocation_size(&self, count: u64) -> Option<u64> {
        if count > self.maximum_count {
            return None;
        }
        match &self.storage {
            ArrayElementStorageV1::ZeroSized { .. } => Some(self.instance.minimum_size()),
            ArrayElementStorageV1::Inline { stride, .. } => {
                let mask = self.instance.instance_alignment() - 1;
                Some((self.instance.inline_offset() + count * stride.get() + mask) & !mask)
            }
        }
    }

    pub const fn storage(&self) -> &ArrayElementStorageV1 {
        &self.storage
    }

    pub const fn instance(&self) -> &TypeInstanceShapeV1 {
        &self.instance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RefScan;

    #[test]
    fn zst_array_count_is_logical_and_allocation_is_constant() {
        let layout = ArrayLayoutV1::new(
            LirTargetProfile::DARWIN_AARCH64,
            ArrayElementStorageV1::zero_sized(16).unwrap(),
        )
        .unwrap();
        for count in [0, 1, i64::MAX as u64] {
            assert_eq!(layout.allocation_size(count), Some(32));
        }
        assert_eq!(layout.allocation_size(i64::MAX as u64 + 1), None);
        assert_eq!(layout.instance().object_scan(), &RefScan::None);
    }

    #[test]
    fn inline_count_limit_accounts_for_tail_padding_and_target_limit() {
        let layout = ArrayLayoutV1::new(
            LirTargetProfile::DARWIN_AARCH64,
            ArrayElementStorageV1::inline(1, 1, RefScan::None).unwrap(),
        )
        .unwrap();
        assert_eq!(layout.allocation_size(0), Some(24));
        assert_eq!(layout.allocation_size(1), Some(32));
        assert_eq!(layout.allocation_size(9), Some(40));
        assert_eq!(
            layout.allocation_size(layout.maximum_count()),
            Some(i64::MAX as u64 & !7)
        );
        assert_eq!(layout.allocation_size(layout.maximum_count() + 1), None);
    }

    #[test]
    fn inline_alignment_and_scan_are_bound_to_one_layout() {
        let layout = ArrayLayoutV1::new(
            LirTargetProfile::DARWIN_AARCH64,
            ArrayElementStorageV1::inline(16, 16, RefScan::References(vec![8])).unwrap(),
        )
        .unwrap();
        assert_eq!(layout.instance().inline_offset(), 32);
        assert_eq!(layout.allocation_size(2), Some(64));
        let RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            ..
        } = layout.instance().object_scan()
        else {
            panic!("reference elements require a nonempty array scan");
        };
        assert_eq!(
            (*length_offset, *first_element_offset, stride.get()),
            (16, 32, 16)
        );
    }
}
