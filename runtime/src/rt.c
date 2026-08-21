/* Scoop runtime, M1 minimal set (DESIGN section 3). C11. */
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    /* Always-leak: memory is never freed (DESIGN 5.1). */
    ScoopObjectHeader *obj = malloc(size);
    if (obj == NULL) {
        fprintf(stderr, "scoop_rt_alloc: out of memory\n");
        abort();
    }
    obj->td = td;
    return obj;
}

void scoop_rt_print(const ScoopString *s) {
    fwrite(s->data, 1, s->len, stdout);
}

void scoop_rt_println(const ScoopString *s) {
    scoop_rt_print(s);
    fputc('\n', stdout);
}

int main(void) {
    scoop_main();
    return 0;
}
