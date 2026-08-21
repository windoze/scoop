/* Runtime smoke test: provides scoop_main (normally emitted by the
 * compiler), exercises alloc + print/println, and checks that a global
 * constant laid out the way codegen emits string literals is readable by
 * the runtime. Expected stdout: "helloworld\n".
 *
 * Build & run:
 *   cc -std=c11 -I runtime/include runtime/src/rt.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test
 *   /tmp/scoop_rt_test
 */
#include <string.h>

#include "scoop_rt.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen. */
static const ScoopTypeDescriptor string_td = {1, 16, 8, NULL};

/* Same layout codegen uses for StringConst globals:
 * { const ScoopTypeDescriptor *td; uint64_t len; char data[N]; }. */
typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t len;
    char data[5];
} HelloConst;

static const HelloConst hello = {&string_td, 5, {'h', 'e', 'l', 'l', 'o'}};

void scoop_main(void) {
    ScoopString *s = scoop_rt_alloc(&string_td, sizeof(ScoopString) + 5);
    s->len = 5;
    memcpy(s->data, "world", 5);

    scoop_rt_print((const ScoopString *)&hello);
    scoop_rt_println(s);
}
