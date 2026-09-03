#ifndef SCOOP_RT_THREAD_TESTS_INTERNAL_H
#define SCOOP_RT_THREAD_TESTS_INTERNAL_H

#include "../support.h"

void run_thread_registration_tests(void);
void run_thread_stw_tests(void);
void run_thread_allocation_tests(void);
void run_thread_transition_tests(void);
void run_thread_borrowed_wait_tests(void);

#endif /* SCOOP_RT_THREAD_TESTS_INTERNAL_H */
