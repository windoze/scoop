#include <stdio.h>
#include <stdlib.h>

#include "internal.h"

static const ScoopImageRegistry *active_registry;

static _Noreturn void active_fatal(const char *message) {
    fprintf(stderr, "scoop runtime registry: %s\n", message);
    abort();
}

void scoop_image_publish(const ScoopImageRegistry *registry) {
    if (registry == NULL || active_registry != NULL) {
        active_fatal("invalid registry publication");
    }
    for (size_t index = 0; index < registry->context_cell_count; index++) {
        const ScoopContextCell *cell = &registry->context_cells[index];
        *cell->use->slot_cell = cell->encoded_slot;
    }
    active_registry = registry;
}

void scoop_image_unpublish(void) {
    if (active_registry == NULL) {
        active_fatal("registry shutdown without publication");
    }
    active_registry = NULL;
}

const ScoopImageRegistry *scoop_image_current(void) {
    if (active_registry == NULL) {
        active_fatal("operation outside the runtime lifetime");
    }
    return active_registry;
}

void scoop_image_require_type(const ScoopTypeDescriptor *td) {
    if (scoop_image_type(scoop_image_current(), td) == NULL) {
        active_fatal("operation received an unregistered TypeDescriptor");
    }
}

void scoop_image_require_unit(const ScoopInitializationUnitDescriptorV1 *unit) {
    if (scoop_record_by_address(scoop_image_current(), SCOOP_RECORD_UNIT, unit) ==
        NULL) {
        active_fatal("operation received an unregistered initialization unit");
    }
}
