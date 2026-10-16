#include "../gc.h"

/* Collection is currently driven by managed allocation. External pressure is
 * deliberately ignored; this operation must remain safe inside release hooks. */
void scoop_gc_adjust_pressure(int64_t delta) { (void)delta; }
