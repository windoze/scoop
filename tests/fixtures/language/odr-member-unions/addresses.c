#include "scoop_rt.h"

extern const ScoopTypeDescriptor union_left_td __asm__("UNION_LEFT_TD");
extern const ScoopTypeDescriptor union_right_td __asm__("UNION_RIGHT_TD");
extern const ScoopTypeDescriptor union_other_td __asm__("UNION_OTHER_TD");
extern const ScoopTypeDescriptor union_interface __asm__("UNION_INTERFACE_TD");

static const void *const *interface_slots(const ScoopTypeDescriptor *td) {
    for (uint64_t index = 0; index < td->itable_count; index++) {
        if (td->itables[index].interface == &union_interface) {
            return td->itables[index].slots;
        }
    }
    return NULL;
}

/* Inspect the actual static descriptors after program shutdown. */
static bool union_metadata_check(void) {
    const ScoopTypeDescriptor *volatile left = &union_left_td;
    const ScoopTypeDescriptor *volatile right = &union_right_td;
    const ScoopTypeDescriptor *volatile other = &union_other_td;
    const void *const *left_slots = interface_slots(left);
    const void *const *right_slots = interface_slots(right);
    const void *const *other_slots = interface_slots(other);
    return left == right && left != other && left_slots != NULL &&
           right_slots != NULL && other_slots != NULL &&
           left_slots[UNION_READ_SLOT] != NULL &&
           left_slots[UNION_READ_SLOT] == right_slots[UNION_READ_SLOT];
}

#define SCOOP_FIXTURE_METADATA_CHECK() union_metadata_check()
