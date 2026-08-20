#include <stdatomic.h>

/* Keep the same out-of-line native operation in every benchmark variant. */
void scoop_test_native_leaf(void) { atomic_signal_fence(memory_order_seq_cst); }
