#include "threads/internal.h"

void run_thread_tests(void) {
    run_thread_registration_tests();
    run_thread_stw_tests();
    run_thread_allocation_tests();
    run_thread_transition_tests();
    run_thread_borrowed_wait_tests();
}
