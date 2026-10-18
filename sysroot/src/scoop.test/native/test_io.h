#ifndef SCOOP_TEST_IO_H
#define SCOOP_TEST_IO_H

#include <stdint.h>
#include <stdio.h>

FILE *scoop_test_open_output(const char *name);
int scoop_test_capture_begin(uint64_t sequence);
int64_t scoop_test_capture_end(void);

#endif
