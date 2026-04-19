// Scoop C runtime: Task<T> manual polling core.
//
// 说明：
// - `Task<T>` 当前只承载 lazy/manual polling 语义；
// - `poll()` / `step()` 都驱动任务执行，直到“下一次挂起或完成”为止；
// - task 的 suspended-state carrier 对外不可见，runtime 内部继续借助 raw continuation；
// - `join` 仍是过渡期 helper：它循环 `poll()`，直到任务完成。

#include <stddef.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>

#include "platform/platform.h"
#include "scoop_gc.h"

void scoop_runtime_init(void);
uint32_t scoop_runtime_is_initialized(void);
void scoop_thread_register(void);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);
void scoop_continuation_resume(void *continuation);

typedef void *(*ScoopTaskBodyFn)(void *closure_obj);

typedef enum ScoopTaskStateU32 {
  SCOOP_TASK_STATE_CREATED = 0u,
  SCOOP_TASK_STATE_RUNNING = 1u,
  SCOOP_TASK_STATE_PENDING = 2u,
  SCOOP_TASK_STATE_COMPLETED = 3u,
} ScoopTaskStateU32;

typedef enum ScoopTaskStepKindU32 {
  SCOOP_TASK_STEP_KIND_READY = 1u,
  SCOOP_TASK_STEP_KIND_PENDING = 2u,
} ScoopTaskStepKindU32;

// `runtime/c/scoop_runtime.c` 中 continuation 的前缀布局。
//
// task runtime 只需要：
// - 向 continuation 写入 resume payload；
// - 在 `scoop_continuation_resume(...)` 返回后，读取 continuation 的 heap state frame，
//   再从标准化的 frame 前缀中取回 handle result（这里即 `__TaskStepResult`）。
typedef struct ScoopContinuation {
  ScoopGcObjectHeader hdr;
  _Atomic uint32_t resumed;
  uint32_t resume_state_tag;
  void *captured_handler_stack_top;
  void *state;
  void *step_fn;
  uint64_t resume_word;
  void *resume_gc_ref;
  void *captured_callee_suspend_state;
} ScoopContinuation;

// state-machine frame 的统一前缀（见 `state_machine_emitter.rs`）：
// - `state_tag`：当前 state / sentinel completion tag
// - `resume_word` / `resume_gc_ref`：handle result transport
typedef struct ScoopEffectFrameResultPrefix {
  ScoopGcObjectHeader hdr;
  uint32_t state_tag;
  uint32_t _padding;
  uint64_t resume_word;
  void *resume_gc_ref;
} ScoopEffectFrameResultPrefix;

enum {
  SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED = 0xFFFFFFFEu,
  SCOOP_EFFECT_FRAME_STATE_TAG_FUNCTION_RETURNED = 0xFFFFFFFFu,
};

typedef struct ScoopTaskStepResult {
  ScoopGcObjectHeader hdr;
  uint32_t kind;
  uint32_t _padding;
  uint64_t value_word;
  void *value_gc_ref;   // GC-traced
  void *awaited_task;   // GC-traced
  void *continuation;   // GC-traced
} ScoopTaskStepResult;

typedef struct ScoopTask {
  ScoopGcObjectHeader hdr;
  ScoopPlatformMutex lock;
  uint32_t state;
  uint32_t lock_initialized;
  ScoopTaskBodyFn body_fn;
  void *body_closure_obj;  // GC-traced
  void *awaited_task;      // GC-traced
  void *continuation;      // GC-traced
  uint64_t result_word;
  void *result_gc_ref;     // GC-traced
} ScoopTask;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopContinuation, resume_word) ==
                   offsetof(ScoopContinuation, step_fn) + sizeof(void *),
               "ScoopContinuation.resume_word must follow step_fn");
_Static_assert(offsetof(ScoopContinuation, resume_gc_ref) ==
                   offsetof(ScoopContinuation, resume_word) + sizeof(uint64_t),
               "ScoopContinuation.resume_gc_ref must follow resume_word");
