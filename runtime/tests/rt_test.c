/* Runtime smoke test: provides scoop_main (normally emitted by the
 * compiler), exercises alloc, string concat/eq, and the String / Int /
 * Boolean print builtins, and checks that globals laid out the way
 * codegen emits string literals are readable by the runtime.
 *
 * Expected stdout:
 *   helloworld
 *   true
 *   false
 *   false
 *   42-7
 *   truefalse
 *
 * Build & run:
 *   cc -std=c11 -Wall -Wextra -I runtime/include runtime/src/rt.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test
 *   /tmp/scoop_rt_test
 */
#include "scoop_rt.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen.
 * Non-static: rt.c references it from scoop_rt_string_concat. */
const ScoopTypeDescriptor scoop_td_String = {1, 16, 8, NULL};

/* Same layout codegen uses for StringConst globals:
 * { const ScoopTypeDescriptor *td; uint64_t len; char data[N]; }. */
typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t len;
    char data[5];
} FiveCharConst;

static const FiveCharConst hello = {&scoop_td_String, 5, {'h', 'e', 'l', 'l', 'o'}};
static const FiveCharConst world = {&scoop_td_String, 5, {'w', 'o', 'r', 'l', 'd'}};

void scoop_main(void) {
    const ScoopString *a = (const ScoopString *)&hello;
    const ScoopString *b = (const ScoopString *)&world;

    /* concat: "hello" + "world" -> "helloworld" */
    const ScoopString *concat = scoop_rt_string_concat(a, b);
    scoop_rt_println(concat);

    /* structural equality: same content, different content, length
     * mismatch */
    const ScoopString *copy = scoop_rt_string_concat(a, b);
    scoop_rt_println_boolean(scoop_rt_string_eq(concat, copy));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, b));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, concat));

    /* int / boolean output (print variants run into the println line) */
    scoop_rt_print_int(42);
    scoop_rt_println_int(-7);
    scoop_rt_print_boolean(true);
    scoop_rt_println_boolean(false);
}
