/* Private Scoop runtime metadata ABI v1.
 *
 * This header is the canonical C layout shared by compiler-emitted metadata,
 * object verification, and the future multi-image runtime consumer. It is not
 * part of the native FFI authoring surface in scoop_rt.h.
 */
#ifndef SCOOP_RUNTIME_METADATA_V1_H
#define SCOOP_RUNTIME_METADATA_V1_H

#include <stddef.h>
#include <stdint.h>

#if UINTPTR_MAX != UINT64_MAX
#error "Scoop runtime metadata ABI v1 requires 64-bit pointers"
#endif

#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error "Scoop runtime metadata ABI v1 requires little-endian byte order"
#endif

#define SCOOP_RUNTIME_METADATA_ABI_VERSION_V1 UINT32_C(2)

/* Unused program/core record magic values 0x53434f4f50505247 and
 * 0x53434f4f50434f52 are retired and must not be reused. */

#define SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50494d47)
#define SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50454e54)
#define SCOOP_STATIC_STORAGE_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f5053544f)
#define SCOOP_IMMORTAL_OBJECT_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50494d4d)
#define SCOOP_INITIALIZATION_UNIT_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50494e49)
#define SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50545950)
#define SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f50535054)
#define SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1 UINT64_C(0x53434f4f5043414c)

#define SCOOP_REGISTRATION_LINKAGE_STRONG_V1 UINT32_C(1)
#define SCOOP_REGISTRATION_LINKAGE_ODR_V1 UINT32_C(2)

#define SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1 UINT32_C(1)
#define SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1 UINT32_C(2)
#define SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1 UINT32_C(3)
#define SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1 UINT32_C(4)
#define SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1 UINT32_C(5)

#define SCOOP_INLINE_STORAGE_NONE_V1 UINT32_C(0)
#define SCOOP_INLINE_STORAGE_INLINE_V1 UINT32_C(1)
#define SCOOP_INLINE_STORAGE_ZERO_SIZED_V1 UINT32_C(2)

#define SCOOP_STATIC_SCAN_NONE_V1 UINT32_C(0)
#define SCOOP_STATIC_SCAN_RECURSIVE_V1 UINT32_C(1)
#define SCOOP_STATIC_INITIAL_ZEROED_FOR_RUNTIME_UNIT_V1 UINT32_C(1)
#define SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1 UINT32_C(2)

#define SCOOP_INITIALIZATION_EAGER_STARTUP_V1 UINT32_C(1)
#define SCOOP_INITIALIZATION_LAZY_ACCESS_V1 UINT32_C(2)

#define SCOOP_SAFEPOINT_MANAGED_POLL_V1 UINT32_C(1)
#define SCOOP_SAFEPOINT_MANAGED_CALL_V1 UINT32_C(2)
#define SCOOP_SAFEPOINT_MANAGED_INVOKE_V1 UINT32_C(3)
#define SCOOP_SAFEPOINT_NATIVE_SAFE_TRANSITION_V1 UINT32_C(4)
#define SCOOP_SAFEPOINT_NATIVE_BORROWED_TRANSITION_V1 UINT32_C(5)

typedef struct ScoopDescriptorPrefixV1 {
    uint64_t magic;
    uint32_t abi_version;
    uint32_t struct_size;
} ScoopDescriptorPrefixV1;

typedef struct ScoopDigest256V1 {
    uint8_t bytes[32];
} ScoopDigest256V1;

typedef struct ScoopByteSpanV1 {
    const uint8_t *data;
    uint64_t length;
} ScoopByteSpanV1;

typedef struct ScoopConeRecordV1 {
    ScoopByteSpanV1 group;
    ScoopByteSpanV1 name;
    ScoopByteSpanV1 version;
    ScoopDigest256V1 identity;
} ScoopConeRecordV1;

typedef struct ScoopInitializationCell {
    uint64_t state;
    void *owner_thread;
} ScoopInitializationCell;

