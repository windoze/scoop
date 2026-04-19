// Scoop C runtime: minimal Task<T> core (executor-free stage).
//
// 说明：
// - 当前阶段只保留 lazy task object 的最小运行时承载；
// - `async` sugar 通过 `scoop_task_create` 创建 created task；
// - `spawn` 的过渡语义通过 `scoop_task_from_result` 直接包装已完成 task；
// - `join` 会在 created task 上同步直驱一次 body，然后读取结果；
// - executor / waiter queue / onComplete 等旧 surface 已从当前主线移除。

#include <stdint.h>
#include <stdlib.h>

#include "platform/platform.h"
#include "scoop_gc.h"

void scoop_runtime_init(void);
uint32_t scoop_runtime_is_initialized(void);
void scoop_thread_register(void);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);

typedef uint64_t (*ScoopTaskBodyFn)(void *closure_obj, void **out_gc_ref);

typedef enum ScoopTaskStateU32 {
  SCOOP_TASK_STATE_CREATED = 0u,
  SCOOP_TASK_STATE_RUNNING = 1u,
  SCOOP_TASK_STATE_COMPLETED = 2u,
} ScoopTaskStateU32;

typedef struct ScoopTask {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex lock;
  uint32_t state;
  uint32_t lock_initialized;
  ScoopTaskBodyFn body_fn;
  void *body_closure_obj;  // GC-traced
  uint64_t result_word;
  void *result_gc_ref;  // GC-traced
} ScoopTask;

static void scoop_pin_nullable_or_die(void *obj) {
  if (obj != 0 && !scoop_pin(obj)) {
    exit(3);
  }
}

static void scoop_unpin_nullable(void *obj) {
  if (obj != 0) {
    (void)scoop_unpin(obj);
  }
}

static uint64_t scoop_task_trace(void *object, ScoopGcTraceVisitor visitor, void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopTask *task = (ScoopTask *)object;
  uint64_t refs = 0;

  if (task->body_closure_obj != 0) {
    void **slot = (void **)&task->body_closure_obj;
    visitor(slot, ctx);
    refs++;
  }
  if (task->result_gc_ref != 0) {
    void **slot = (void **)&task->result_gc_ref;
    visitor(slot, ctx);
    refs++;
  }

  return refs;
}

static void scoop_task_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopTask *task = (ScoopTask *)object;
  if (task->lock_initialized) {
    scoop_platform_sync_mutex_destroy(&task->lock);
    task->lock_initialized = 0;
  }
}

static const ScoopTypeDescriptor SCOOP_TASK_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopTask),
    .align_bytes = (uint64_t)_Alignof(ScoopTask),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_task_trace,
    .release_fn = scoop_task_release,
};

static ScoopTask *scoop_task_alloc(ScoopTaskBodyFn body_fn, void *body_closure_obj) {
  scoop_pin_nullable_or_die(body_closure_obj);

  ScoopTask *task = (ScoopTask *)scoop_alloc_typed(
      &SCOOP_TASK_TYPE_DESC, (uint64_t)sizeof(ScoopTask));
  if (task == 0) {
    scoop_unpin_nullable(body_closure_obj);
    return 0;
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_CREATED;
  task->lock_initialized = 0;
  task->body_fn = body_fn;
  task->body_closure_obj = body_closure_obj;
  task->result_word = 0;
  task->result_gc_ref = 0;

  if (!scoop_platform_sync_mutex_init(&task->lock)) {
    scoop_unpin_nullable(body_closure_obj);
    return 0;
  }

  task->lock_initialized = 1;
  scoop_unpin_nullable(body_closure_obj);
  return task;
}

static uint32_t scoop_task_complete_internal(ScoopTask *task,
                                             uint64_t result_word,
                                             void *result_gc_ref) {
  if (task == 0 || !task->lock_initialized) {
    scoop_unpin_nullable(result_gc_ref);
    return 0;
  }

  scoop_pin_nullable_or_die(result_gc_ref);

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    scoop_unpin_nullable(result_gc_ref);
    exit(3);
  }
  if (task->state != (uint32_t)SCOOP_TASK_STATE_CREATED &&
      task->state != (uint32_t)SCOOP_TASK_STATE_RUNNING) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    scoop_unpin_nullable(result_gc_ref);
    exit(3);
  }

  task->result_word = result_word;
  task->result_gc_ref = result_gc_ref;
  task->state = (uint32_t)SCOOP_TASK_STATE_COMPLETED;
  task->body_fn = 0;
  task->body_closure_obj = 0;
  scoop_platform_sync_mutex_unlock(&task->lock);
  return 1;
}

static uint32_t scoop_task_read_completed_result(ScoopTask *task,
                                                 uint64_t *out_word,
                                                 void **out_gc_ref) {
  if (task == 0 || !task->lock_initialized) {
    return 0;
  }

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state != (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    exit(3);
  }

  if (out_word != 0) {
    *out_word = task->result_word;
  }
  if (out_gc_ref != 0) {
    *out_gc_ref = task->result_gc_ref;
  }
  scoop_platform_sync_mutex_unlock(&task->lock);
  return 1;
}

static uint32_t scoop_task_run_body(ScoopTask *task) {
  if (task == 0 || !task->lock_initialized) {
    return 0;
  }

  ScoopTaskBodyFn body_fn = 0;
  void *body_closure_obj = 0;

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 1;
  }
  if (task->state != (uint32_t)SCOOP_TASK_STATE_CREATED || task->body_fn == 0) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 0;
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_RUNNING;
  body_fn = task->body_fn;
  body_closure_obj = task->body_closure_obj;
  scoop_pin_nullable_or_die(body_closure_obj);
  scoop_platform_sync_mutex_unlock(&task->lock);

  uint64_t result_word = 0;
  void *result_gc_ref = 0;
  if (body_fn != 0) {
    result_word = body_fn(body_closure_obj, &result_gc_ref);
  }

  scoop_unpin_nullable(body_closure_obj);
  return scoop_task_complete_internal(task, result_word, result_gc_ref);
}

void *scoop_task_create(ScoopTaskBodyFn body_fn, void *body_closure_obj) {
  scoop_thread_register();
  return (void *)scoop_task_alloc(body_fn, body_closure_obj);
}

void *scoop_task_from_result(uint64_t result_word, void *result_gc_ref) {
  if (!scoop_runtime_is_initialized()) {
    scoop_runtime_init();
  }

  scoop_thread_register();

  ScoopTask *task = scoop_task_alloc(0, 0);
  if (task == 0) {
    scoop_unpin_nullable(result_gc_ref);
    return 0;
  }

  if (!scoop_task_complete_internal(task, result_word, result_gc_ref)) {
    return 0;
  }

  return (void *)task;
}

uint64_t scoop_task_join(void *task_obj, void **out_gc_ref) {
  if (out_gc_ref != 0) {
    *out_gc_ref = 0;
  }
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopTask *task = (ScoopTask *)task_obj;
  if (!task->lock_initialized) {
    return 0;
  }

  // 当前 stage 仍未公开 `poll()`；`join` 先作为最小驱动器，在 created task 上同步直驱一次 body。
  (void)scoop_task_run_body(task);

  uint64_t word = 0;
  void *gc_ref = 0;
  (void)scoop_task_read_completed_result(task, &word, &gc_ref);
  if (out_gc_ref != 0) {
    *out_gc_ref = gc_ref;
  }
  return word;
}