_Static_assert(offsetof(ScoopEffectFrameResultPrefix, state_tag) ==
                   sizeof(ScoopGcObjectHeader),
               "ScoopEffectFrameResultPrefix.state_tag offset must follow header");
_Static_assert(offsetof(ScoopEffectFrameResultPrefix, resume_word) ==
                   sizeof(ScoopGcObjectHeader) + 8u,
               "ScoopEffectFrameResultPrefix.resume_word offset must match state-machine frame");
#endif

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

static uint64_t scoop_task_step_result_trace(void *object,
                                             ScoopGcTraceVisitor visitor,
                                             void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopTaskStepResult *step = (ScoopTaskStepResult *)object;
  uint64_t refs = 0;

  if (step->value_gc_ref != 0) {
    void **slot = (void **)&step->value_gc_ref;
    visitor(slot, ctx);
    refs++;
  }
  if (step->awaited_task != 0) {
    void **slot = (void **)&step->awaited_task;
    visitor(slot, ctx);
    refs++;
  }
  if (step->continuation != 0) {
    void **slot = (void **)&step->continuation;
    visitor(slot, ctx);
    refs++;
  }

  return refs;
}

static const ScoopTypeDescriptor SCOOP_TASK_STEP_RESULT_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopTaskStepResult),
    .align_bytes = (uint64_t)_Alignof(ScoopTaskStepResult),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_task_step_result_trace,
    .release_fn = 0,
};

static ScoopTaskStepResult *scoop_task_step_result_alloc(void) {
  return (ScoopTaskStepResult *)scoop_alloc_typed(
      &SCOOP_TASK_STEP_RESULT_TYPE_DESC,
      (uint64_t)sizeof(ScoopTaskStepResult));
}

void *scoop_task_step_ready(uint64_t word, void *gc_ref) {
  scoop_thread_register();
  scoop_pin_nullable_or_die(gc_ref);

  ScoopTaskStepResult *step = scoop_task_step_result_alloc();
  if (step == 0) {
    scoop_unpin_nullable(gc_ref);
    return 0;
  }

  step->kind = (uint32_t)SCOOP_TASK_STEP_KIND_READY;
  step->_padding = 0;
  step->value_word = word;
  step->value_gc_ref = gc_ref;
  step->awaited_task = 0;
  step->continuation = 0;

  scoop_unpin_nullable(gc_ref);
  return (void *)step;
}

void *scoop_task_step_pending(void *awaited_task, void *continuation) {
  scoop_thread_register();
  scoop_pin_nullable_or_die(awaited_task);
  scoop_pin_nullable_or_die(continuation);

  ScoopTaskStepResult *step = scoop_task_step_result_alloc();
  if (step == 0) {
    scoop_unpin_nullable(continuation);
    scoop_unpin_nullable(awaited_task);
    return 0;
  }

  step->kind = (uint32_t)SCOOP_TASK_STEP_KIND_PENDING;
  step->_padding = 0;
  step->value_word = 0;
  step->value_gc_ref = 0;
  step->awaited_task = awaited_task;
  step->continuation = continuation;

  scoop_unpin_nullable(continuation);
  scoop_unpin_nullable(awaited_task);
  return (void *)step;
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
  if (task->awaited_task != 0) {
    void **slot = (void **)&task->awaited_task;
    visitor(slot, ctx);
    refs++;
  }
  if (task->continuation != 0) {
    void **slot = (void **)&task->continuation;
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
  task->awaited_task = 0;
  task->continuation = 0;
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

static void scoop_task_set_completed(ScoopTask *task,
                                     uint64_t result_word,
                                     void *result_gc_ref) {
  if (task == 0 || !task->lock_initialized) {
    exit(3);
  }

  scoop_pin_nullable_or_die(result_gc_ref);

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state != (uint32_t)SCOOP_TASK_STATE_CREATED &&
      task->state != (uint32_t)SCOOP_TASK_STATE_RUNNING) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    scoop_unpin_nullable(result_gc_ref);
    exit(3);
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_COMPLETED;
  task->body_fn = 0;
  task->body_closure_obj = 0;
  task->awaited_task = 0;
  task->continuation = 0;
  task->result_word = result_word;
  task->result_gc_ref = result_gc_ref;
  scoop_platform_sync_mutex_unlock(&task->lock);

  scoop_unpin_nullable(result_gc_ref);
}

static void scoop_task_set_pending(ScoopTask *task,
                                   void *awaited_task,
                                   void *continuation) {
  if (task == 0 || !task->lock_initialized) {
    exit(3);
  }
  if (awaited_task == 0 || continuation == 0) {
    exit(3);
  }

  scoop_pin_nullable_or_die(awaited_task);
  scoop_pin_nullable_or_die(continuation);

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state != (uint32_t)SCOOP_TASK_STATE_RUNNING) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    scoop_unpin_nullable(continuation);
    scoop_unpin_nullable(awaited_task);
    exit(3);
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_PENDING;
  task->body_fn = 0;
  task->body_closure_obj = 0;
  task->awaited_task = awaited_task;
  task->continuation = continuation;
  task->result_word = 0;
  task->result_gc_ref = 0;
  scoop_platform_sync_mutex_unlock(&task->lock);

  scoop_unpin_nullable(continuation);
  scoop_unpin_nullable(awaited_task);
}