typedef struct ScoopTypeInstanceShapeV1 {
    uint32_t instance_kind;
    uint32_t inline_storage_kind;
    uint64_t minimum_size;
    uint64_t instance_alignment;
    uint64_t inline_offset;
    uint64_t inline_size;
    uint64_t inline_stride;
    uint64_t inline_alignment;
    const uint64_t *inline_scan;
} ScoopTypeInstanceShapeV1;

typedef struct ScoopTypeDescriptor ScoopTypeDescriptor;

typedef struct ScoopItableEntryV1 {
    const ScoopTypeDescriptor *interface;
    const void *const *slots;
} ScoopItableEntryV1;

struct ScoopTypeDescriptor {
    uint64_t type_id;
    ScoopTypeInstanceShapeV1 instance_shape;
    const uint64_t *object_scan;
    const ScoopTypeDescriptor *parent;
    const void *const *vtable;
    const ScoopItableEntryV1 *itables;
    uint64_t itable_count;
    ScoopByteSpanV1 diagnostic_name;
    uint32_t relation_kind;
    uint32_t related_type_count;
    const ScoopTypeDescriptor *function_result;
    const ScoopTypeDescriptor *related_types[];
};

typedef struct ScoopRegistrationIdentityV1 {
    uint32_t linkage_kind;
    uint32_t reserved_zero;
    ScoopDigest256V1 semantic_id;
    ScoopDigest256V1 odr_group_id;
    ScoopDigest256V1 odr_member_id;
    ScoopDigest256V1 definition_fingerprint;
} ScoopRegistrationIdentityV1;

typedef struct ScoopImmortalObjectDescriptorV1 ScoopImmortalObjectDescriptorV1;

typedef struct ScoopStaticImmortalRelocationV1 {
    uint64_t pointer_offset;
    const ScoopImmortalObjectDescriptorV1 *target;
} ScoopStaticImmortalRelocationV1;

typedef struct ScoopStaticStorageDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint32_t scan_kind;
    uint32_t initial_state_kind;
    void *writable_base;
    uint64_t byte_size;
    uint64_t allocation_extent;
    uint64_t required_alignment;
    const uint64_t *scan_program;
    ScoopDigest256V1 scan_fingerprint;
    ScoopDigest256V1 layout_fingerprint;
    ScoopByteSpanV1 initial_template;
    const ScoopStaticImmortalRelocationV1 *initial_relocations;
    uint64_t initial_relocation_count;
} ScoopStaticStorageDescriptorV1;

typedef struct ScoopTypeRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint64_t runtime_type_id;
    uint64_t reserved_zero;
    const ScoopTypeDescriptor *descriptor;
    ScoopDigest256V1 descriptor_fingerprint;
    ScoopDigest256V1 layout_fingerprint;
} ScoopTypeRegistrationDescriptorV1;

typedef void (*ScoopCallableAddressV1)(void);

typedef struct ScoopCallableRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    ScoopDigest256V1 body_definition_fingerprint;
    ScoopCallableAddressV1 entry;
} ScoopCallableRegistrationDescriptorV1;

struct ScoopImmortalObjectDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    const void *object_start;
    uint64_t object_size;
    uint64_t required_alignment;
    const ScoopTypeRegistrationDescriptorV1 *type_registration;
};

typedef void (*ScoopManagedUnitEntryFnV1)(void);

typedef struct ScoopInitializationUnitDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint32_t schedule_kind;
    uint32_t reserved_zero;
    ScoopByteSpanV1 diagnostic_path;
    ScoopInitializationCell *cell;
    const ScoopStaticStorageDescriptorV1 *storage;
    const ScoopStaticStorageDescriptorV1 *failure_root;
    ScoopDigest256V1 initializer_callable_id;
    ScoopDigest256V1 ensure_callable_id;
    ScoopManagedUnitEntryFnV1 initializer_entry;
    ScoopManagedUnitEntryFnV1 ensure_entry;
    ScoopDigest256V1 startup_gateway_callable_id;
    ScoopDigest256V1 startup_gateway_definition_fingerprint;
    uint32_t (*startup_gateway)(void);
} ScoopInitializationUnitDescriptorV1;

