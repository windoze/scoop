#include "../stackmap.h"

static bool same_location(const ScoopStackMapLocation *left, const ScoopStackMapLocation *right) {
    bool left_constant =
        left->kind == SCOOP_STACKMAP_CONSTANT || left->kind == SCOOP_STACKMAP_CONSTANT_INDEX;
    bool right_constant =
        right->kind == SCOOP_STACKMAP_CONSTANT || right->kind == SCOOP_STACKMAP_CONSTANT_INDEX;
    if (left_constant && right_constant) {
        return left->size == right->size && left->constant == right->constant;
    }
    return left->kind == right->kind && left->size == right->size &&
           left->dwarf_register == right->dwarf_register && left->offset == right->offset;
}

bool scoop_stackmap_same_payload(const ScoopStackMapRecord *left,
                                 const ScoopStackMapRecord *right) {
    if (left->instruction_offset != right->instruction_offset ||
        left->stack_size != right->stack_size || left->root_count != right->root_count ||
        left->live_out_count != right->live_out_count)
        return false;
    for (size_t index = 0; index < 3; index++) {
        if (!same_location(&left->header[index], &right->header[index]))
            return false;
    }
    for (size_t index = 0; index < left->root_count; index++) {
        if (!same_location(&left->roots[index].base, &right->roots[index].base) ||
            !same_location(&left->roots[index].derived, &right->roots[index].derived))
            return false;
    }
    for (size_t index = 0; index < left->live_out_count; index++) {
        if (left->live_outs[index].dwarf_register != right->live_outs[index].dwarf_register ||
            left->live_outs[index].size != right->live_outs[index].size)
            return false;
    }
    return true;
}
