#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "../platform.h"

#if UINTPTR_MAX != UINT64_MAX
#error "the amd64 frame decoder requires 64-bit addresses"
#endif

enum { AMD64_DWARF_SP = 7, AMD64_DWARF_FP = 6, FRAME_RECORD_SIZE = 16 };

static bool range_contains(ScoopPlatformStackBounds bounds, uintptr_t address,
                           size_t size) {
    uintptr_t low = (uintptr_t)bounds.low;
    uintptr_t high = (uintptr_t)bounds.high;
    return low < high && address >= low && address <= high &&
           size <= high - address;
}

/* LLVM's stack size includes saved RBP, but excludes the return address. */
static bool valid_stack_size(uint64_t size) {
    return size >= 8 && size <= INT64_MAX && size % 16 == 8;
}

static bool root_frame_offset(const ScoopStackMapRecord *record,
                              const ScoopStackMapLocation *location,
                              uint64_t *offset) {
    uint64_t fp_offset = record->stack_size - 8;
    uint64_t base = location->dwarf_register == AMD64_DWARF_SP ? 0 : fp_offset;
    int64_t displacement = location->offset;
    uint64_t resolved;
    if (displacement < 0) {
        uint64_t amount = (uint64_t)-displacement;
        if (amount > base) {
            return false;
        }
        resolved = base - amount;
    } else {
        if ((uint64_t)displacement > fp_offset - base) {
            return false;
        }
        resolved = base + (uint64_t)displacement;
    }
    if (resolved > fp_offset || fp_offset - resolved < 8 || resolved % 8 != 0) {
        return false;
    }
    *offset = resolved;
    return true;
}

static bool root_location(const ScoopStackMapLocation *location) {
    return location->kind == SCOOP_STACKMAP_INDIRECT && location->size == 8 &&
           (location->dwarf_register == AMD64_DWARF_SP ||
            location->dwarf_register == AMD64_DWARF_FP);
}

static bool locations_equal(const ScoopStackMapLocation *left,
                            const ScoopStackMapLocation *right) {
    return left->kind == right->kind && left->size == right->size &&
           left->dwarf_register == right->dwarf_register &&
           left->offset == right->offset && left->constant == right->constant;
}

static bool amd64_validate_record(const ScoopStackMapRecord *record,
                                  ScoopPlatformError *error) {
    if (record == NULL || error == NULL ||
        !valid_stack_size(record->stack_size) ||
        (record->root_count != 0 && record->roots == NULL)) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        }
        return false;
    }
    for (uint16_t index = 0; index < record->root_count; ++index) {
        const ScoopStackMapRootPair *pair = &record->roots[index];
        if (!root_location(&pair->base) ||
            !locations_equal(&pair->base, &pair->derived)) {
            *error =
                (ScoopPlatformError){SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION,
                                     record->safepoint_id, index};
            return false;
        }
        uint64_t offset;
        if (!root_frame_offset(record, &pair->base, &offset)) {
            *error = (ScoopPlatformError){SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME,
                                          record->safepoint_id, index};
            return false;
        }
    }
    return true;
}

static bool amd64_frame_from_anchor(const ScoopManagedAnchor *anchor,
                                    const ScoopStackMapRecord *record,
                                    ScoopPlatformStackBounds bounds,
                                    ScoopManagedFrame *frame,
                                    ScoopPlatformError *error) {
    if (anchor == NULL || record == NULL || frame == NULL || error == NULL ||
        !valid_stack_size(record->stack_size) ||
        anchor->return_pc != record->return_pc ||
        anchor->stack_pointer % 16 != 0 ||
        !range_contains(bounds, anchor->stack_pointer,
                        record->stack_size + 8) ||
        anchor->frame_pointer !=
            anchor->stack_pointer + record->stack_size - 8) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_ANCHOR;
            error->safepoint_id = record == NULL ? 0 : record->safepoint_id;
        }
        return false;
    }
    *frame = (ScoopManagedFrame){anchor->return_pc, anchor->stack_pointer,
                                 anchor->frame_pointer, record};
    return true;
}

static bool amd64_resolve_root(const ScoopManagedFrame *frame,
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
    uint64_t offset;
    if (!root_location(location)) {
        *error = (ScoopPlatformError){SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION,
                                      frame->record->safepoint_id, root_index};
        return false;
    }
    if (!root_frame_offset(frame->record, location, &offset)) {
        *error = (ScoopPlatformError){SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME,
                                      frame->record->safepoint_id, root_index};
        return false;
    }
    *slot = (void **)(frame->stack_pointer + offset);
    return true;
}

static bool amd64_next_frame(const ScoopManagedFrame *frame, uintptr_t boundary,
                             ScoopPlatformStackBounds bounds,
                             uintptr_t *return_pc, uintptr_t *stack_pointer,
                             uintptr_t *frame_pointer, bool *has_next,
                             ScoopPlatformError *error) {
    if (frame == NULL || return_pc == NULL || stack_pointer == NULL ||
        frame_pointer == NULL || has_next == NULL || error == NULL ||
        boundary <= frame->frame_pointer || boundary > (uintptr_t)bounds.high ||
        !range_contains(bounds, frame->frame_pointer, FRAME_RECORD_SIZE)) {
        if (error != NULL) {
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        }
        return false;
    }
    uintptr_t saved[2];
    memcpy(saved, (const void *)frame->frame_pointer, sizeof saved);
    if (saved[0] >= boundary) {
        *has_next = false;
        return true;
    }
    uintptr_t next_sp = frame->frame_pointer + FRAME_RECORD_SIZE;
    if (saved[0] <= frame->frame_pointer || next_sp >= boundary ||
        saved[1] == 0 || saved[0] % 16 != 0 ||
        !range_contains(bounds, saved[0], FRAME_RECORD_SIZE)) {
        error->code = SCOOP_PLATFORM_INVALID_FRAME;
        error->safepoint_id =
            frame->record == NULL ? 0 : frame->record->safepoint_id;
        return false;
    }
    *return_pc = saved[1];
    *stack_pointer = next_sp;
    *frame_pointer = saved[0];
    *has_next = true;
    return true;
}

const ScoopManagedFrameOps scoop_amd64_managed_frame_ops = {
    .validate_record = amd64_validate_record,
    .frame_from_anchor = amd64_frame_from_anchor,
    .resolve_root = amd64_resolve_root,
    .next_frame = amd64_next_frame,
};
