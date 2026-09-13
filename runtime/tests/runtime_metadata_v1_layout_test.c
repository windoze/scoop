#include "scoop_runtime_metadata_v1.h"

#include <string.h>

int main(void) {
    static const uint8_t expected_string_capability[32] = {
        0xd6, 0x96, 0x47, 0x67, 0x50, 0x41, 0xab, 0x4e,
        0x7a, 0x25, 0x50, 0xb9, 0xa5, 0xa2, 0x7b, 0x49,
        0x74, 0x1c, 0x08, 0xb3, 0x0a, 0xb0, 0x7b, 0x25,
        0x9d, 0x99, 0x8e, 0x60, 0x6f, 0x21, 0x8b, 0x77,
    };
    ScoopDescriptorPrefixV1 prefix = {
        SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1,
        SCOOP_RUNTIME_METADATA_ABI_VERSION_V1,
        (uint32_t)sizeof(ScoopImageDescriptorV1),
    };

    if (memcmp(SCOOP_CORE_STRING_CAPABILITY_ID_V1,
               expected_string_capability,
               sizeof(expected_string_capability)) != 0) {
        return 1;
    }
    if (prefix.magic != UINT64_C(0x53434f4f50494d47) ||
        prefix.abi_version != 1 || prefix.struct_size != 240) {
        return 2;
    }
    if (SCOOP_PROGRAM_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50505247) ||
        SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1 != UINT64_C(0x53434f4f50454e54) ||
        SCOOP_RUNTIME_CORE_BINDINGS_MAGIC_V1 != UINT64_C(0x53434f4f50434f52) ||
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
