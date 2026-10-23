#include "scoop_rt.h"
#include <assert.h>
#include <stdio.h>

static void method(void) {}
static const void *const slots[] = {(const void *)method};
static const ScoopTypeDescriptor parent = {.relation_kind = 3};
static const ScoopTypeDescriptor child = {.relation_kind = 3};
static const ScoopTypeDescriptor empty = {.relation_kind = 3};
static const ScoopTypeDescriptor generic_int = {.relation_kind = 3};
static const ScoopTypeDescriptor generic_string = {.relation_kind = 3};
static const ScoopItableEntryV1 entries[] = {
    {&parent, slots}, {&child, slots}, {&empty, NULL}, {&generic_int, slots}};
static const ScoopTypeDescriptor implemented = {
    .itables = entries, .itable_count = sizeof entries / sizeof entries[0]};
static const ScoopTypeDescriptor absent = {0};

int main(void) {
    const ScoopObjectHeader object = {.td = &implemented};
    assert(scoop_rt_itable_find(&implemented, &parent) == &entries[0]);
    assert(scoop_rt_itable_find(&implemented, &child) == &entries[1]);
    const ScoopItableEntryV1 *marker = scoop_rt_itable_find(&implemented, &empty);
    assert(marker == &entries[2] && marker->slots == NULL);
    assert(scoop_rt_itable_lookup(&implemented, &empty) == NULL);
    assert(scoop_rt_itable_find(&implemented, &generic_int) == &entries[3]);
    assert(scoop_rt_itable_find(&implemented, &generic_string) == NULL);
    assert(scoop_rt_itable_find(&absent, &parent) == NULL);
    for (size_t i = 0; i < sizeof entries / sizeof entries[0]; i++) {
        assert(scoop_rt_is_instance(&object, entries[i].interface));
        assert(scoop_rt_itable_lookup(&implemented, entries[i].interface) == entries[i].slots);
    }
    assert(!scoop_rt_is_instance(&object, &generic_string));
    puts("interface queries preserve empty and exact entries");
    return 0;
}