typedef struct ScoopSafepointRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint64_t safepoint_id;
    uint32_t site_role;
    uint32_t root_pair_count;
    ScoopDigest256V1 owner_callable_id;
    ScoopDigest256V1 normalized_stackmap_fingerprint;
} ScoopSafepointRegistrationDescriptorV1;

typedef uint32_t (*ScoopRootEntryGatewayFnV1)(void);

typedef struct ScoopRootEntryDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopDigest256V1 owner_cone_identity;
    ScoopDigest256V1 callable_id;
    ScoopDigest256V1 source_signature_fingerprint;
    ScoopDigest256V1 gateway_callable_id;
    ScoopDigest256V1 gateway_definition_fingerprint;
    const ScoopStaticStorageDescriptorV1 *failure_root;
    ScoopRootEntryGatewayFnV1 gateway;
} ScoopRootEntryDescriptorV1;

typedef struct ScoopImageDescriptorV1 ScoopImageDescriptorV1;

struct ScoopImageDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopConeRecordV1 cone;
    ScoopDigest256V1 runtime_image_fingerprint;
    const ScoopDigest256V1 *dependencies;
    uint64_t dependency_count;
    const ScoopStaticStorageDescriptorV1 *const *static_storages;
    uint64_t static_storage_count;
    const ScoopImmortalObjectDescriptorV1 *const *immortal_objects;
    uint64_t immortal_object_count;
    const ScoopInitializationUnitDescriptorV1 *const *initialization_units;
    uint64_t initialization_unit_count;
    const ScoopTypeRegistrationDescriptorV1 *const *type_registrations;
    uint64_t type_registration_count;
    const ScoopSafepointRegistrationDescriptorV1 *const *safepoints;
    uint64_t safepoint_count;
    const ScoopCallableRegistrationDescriptorV1 *const *callables;
    uint64_t callable_count;
};

