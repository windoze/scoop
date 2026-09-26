#include "scoop_runtime_metadata_v1.h"

int main(void) {
    ScoopDescriptorPrefixV1 prefix = {
        SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1,
        SCOOP_RUNTIME_METADATA_ABI_VERSION_V1,
        (uint32_t)sizeof(ScoopImageDescriptorV1),
    };

    if (prefix.magic != UINT64_C(0x53434f4f50494d47) ||
        prefix.abi_version != 1 || prefix.struct_size != 240) {
        return 2;
    }
    if (SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50454e54) ||
        SCOOP_STATIC_STORAGE_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f5053544f) ||
        SCOOP_IMMORTAL_OBJECT_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50494d4d) ||
        SCOOP_INITIALIZATION_UNIT_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50494e49) ||
        SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50545950) ||
        SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50535054) ||
        SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f5043414c)) {
        return 3;
    }
    return 0;
}
