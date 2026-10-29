#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "../platform.h"

enum { AMD64_DWARF_SP = 7, AMD64_DWARF_FP = 6, SEH_FRAME_RBP = 5 };

static bool range_contains(ScoopPlatformStackBounds bounds, uintptr_t address, uint64_t size) {
    uintptr_t low = (uintptr_t)bounds.low, high = (uintptr_t)bounds.high;
    return low < high && address >= low && address <= high && size <= high - address;
}

static bool valid_stack_size(uint64_t size) {
    return size >= 8 && size <= INT64_MAX && size % 8 == 0;
}

static bool root_location(const ScoopStackMapLocation *location) {
    return location->kind == SCOOP_STACKMAP_INDIRECT && location->size == 8 &&
           (location->dwarf_register == AMD64_DWARF_SP ||
            location->dwarf_register == AMD64_DWARF_FP);
}

static bool locations_equal(const ScoopStackMapLocation *left, const ScoopStackMapLocation *right) {
    return left->kind == right->kind && left->size == right->size &&
           left->dwarf_register == right->dwarf_register && left->offset == right->offset &&
           left->constant == right->constant;
}

static bool windows_validate_record(const ScoopStackMapRecord *record, ScoopPlatformError *error) {
    if (record == NULL || error == NULL || !valid_stack_size(record->stack_size) ||
        (record->root_count != 0 && record->roots == NULL)) {
        if (error != NULL)
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        return false;
    }
    for (uint16_t index = 0; index < record->root_count; ++index) {
        const ScoopStackMapRootPair *pair = &record->roots[index];
        if (!root_location(&pair->base) || !locations_equal(&pair->base, &pair->derived)) {
            *error = (ScoopPlatformError){SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION,
                                          record->safepoint_id, index};
            return false;
        }
        if (pair->base.dwarf_register == AMD64_DWARF_SP &&
            (pair->base.offset < 0 || (uint64_t)pair->base.offset > record->stack_size - 8 ||
             pair->base.offset % 8 != 0)) {
            *error = (ScoopPlatformError){SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME, record->safepoint_id,
                                          index};
            return false;
        }
    }
    return true;
}

static bool windows_frame_from_anchor(const ScoopManagedAnchor *anchor,
                                      const ScoopStackMapRecord *record,
                                      ScoopPlatformStackBounds bounds, ScoopManagedFrame *frame,
                                      ScoopPlatformError *error) {
    if (anchor == NULL || record == NULL || frame == NULL || error == NULL ||
        !valid_stack_size(record->stack_size) || anchor->return_pc != record->return_pc ||
        anchor->stack_pointer % 16 != 0 ||
        !range_contains(bounds, anchor->stack_pointer, record->stack_size + 8) ||
        anchor->frame_pointer < anchor->stack_pointer ||
        anchor->frame_pointer > anchor->stack_pointer + record->stack_size) {
        if (error != NULL)
            *error = (ScoopPlatformError){SCOOP_PLATFORM_INVALID_ANCHOR,
                                          record == NULL ? 0 : record->safepoint_id, 0};
        return false;
    }
    *frame = (ScoopManagedFrame){anchor->return_pc, anchor->stack_pointer, anchor->frame_pointer,
                                 record};
    return true;
}

static bool windows_resolve_root(const ScoopManagedFrame *frame, uint16_t index, void ***slot,
                                 ScoopPlatformError *error) {
    if (frame == NULL || frame->record == NULL || slot == NULL || error == NULL ||
        index >= frame->record->root_count) {
        if (error != NULL)
            error->code = SCOOP_PLATFORM_INVALID_FRAME;
        return false;
    }
    const ScoopStackMapLocation *location = &frame->record->roots[index].base;
    if (!root_location(location)) {
        *error = (ScoopPlatformError){SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION,
                                      frame->record->safepoint_id, index};
        return false;
    }
    uint64_t base = location->dwarf_register == AMD64_DWARF_SP
                        ? 0
                        : frame->frame_pointer - frame->stack_pointer;
    int64_t offset = location->offset;
    if ((offset < 0 && (uint64_t)-offset > base) ||
        (offset >= 0 && (uint64_t)offset > frame->record->stack_size - base))
        goto outside;
    uint64_t resolved = offset < 0 ? base - (uint64_t)-offset : base + (uint64_t)offset;
    if (resolved > frame->record->stack_size - 8 || resolved % 8 != 0)
        goto outside;
    *slot = (void **)(frame->stack_pointer + resolved);
    return true;
outside:
    *error =
        (ScoopPlatformError){SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME, frame->record->safepoint_id, index};
    return false;
}

static bool windows_next_frame(const ScoopManagedFrame *frame, uintptr_t boundary,
                               ScoopPlatformStackBounds bounds, uintptr_t *pc, uintptr_t *sp,
                               uintptr_t *fp, bool *has_next, ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    if (frame == NULL || pc == NULL || sp == NULL || fp == NULL || has_next == NULL ||
        boundary <= frame->frame_pointer || boundary > (uintptr_t)bounds.high)
        goto invalid;
    DWORD64 image_base = 0;
    PRUNTIME_FUNCTION function = RtlLookupFunctionEntry(frame->return_pc, &image_base, NULL);
    if (function == NULL)
        goto invalid;
    /* The selected LLVM profile uses RBP as its only unwind frame register.
     * Chained entries must have the same frame register as their primary. */
    const uint8_t *unwind = (const void *)(uintptr_t)(image_base + function->UnwindData);
    unsigned frame_register = unwind[3] & 15;
    if (frame_register != 0 && frame_register != SEH_FRAME_RBP)
        goto invalid;
    CONTEXT context = {0};
    context.ContextFlags = CONTEXT_CONTROL | CONTEXT_INTEGER;
    context.Rip = frame->return_pc;
    context.Rsp = frame->stack_pointer;
    context.Rbp = frame->frame_pointer;
    void *handler_data = NULL;
    DWORD64 establisher;
    RtlVirtualUnwind(UNW_FLAG_NHANDLER, image_base, context.Rip, function, &context, &handler_data,
                     &establisher, NULL);
    if (context.Rip == 0 || context.Rsp <= frame->stack_pointer || context.Rsp % 16 != 0 ||
        !range_contains(bounds, context.Rsp, 0) || !range_contains(bounds, context.Rbp, 0))
        goto invalid;
    if (context.Rbp >= boundary) {
        *has_next = false;
        return true;
    }
    if (context.Rsp >= boundary || context.Rbp < context.Rsp || context.Rbp <= frame->frame_pointer)
        goto invalid;
    *pc = context.Rip;
    *sp = context.Rsp;
    *fp = context.Rbp;
    *has_next = true;
    return true;
invalid:
    *error = (ScoopPlatformError){
        SCOOP_PLATFORM_INVALID_FRAME,
        frame == NULL || frame->record == NULL ? 0 : frame->record->safepoint_id, 0};
    return false;
}

const ScoopManagedFrameOps scoop_windows_amd64_managed_frame_ops = {
    .validate_record = windows_validate_record,
    .frame_from_anchor = windows_frame_from_anchor,
    .resolve_root = windows_resolve_root,
    .next_frame = windows_next_frame,
};