#define SCOOP_METADATA_ASSERT_LAYOUT(type, size, alignment) \
    _Static_assert(sizeof(type) == (size), #type " size"); \
    _Static_assert(_Alignof(type) == (alignment), #type " alignment")

#define SCOOP_METADATA_ASSERT_OFFSET(type, field, offset) \
    _Static_assert(offsetof(type, field) == (offset), #type "." #field " offset")

_Static_assert(sizeof(void *) == 8, "runtime metadata pointer size");
_Static_assert(_Alignof(void *) == 8, "runtime metadata pointer alignment");
SCOOP_METADATA_ASSERT_LAYOUT(ScoopDescriptorPrefixV1, 16, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopDescriptorPrefixV1, magic, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopDescriptorPrefixV1, abi_version, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopDescriptorPrefixV1, struct_size, 12);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopDigest256V1, 32, 1);
SCOOP_METADATA_ASSERT_OFFSET(ScoopDigest256V1, bytes, 0);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopByteSpanV1, 16, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopByteSpanV1, data, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopByteSpanV1, length, 8);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopConeRecordV1, 80, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopConeRecordV1, group, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopConeRecordV1, name, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopConeRecordV1, version, 32);
SCOOP_METADATA_ASSERT_OFFSET(ScoopConeRecordV1, identity, 48);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopInitializationCell, 16, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationCell, state, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationCell, owner_thread, 8);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopTypeInstanceShapeV1, 64, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, instance_kind, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_storage_kind, 4);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, minimum_size, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, instance_alignment, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_offset, 24);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_size, 32);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_stride, 40);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_alignment, 48);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeInstanceShapeV1, inline_scan, 56);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopItableEntryV1, 16, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopItableEntryV1, interface, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopItableEntryV1, slots, 8);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopTypeDescriptor, 144, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, type_id, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, instance_shape, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, object_scan, 72);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, parent, 80);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, vtable, 88);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, itables, 96);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, itable_count, 104);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, diagnostic_name, 112);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, relation_kind, 128);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, related_type_count, 132);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, function_result, 136);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeDescriptor, related_types, 144);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopRegistrationIdentityV1, 136, 4);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, linkage_kind, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, reserved_zero, 4);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, semantic_id, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, odr_group_id, 40);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, odr_member_id, 72);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRegistrationIdentityV1, definition_fingerprint, 104);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopStaticImmortalRelocationV1, 16, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticImmortalRelocationV1, pointer_offset, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticImmortalRelocationV1, target, 8);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopStaticStorageDescriptorV1, 296, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, scan_kind, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, initial_state_kind, 156);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, writable_base, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, byte_size, 168);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, allocation_extent, 176);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, required_alignment, 184);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, scan_program, 192);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, scan_fingerprint, 200);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, layout_fingerprint, 232);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, initial_template, 264);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, initial_relocations, 280);
SCOOP_METADATA_ASSERT_OFFSET(ScoopStaticStorageDescriptorV1, initial_relocation_count, 288);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopTypeRegistrationDescriptorV1, 240, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, runtime_type_id, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, reserved_zero, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, descriptor, 168);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, descriptor_fingerprint, 176);
SCOOP_METADATA_ASSERT_OFFSET(ScoopTypeRegistrationDescriptorV1, layout_fingerprint, 208);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopCallableRegistrationDescriptorV1, 192, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopCallableRegistrationDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopCallableRegistrationDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopCallableRegistrationDescriptorV1,
                             body_definition_fingerprint, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopCallableRegistrationDescriptorV1, entry, 184);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopImmortalObjectDescriptorV1, 184, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, object_start, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, object_size, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, required_alignment, 168);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImmortalObjectDescriptorV1, type_registration, 176);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopInitializationUnitDescriptorV1, 352, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, schedule_kind, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, reserved_zero, 156);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, diagnostic_path, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, cell, 176);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, storage, 184);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, failure_root, 192);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, initializer_callable_id, 200);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, ensure_callable_id, 232);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, initializer_entry, 264);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, ensure_entry, 272);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1,
                             startup_gateway_callable_id, 280);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1,
                             startup_gateway_definition_fingerprint, 312);
SCOOP_METADATA_ASSERT_OFFSET(ScoopInitializationUnitDescriptorV1, startup_gateway, 344);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopSafepointRegistrationDescriptorV1, 232, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, registration, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, safepoint_id, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, site_role, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, root_pair_count, 164);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1, owner_callable_id, 168);
SCOOP_METADATA_ASSERT_OFFSET(ScoopSafepointRegistrationDescriptorV1,
                             normalized_stackmap_fingerprint, 200);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopRootEntryDescriptorV1, 192, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, owner_cone_identity, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, callable_id, 48);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1,
                             source_signature_fingerprint, 80);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, gateway_callable_id, 112);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1,
                             gateway_definition_fingerprint, 144);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, failure_root, 176);
SCOOP_METADATA_ASSERT_OFFSET(ScoopRootEntryDescriptorV1, gateway, 184);
SCOOP_METADATA_ASSERT_LAYOUT(ScoopImageDescriptorV1, 240, 8);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, prefix, 0);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, cone, 16);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, runtime_image_fingerprint, 96);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, dependencies, 128);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, dependency_count, 136);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, static_storages, 144);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, static_storage_count, 152);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, immortal_objects, 160);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, immortal_object_count, 168);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, initialization_units, 176);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, initialization_unit_count, 184);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, type_registrations, 192);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, type_registration_count, 200);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, safepoints, 208);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, safepoint_count, 216);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, callables, 224);
SCOOP_METADATA_ASSERT_OFFSET(ScoopImageDescriptorV1, callable_count, 232);

#undef SCOOP_METADATA_ASSERT_OFFSET
#undef SCOOP_METADATA_ASSERT_LAYOUT

#endif /* SCOOP_RUNTIME_METADATA_V1_H */
