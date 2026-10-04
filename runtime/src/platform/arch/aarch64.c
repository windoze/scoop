#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "../platform.h"

#if UINTPTR_MAX != UINT64_MAX
#error "the AArch64 frame decoder requires 64-bit addresses"
#endif

#define AARCH64_DWARF_FP 29
#define AARCH64_DWARF_SP 31
#define AARCH64_FRAME_RECORD_SIZE UINT64_C(16)

static bool address_add(uintptr_t base, int64_t offset, uintptr_t *result) {
    if (offset >= 0) {
        if ((uint64_t)offset > UINTPTR_MAX - base) {
            return false;
        }
        *result = base + (uintptr_t)offset;
        return true;
    }
    uint64_t magnitude = (uint64_t)(-(offset + 1)) + 1;
    if (magnitude > base) {
        return false;
    }
    *result = base - (uintptr_t)magnitude;
    return true;
}

static bool range_contains(ScoopPlatformStackBounds bounds, uintptr_t address,
                           size_t size) {
    uintptr_t low = (uintptr_t)bounds.low;
    uintptr_t high = (uintptr_t)bounds.high;
    return low < high && address >= low && address <= high &&
           size <= high - address;
}

static bool location_frame_offset(const ScoopStackMapRecord *record,
                                  const ScoopStackMapLocation *location,
                                  uint64_t *offset) {
    uint64_t base = location->dwarf_register == AARCH64_DWARF_SP
                        ? 0
                        : record->stack_size - AARCH64_FRAME_RECORD_SIZE;
    uint64_t resolved;
    if (location->offset < 0) {
        uint64_t magnitude = (uint64_t)(-(int64_t)location->offset);
        if (magnitude > base)
            return false;
        resolved = base - magnitude;
    } else {
        uint64_t amount = (uint64_t)location->offset;
        if (amount > record->stack_size - base)
            return false;
        resolved = base + amount;
    }
    if (resolved > record->stack_size - 8) {
        return false;
    }
    *offset = resolved;
    return true;
}

static bool locations_equal(const ScoopStackMapLocation *left,
                            const ScoopStackMapLocation *right) {
    return left->kind == right->kind && left->size == right->size &&
           left->dwarf_register == right->dwarf_register &&
           left->offset == right->offset && left->constant == right->constant;
}

static bool aarch64_validate_record(const ScoopStackMapRecord *record,
                                    ScoopPlatformError *error) {
    if (record == NULL || error == NULL ||
        record->stack_size < AARCH64_FRAME_RECORD_SIZE ||
        record->stack_size % 16 != 0 || record->stack_size > INT64_MAX ||
        (record->root_count != 0 && record->roots == NULL)) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        }
        return false;
    }
    for (uint16_t index = 0; index < record->root_count; index++) {
        const ScoopStackMapRootPair *pair = &record->roots[index];
        const ScoopStackMapLocation *base = &pair->base;
        const ScoopStackMapLocation *derived = &pair->derived;
        if (base->kind != SCOOP_STACKMAP_INDIRECT || base->size != 8 ||
            (base->dwarf_register != AARCH64_DWARF_SP &&
             base->dwarf_register != AARCH64_DWARF_FP) ||
            !locations_equal(base, derived)) {
            error->code = SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION;
            error->safepoint_id = record->safepoint_id;
            error->root_index = index;
            return false;
        }
        uint64_t frame_offset;
        if (!location_frame_offset(record, base, &frame_offset)) {
            error->code = SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME;
            error->safepoint_id = record->safepoint_id;
            error->root_index = index;
            return false;
        }
    }
    return true;
}