static void scoop_task_apply_step_result(ScoopTask *task, ScoopTaskStepResult *step) {
  if (step == 0) {
    exit(3);
  }

  switch (step->kind) {
    case SCOOP_TASK_STEP_KIND_READY:
      scoop_task_set_completed(task, step->value_word, step->value_gc_ref);
      return;
    case SCOOP_TASK_STEP_KIND_PENDING:
      scoop_task_set_pending(task, step->awaited_task, step->continuation);
      return;
    default:
      exit(3);
  }
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
    return 0;
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

static ScoopTaskStepResult *scoop_task_resume_continuation_to_step(
    void *continuation,
    uint64_t resume_word,
    void *resume_gc_ref) {
  if (continuation == 0) {
    exit(3);
  }

  ScoopContinuation *k = (ScoopContinuation *)continuation;
  k->resume_word = resume_word;
  k->resume_gc_ref = resume_gc_ref;
  scoop_continuation_resume(continuation);

  if (k->state == 0) {
    exit(3);
  }

  ScoopEffectFrameResultPrefix *frame =
      (ScoopEffectFrameResultPrefix *)k->state;
  if (frame->state_tag != SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED &&
      frame->state_tag != SCOOP_EFFECT_FRAME_STATE_TAG_FUNCTION_RETURNED) {
    exit(3);
  }
  if (frame->resume_gc_ref == 0) {
    exit(3);
  }

  return (ScoopTaskStepResult *)frame->resume_gc_ref;
}

static uint32_t scoop_task_begin_running_created(ScoopTask *task,
                                                 ScoopTaskBodyFn *out_body_fn,
                                                 void **out_body_closure_obj) {
  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 0;
  }
  if (task->state == (uint32_t)SCOOP_TASK_STATE_RUNNING) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 2;
  }
  if (task->state != (uint32_t)SCOOP_TASK_STATE_CREATED || task->body_fn == 0) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    exit(3);
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_RUNNING;
  *out_body_fn = task->body_fn;
  *out_body_closure_obj = task->body_closure_obj;
  scoop_pin_nullable_or_die(*out_body_closure_obj);
  scoop_platform_sync_mutex_unlock(&task->lock);
  return 1;
}

static uint32_t scoop_task_begin_running_pending(ScoopTask *task,
                                                 void **out_awaited_task,
                                                 void **out_continuation) {
  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 0;
  }
  if (task->state == (uint32_t)SCOOP_TASK_STATE_RUNNING) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 2;
  }
  if (task->state != (uint32_t)SCOOP_TASK_STATE_PENDING ||
      task->awaited_task == 0 || task->continuation == 0) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    exit(3);
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_RUNNING;
  *out_awaited_task = task->awaited_task;
  *out_continuation = task->continuation;
  scoop_pin_nullable_or_die(*out_awaited_task);
  scoop_pin_nullable_or_die(*out_continuation);
  scoop_platform_sync_mutex_unlock(&task->lock);
  return 1;
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
    return 0;
  }

  scoop_task_set_completed(task, result_word, result_gc_ref);
  return (void *)task;
}

