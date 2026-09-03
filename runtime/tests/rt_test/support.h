#ifndef SCOOP_RT_SMOKE_TEST_SUPPORT_H
#define SCOOP_RT_SMOKE_TEST_SUPPORT_H

#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../../src/generated_entries.h"

typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t gc_word;
    uint64_t len;
    char data[5];
} FiveCharConst;

typedef struct ScoopNode {
    ScoopObjectHeader header;
    int64_t value;
    struct ScoopNode *next;
} ScoopNode;

typedef struct {
    ScoopObjectHeader header;
    int64_t words[6];
} ScoopBig64;

typedef struct {
    ScoopObjectHeader header;
    uint64_t tag;
    uint64_t pure_payload;
    const ScoopString *a_ref;
} ScoopBoxedEnum;

typedef struct {
    uint64_t tag;
    uint64_t pure_payload;
    const ScoopString *a_ref;
    const ScoopString *tail;
} ScoopNestedElement;

extern const ScoopTypeDescriptor scoop_td_String;
extern const FiveCharConst hello;
extern const FiveCharConst world;
extern const ScoopTypeDescriptor describable_td;
extern const void *const point_describable_slots[];
extern const ScoopTypeDescriptor shape_td;
extern const ScoopTypeDescriptor point_td;
extern const ScoopTypeDescriptor node_td;
extern const ScoopTypeDescriptor big64_td;
extern ScoopNode *image_global_rooted;
extern void *image_immortal_rooted;
extern ScoopNode *global_rooted;

ScoopNode *new_node(int64_t value, ScoopNode *next);
void make_garbage_nodes(int count);
void make_garbage_big64(void);
void make_garbage_big64_many(void);
ScoopArray *make_ref_array(void);
ScoopBoxedEnum *make_boxed_enum_a(void);
ScoopBoxedEnum *make_boxed_enum_b(void);
ScoopArray *make_nested_array(void);
void clobber_stack(void);

void run_thread_tests(void);
void run_callback_tests(void);
void run_core_runtime_tests(void);
void run_gc_tests(void);

#endif /* SCOOP_RT_SMOKE_TEST_SUPPORT_H */
