#ifndef SCOOP_PROFILING_PATH_H
#define SCOOP_PROFILING_PATH_H
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
bool scoop_profile_open_files(FILE **counters, FILE **metrics, uint64_t now_ns);
#endif