uint32_t scoop_task_poll(void *task_obj, uint64_t *out_word, void **out_gc_ref) {
  if (out_word != 0) {
    *out_word = 0;
  }
  if (out_gc_ref != 0) {
    *out_gc_ref = 0;
  }
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();
  scoop_pin_nullable_or_die(task_obj);

  ScoopTask *task = (ScoopTask *)task_obj;
  if (!task->lock_initialized) {
    scoop_unpin_nullable(task_obj);
    return 0;
  }

  for (;;) {
    if (scoop_task_read_completed_result(task, out_word, out_gc_ref)) {
      scoop_unpin_nullable(task_obj);
      return 1u;
    }

    uint32_t state = 0;
    scoop_platform_sync_mutex_lock(&task->lock);
    state = task->state;
    scoop_platform_sync_mutex_unlock(&task->lock);

    switch (state) {
      case SCOOP_TASK_STATE_CREATED: {
        ScoopTaskBodyFn body_fn = 0;
        void *body_closure_obj = 0;
        uint32_t begin =
            scoop_task_begin_running_created(task, &body_fn, &body_closure_obj);
        if (begin == 0) {
          continue;
        }
        if (begin == 2) {
          scoop_unpin_nullable(task_obj);
          return 0;
        }

        ScoopTaskStepResult *step = 0;
        if (body_fn != 0) {
          step = (ScoopTaskStepResult *)body_fn(body_closure_obj);
        }
        scoop_unpin_nullable(body_closure_obj);
        scoop_task_apply_step_result(task, step);
        continue;
      }

      case SCOOP_TASK_STATE_PENDING: {
        void *awaited_task = 0;
        void *continuation = 0;
        uint32_t begin =
            scoop_task_begin_running_pending(task, &awaited_task, &continuation);
        if (begin == 0) {
          continue;
        }
        if (begin == 2) {
          scoop_unpin_nullable(task_obj);
          return 0;
        }

        uint64_t awaited_word = 0;
        void *awaited_gc_ref = 0;
        uint32_t awaited_ready =
            scoop_task_poll(awaited_task, &awaited_word, &awaited_gc_ref);
        if (!awaited_ready) {
          scoop_platform_sync_mutex_lock(&task->lock);
          if (task->state == (uint32_t)SCOOP_TASK_STATE_RUNNING) {
            task->state = (uint32_t)SCOOP_TASK_STATE_PENDING;
          }
          scoop_platform_sync_mutex_unlock(&task->lock);
          scoop_unpin_nullable(continuation);
          scoop_unpin_nullable(awaited_task);
          scoop_unpin_nullable(task_obj);
          return 0;
        }

        ScoopTaskStepResult *step = scoop_task_resume_continuation_to_step(
            continuation, awaited_word, awaited_gc_ref);
        scoop_unpin_nullable(continuation);
        scoop_unpin_nullable(awaited_task);
        scoop_task_apply_step_result(task, step);
        continue;
      }

      case SCOOP_TASK_STATE_RUNNING:
        scoop_unpin_nullable(task_obj);
        return 0;

      case SCOOP_TASK_STATE_COMPLETED:
        continue;

      default:
        scoop_unpin_nullable(task_obj);
        exit(3);
    }
  }
}

uint64_t scoop_task_join(void *task_obj, void **out_gc_ref) {
  if (out_gc_ref != 0) {
    *out_gc_ref = 0;
  }
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  for (;;) {
    uint64_t word = 0;
    void *gc_ref = 0;
    if (scoop_task_poll(task_obj, &word, &gc_ref)) {
      if (out_gc_ref != 0) {
        *out_gc_ref = gc_ref;
      }
      return word;
    }
    scoop_platform_thread_yield();
  }
}