static bool aarch64_frame_from_anchor(const ScoopManagedAnchor *anchor,
                                      const ScoopStackMapRecord *record,
                                      ScoopPlatformStackBounds bounds,
                                      ScoopManagedFrame *frame,
                                      ScoopPlatformError *error) {
    bool frame_end_overflows =
        anchor != NULL &&
        anchor->frame_pointer > UINTPTR_MAX - AARCH64_FRAME_RECORD_SIZE;
    bool stack_end_overflows =
        anchor != NULL && record != NULL &&
        record->stack_size > UINTPTR_MAX - anchor->stack_pointer;
    if (anchor == NULL || record == NULL || frame == NULL || error == NULL ||
        frame_end_overflows || stack_end_overflows ||
        anchor->return_pc != record->return_pc ||
        !range_contains(bounds, anchor->stack_pointer, record->stack_size) ||
        anchor->frame_pointer < anchor->stack_pointer ||
        anchor->frame_pointer + AARCH64_FRAME_RECORD_SIZE !=
            anchor->stack_pointer + record->stack_size) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_ANCHOR;
            error->safepoint_id = record == NULL ? 0 : record->safepoint_id;
        }
        return false;
    }
    *frame = (ScoopManagedFrame){
        .return_pc = anchor->return_pc,
        .stack_pointer = anchor->stack_pointer,
        .frame_pointer = anchor->frame_pointer,
        .record = record,
    };
    return true;
}

static bool aarch64_resolve_root(const ScoopManagedFrame *frame,
                                 uint16_t root_index, void ***slot,
                                 ScoopPlatformError *error) {
    if (frame == NULL || frame->record == NULL || slot == NULL ||
        error == NULL || root_index >= frame->record->root_count) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        }
        return false;
    }
    const ScoopStackMapLocation *location =
        &frame->record->roots[root_index].base;
    if (location->kind != SCOOP_STACKMAP_INDIRECT || location->size != 8 ||
        (location->dwarf_register != AARCH64_DWARF_SP &&
         location->dwarf_register != AARCH64_DWARF_FP)) {
        error->code = SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION;
        error->safepoint_id = frame->record->safepoint_id;
        error->root_index = root_index;
        return false;
    }
    uintptr_t base = location->dwarf_register == AARCH64_DWARF_SP
                         ? frame->stack_pointer
                         : frame->frame_pointer;
    uintptr_t address;
    uint64_t frame_offset;
    if (!location_frame_offset(frame->record, location, &frame_offset) ||
        !address_add(base, location->offset, &address) ||
        address != frame->stack_pointer + frame_offset) {
        error->code = SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME;
        error->safepoint_id = frame->record->safepoint_id;
        error->root_index = root_index;
        return false;
    }
    *slot = (void **)address;
    return true;
}

static bool aarch64_next_frame(const ScoopManagedFrame *frame,
                               uintptr_t managed_boundary,
                               ScoopPlatformStackBounds bounds,
                               uintptr_t *return_pc, uintptr_t *stack_pointer,
                               uintptr_t *frame_pointer, bool *has_next,
                               ScoopPlatformError *error) {
    if (frame == NULL || return_pc == NULL || stack_pointer == NULL ||
        frame_pointer == NULL || has_next == NULL || error == NULL ||
        managed_boundary <= frame->frame_pointer ||
        managed_boundary > (uintptr_t)bounds.high ||
        !range_contains(bounds, frame->frame_pointer,
                        AARCH64_FRAME_RECORD_SIZE)) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
            error->safepoint_id = frame == NULL || frame->record == NULL
                                      ? 0
                                      : frame->record->safepoint_id;
        }
        return false;
    }
    uintptr_t frame_record[2];
    memcpy(frame_record, (const void *)frame->frame_pointer,
           sizeof frame_record);
    uintptr_t next_stack_pointer =
        frame->frame_pointer + AARCH64_FRAME_RECORD_SIZE;
    uintptr_t next_frame_pointer = frame_record[0];
    if (next_frame_pointer >= managed_boundary) {
        *has_next = false;
        return true;
    }
    if (next_frame_pointer <= frame->frame_pointer ||
        next_stack_pointer >= managed_boundary || frame_record[1] == 0 ||
        !range_contains(bounds, next_frame_pointer,
                        AARCH64_FRAME_RECORD_SIZE)) {
        error->code = SCOOP_PLATFORM_INVALID_FRAME;
        error->safepoint_id =
            frame->record == NULL ? 0 : frame->record->safepoint_id;
        return false;
    }
    *return_pc = frame_record[1];
    *stack_pointer = next_stack_pointer;
    *frame_pointer = next_frame_pointer;
    *has_next = true;
    return true;
}

const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops = {
    .validate_record = aarch64_validate_record,
    .frame_from_anchor = aarch64_frame_from_anchor,
    .resolve_root = aarch64_resolve_root,
    .next_frame = aarch64_next_frame,
};
