#include "scoop_rt.h"

extern void *union_left_object(void) __asm__("UNION_LEFT_OBJECT");
extern void *union_right_object(void) __asm__("UNION_RIGHT_OBJECT");
extern void *union_other_object(void) __asm__("UNION_OTHER_OBJECT");
extern const ScoopTypeDescriptor union_interface __asm__("UNION_INTERFACE_TD");

/* Allocating calls stay in main's published managed boundary frame. */
static inline __attribute__((always_inline)) bool union_metadata_check(void) {
    /* Only static descriptors survive the next allocating call. */
    const ScoopTypeDescriptor *left = ((ScoopObjectHeader *)union_left_object())->td;
    const ScoopTypeDescriptor *right = ((ScoopObjectHeader *)union_right_object())->td;
    const ScoopTypeDescriptor *other = ((ScoopObjectHeader *)union_other_object())->td;
    const void *const *left_slots = scoop_rt_itable_lookup(left, &union_interface);
    const void *const *right_slots = scoop_rt_itable_lookup(right, &union_interface);
    const void *const *other_slots = scoop_rt_itable_lookup(other, &union_interface);
    return left == right && left != other && left_slots != NULL &&
        right_slots != NULL && other_slots != NULL &&
        left_slots[UNION_READ_SLOT] != NULL &&
        left_slots[UNION_READ_SLOT] == right_slots[UNION_READ_SLOT];
}

#define SCOOP_FIXTURE_METADATA_CHECK() union_metadata_check()
