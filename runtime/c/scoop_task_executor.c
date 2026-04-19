// Scoop C runtime: Task<T> / Executor primitives (object-model stage).
//
// 说明：
// - `Task<T>` / `Executor` 现在都是真正的 GC-managed 对象，而不是 word-sized handle；
// - runtime 私有队列 / waiter 节点继续使用 `malloc`，但所有跨 GC 边界悬挂的对象引用
//   都通过 `pin/unpin` 显式托管，避免 moving GC 下的悬挂指针；
// - 调度语义仍保持最小 cooperative executor：单队列、无取消、无 work-stealing。

#include <stddef.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>

#include "platform/platform.h"
#include "scoop_gc.h"

void scoop_thread_register(void);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);
void scoop_continuation_resume(void *continuation);
uint32_t scoop_task_complete(void *task_obj, uint64_t result_word, void *result_gc_ref);

typedef struct ScoopEffectHandlerFrame ScoopEffectHandlerFrame;
typedef void (*ScoopContinuationStepFn)(void *state, uint64_t resume_word, void *resume_gc_ref);

typedef struct ScoopContinuationLayout {
  ScoopGcObjectHeader hdr;
  _Atomic uint32_t resumed;
  uint32_t resume_state_tag;
  ScoopEffectHandlerFrame *captured_handler_stack_top;
  void *state;
  ScoopContinuationStepFn step_fn;
  uint64_t resume_word;
  void *resume_gc_ref;
  void *captured_callee_suspend_state;
} ScoopContinuationLayout;

typedef enum ScoopTaskStateU32 {
  SCOOP_TASK_STATE_CREATED = 0u,
  SCOOP_TASK_STATE_SCHEDULED = 1u,
  SCOOP_TASK_STATE_RUNNING = 2u,
  SCOOP_TASK_STATE_COMPLETED = 3u,
} ScoopTaskStateU32;

typedef struct ScoopExecutor ScoopExecutor;
typedef struct ScoopTask ScoopTask;

typedef uint64_t (*ScoopTaskBodyFn)(void *closure_obj, void **out_gc_ref);

typedef struct ScoopExecutorJob {
  struct ScoopExecutorJob *next;
  uint32_t kind;
  uint32_t _reserved_u32;
  union {
    struct {
      void *continuation;   // pinned while queued
      uint64_t resume_word;
      void *resume_gc_ref;  // pinned while queued when non-null
    } resume;
    struct {
      ScoopTask *task;  // pinned while queued
    } run_task;
  } as;
} ScoopExecutorJob;

typedef struct ScoopTaskWaiter {
  struct ScoopTaskWaiter *next;
  ScoopExecutor *executor;  // pinned while waiting
  void *continuation;       // pinned while waiting
} ScoopTaskWaiter;

struct ScoopExecutor {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex lock;
  ScoopExecutorJob *head;
  ScoopExecutorJob *tail;
  uint64_t pending_count;
  uint32_t destroyed;
  uint32_t lock_initialized;
};

struct ScoopTask {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex lock;
  uint32_t state;
  uint32_t lock_initialized;
  ScoopTaskBodyFn body_fn;
  void *body_closure_obj;  // GC-traced
  uint64_t result_word;
  void *result_gc_ref;  // GC-traced
  ScoopTaskWaiter *waiters_head;
  ScoopTaskWaiter *waiters_tail;
};

enum {
  SCOOP_EXECUTOR_JOB_RESUME_TRANSPORT = 1u,
  SCOOP_EXECUTOR_JOB_RUN_TASK = 2u,
};

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

static void scoop_executor_release(void *object);
static void scoop_task_release(void *object);

static const ScoopTypeDescriptor SCOOP_EXECUTOR_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopExecutor),
    .align_bytes = (uint64_t)_Alignof(ScoopExecutor),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_executor_release,
};

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

static int scoop_executor_can_queue(const ScoopExecutor *executor) {
  return executor != 0 && executor->lock_initialized != 0 && executor->destroyed == 0;
}

static void scoop_executor_job_release(ScoopExecutorJob *job) {
  if (job == 0) {
    return;
  }

  switch (job->kind) {
    case SCOOP_EXECUTOR_JOB_RESUME_TRANSPORT:
      scoop_unpin_nullable(job->as.resume.continuation);
      scoop_unpin_nullable(job->as.resume.resume_gc_ref);
      break;
    case SCOOP_EXECUTOR_JOB_RUN_TASK:
      scoop_unpin_nullable((void *)job->as.run_task.task);
      break;
    default:
      break;
  }

  free(job);
}

static void scoop_task_waiter_release(ScoopTaskWaiter *waiter) {
  if (waiter == 0) {
    return;
  }

  scoop_unpin_nullable((void *)waiter->executor);
  scoop_unpin_nullable(waiter->continuation);
  free(waiter);
}

static uint32_t scoop_executor_enqueue_job_owned(ScoopExecutor *executor, ScoopExecutorJob *job) {
  if (!scoop_executor_can_queue(executor) || job == 0) {
    scoop_executor_job_release(job);
    return 0;
  }

  scoop_platform_sync_mutex_lock(&executor->lock);
  if (executor->destroyed) {
    scoop_platform_sync_mutex_unlock(&executor->lock);
    scoop_executor_job_release(job);
    return 0;
  }

  job->next = 0;
  if (executor->tail == 0) {
    executor->head = job;
    executor->tail = job;
  } else {
    executor->tail->next = job;
    executor->tail = job;
  }
  executor->pending_count += 1;
  scoop_platform_sync_mutex_unlock(&executor->lock);
  return 1;
}

static ScoopExecutorJob *scoop_executor_try_pop_job(ScoopExecutor *executor) {
  if (!scoop_executor_can_queue(executor)) {
    return 0;
  }

  scoop_platform_sync_mutex_lock(&executor->lock);
  ScoopExecutorJob *job = executor->head;
  if (job == 0) {
    scoop_platform_sync_mutex_unlock(&executor->lock);
    return 0;
  }

  executor->head = job->next;
  if (executor->head == 0) {
    executor->tail = 0;
  }
  if (executor->pending_count > 0) {
    executor->pending_count -= 1;
  }
  scoop_platform_sync_mutex_unlock(&executor->lock);

  job->next = 0;
  return job;
}

static void scoop_executor_cleanup_queue(ScoopExecutor *executor) {
  if (executor == 0) {
    return;
  }

  ScoopExecutorJob *jobs = 0;
  if (executor->lock_initialized) {
    scoop_platform_sync_mutex_lock(&executor->lock);
    jobs = executor->head;
    executor->head = 0;
    executor->tail = 0;
    executor->pending_count = 0;
    scoop_platform_sync_mutex_unlock(&executor->lock);
  } else {
    jobs = executor->head;
    executor->head = 0;
    executor->tail = 0;
    executor->pending_count = 0;
  }

  while (jobs != 0) {
    ScoopExecutorJob *next = jobs->next;
    jobs->next = 0;
    scoop_executor_job_release(jobs);
    jobs = next;
  }
}

static void scoop_task_cleanup_waiters(ScoopTask *task) {
  if (task == 0) {
    return;
  }

  ScoopTaskWaiter *waiters = 0;
  if (task->lock_initialized) {
    scoop_platform_sync_mutex_lock(&task->lock);
    waiters = task->waiters_head;
    task->waiters_head = 0;
    task->waiters_tail = 0;
    scoop_platform_sync_mutex_unlock(&task->lock);
  } else {
    waiters = task->waiters_head;
    task->waiters_head = 0;
    task->waiters_tail = 0;
  }

  while (waiters != 0) {
    ScoopTaskWaiter *next = waiters->next;
    waiters->next = 0;
    scoop_task_waiter_release(waiters);
    waiters = next;
  }
}

static void scoop_executor_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopExecutor *executor = (ScoopExecutor *)object;
  if (executor->destroyed) {
    return;
  }

  scoop_executor_cleanup_queue(executor);
  if (executor->lock_initialized) {
    scoop_platform_sync_mutex_destroy(&executor->lock);
    executor->lock_initialized = 0;
  }
  executor->destroyed = 1;
}

static void scoop_task_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopTask *task = (ScoopTask *)object;
  scoop_task_cleanup_waiters(task);
  if (task->lock_initialized) {
    scoop_platform_sync_mutex_destroy(&task->lock);
    task->lock_initialized = 0;
  }
}

static uint32_t scoop_executor_enqueue_resume_transport_pinned(ScoopExecutor *executor,
                                                               void *continuation,
                                                               uint64_t resume_word,
                                                               void *resume_gc_ref) {
  if (!scoop_executor_can_queue(executor) || continuation == 0) {
    scoop_unpin_nullable(continuation);
    return 0;
  }

  scoop_pin_nullable_or_die(resume_gc_ref);

  ScoopExecutorJob *job = (ScoopExecutorJob *)malloc(sizeof(ScoopExecutorJob));
  if (job == 0) {
    exit(3);
  }

  job->next = 0;
  job->kind = SCOOP_EXECUTOR_JOB_RESUME_TRANSPORT;
  job->_reserved_u32 = 0;
  job->as.resume.continuation = continuation;
  job->as.resume.resume_word = resume_word;
  job->as.resume.resume_gc_ref = resume_gc_ref;
  return scoop_executor_enqueue_job_owned(executor, job);
}

static uint32_t scoop_executor_enqueue_run_task(ScoopExecutor *executor, ScoopTask *task) {
  if (!scoop_executor_can_queue(executor) || task == 0) {
    return 0;
  }

  scoop_pin_nullable_or_die((void *)task);

  ScoopExecutorJob *job = (ScoopExecutorJob *)malloc(sizeof(ScoopExecutorJob));
  if (job == 0) {
    exit(3);
  }

  job->next = 0;
  job->kind = SCOOP_EXECUTOR_JOB_RUN_TASK;
  job->_reserved_u32 = 0;
  job->as.run_task.task = task;
  return scoop_executor_enqueue_job_owned(executor, job);
}

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
  task->waiters_head = 0;
  task->waiters_tail = 0;

  if (!scoop_platform_sync_mutex_init(&task->lock)) {
    scoop_unpin_nullable(body_closure_obj);
    return 0;
  }

  task->lock_initialized = 1;
  scoop_unpin_nullable(body_closure_obj);
  return task;
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

static void scoop_task_run_body_on_executor(ScoopTask *task) {
  if (task == 0 || !task->lock_initialized) {
    return;
  }

  ScoopTaskBodyFn body_fn = 0;
  void *body_closure_obj = 0;

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return;
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
  (void)scoop_task_complete((void *)task, result_word, result_gc_ref);
}

void *scoop_executor_create(void) {
  scoop_thread_register();

  ScoopExecutor *executor = (ScoopExecutor *)scoop_alloc_typed(
      &SCOOP_EXECUTOR_TYPE_DESC, (uint64_t)sizeof(ScoopExecutor));
  if (executor == 0) {
    return 0;
  }

  executor->head = 0;
  executor->tail = 0;
  executor->pending_count = 0;
  executor->destroyed = 0;
  executor->lock_initialized = 0;

  if (!scoop_platform_sync_mutex_init(&executor->lock)) {
    return 0;
  }

  executor->lock_initialized = 1;
  return (void *)executor;
}

void scoop_executor_destroy(void *executor_obj) {
  if (executor_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopExecutor *executor = (ScoopExecutor *)executor_obj;
  if (executor->destroyed) {
    return;
  }

  scoop_executor_cleanup_queue(executor);
  if (executor->lock_initialized) {
    scoop_platform_sync_mutex_destroy(&executor->lock);
    executor->lock_initialized = 0;
  }
  executor->destroyed = 1;
}

uint64_t scoop_executor_debug_pending_count(void *executor_obj) {
  if (executor_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopExecutor *executor = (ScoopExecutor *)executor_obj;
  if (!scoop_executor_can_queue(executor)) {
    return 0;
  }

  scoop_platform_sync_mutex_lock(&executor->lock);
  uint64_t n = executor->pending_count;
  scoop_platform_sync_mutex_unlock(&executor->lock);
  return n;
}

uint64_t scoop_executor_run_next(void *executor_obj) {
  if (executor_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopExecutor *executor = (ScoopExecutor *)executor_obj;
  ScoopExecutorJob *job = scoop_executor_try_pop_job(executor);
  if (job == 0) {
    return 0;
  }

  if (job->kind == SCOOP_EXECUTOR_JOB_RESUME_TRANSPORT) {
    ScoopContinuationLayout *continuation =
        (ScoopContinuationLayout *)job->as.resume.continuation;
    if (continuation != 0) {
      continuation->resume_word = job->as.resume.resume_word;
      continuation->resume_gc_ref = job->as.resume.resume_gc_ref;
      scoop_continuation_resume((void *)continuation);
    }
    scoop_executor_job_release(job);
    return 1;
  }

  if (job->kind == SCOOP_EXECUTOR_JOB_RUN_TASK) {
    ScoopTask *task = job->as.run_task.task;
    scoop_task_run_body_on_executor(task);
    scoop_executor_job_release(job);
    return 1;
  }

  scoop_executor_job_release(job);
  return 1;
}

uint64_t scoop_executor_run_until_idle(void *executor_obj, uint64_t max_steps) {
  if (executor_obj == 0) {
    return 0;
  }

  uint64_t limit = max_steps == 0 ? UINT64_MAX : max_steps;
  uint64_t ran = 0;

  while (ran < limit) {
    if (!scoop_executor_run_next(executor_obj)) {
      break;
    }
    ran++;
  }

  return ran;
}

void *scoop_task_create(ScoopTaskBodyFn body_fn, void *body_closure_obj) {
  scoop_thread_register();
  return (void *)scoop_task_alloc(body_fn, body_closure_obj);
}

void *scoop_task_create_manual(void) {
  scoop_thread_register();
  return (void *)scoop_task_alloc(0, 0);
}

uint32_t scoop_task_state(void *task_obj) {
  if (task_obj == 0) {
    return (uint32_t)SCOOP_TASK_STATE_CREATED;
  }

  scoop_thread_register();

  ScoopTask *task = (ScoopTask *)task_obj;
  if (!task->lock_initialized) {
    return (uint32_t)SCOOP_TASK_STATE_CREATED;
  }

  scoop_platform_sync_mutex_lock(&task->lock);
  uint32_t state = task->state;
  scoop_platform_sync_mutex_unlock(&task->lock);
  return state;
}

uint64_t scoop_task_result_word(void *task_obj) {
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  uint64_t word = 0;
  (void)scoop_task_read_completed_result((ScoopTask *)task_obj, &word, 0);
  return word;
}

void *scoop_task_result_gc_ref(void *task_obj) {
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  void *gc_ref = 0;
  (void)scoop_task_read_completed_result((ScoopTask *)task_obj, 0, &gc_ref);
  return gc_ref;
}

uint32_t scoop_task_try_start(void *task_obj, void *executor_obj) {
  if (task_obj == 0 || executor_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopTask *task = (ScoopTask *)task_obj;
  ScoopExecutor *executor = (ScoopExecutor *)executor_obj;
  if (!task->lock_initialized || !scoop_executor_can_queue(executor)) {
    return 0;
  }

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state != (uint32_t)SCOOP_TASK_STATE_CREATED || task->body_fn == 0) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 0;
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_SCHEDULED;
  scoop_platform_sync_mutex_unlock(&task->lock);

  if (!scoop_executor_enqueue_run_task(executor, task)) {
    scoop_platform_sync_mutex_lock(&task->lock);
    if (task->state == (uint32_t)SCOOP_TASK_STATE_SCHEDULED) {
      task->state = (uint32_t)SCOOP_TASK_STATE_CREATED;
    }
    scoop_platform_sync_mutex_unlock(&task->lock);
    return 0;
  }

  return 1;
}

uint32_t scoop_task_complete(void *task_obj, uint64_t result_word, void *result_gc_ref) {
  if (task_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopTask *task = (ScoopTask *)task_obj;
  if (!task->lock_initialized) {
    return 0;
  }

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    exit(3);
  }

  task->state = (uint32_t)SCOOP_TASK_STATE_COMPLETED;
  task->body_fn = 0;
  task->body_closure_obj = 0;
  task->result_word = result_word;
  task->result_gc_ref = result_gc_ref;

  ScoopTaskWaiter *waiters = task->waiters_head;
  task->waiters_head = 0;
  task->waiters_tail = 0;
  scoop_platform_sync_mutex_unlock(&task->lock);

  while (waiters != 0) {
    ScoopTaskWaiter *next = waiters->next;
    waiters->next = 0;
    (void)scoop_executor_enqueue_resume_transport_pinned(
        waiters->executor, waiters->continuation, result_word, result_gc_ref);
    scoop_unpin_nullable((void *)waiters->executor);
    free(waiters);
    waiters = next;
  }

  return 1;
}

uint32_t scoop_task_on_complete(void *task_obj, void *executor_obj, void *continuation) {
  if (task_obj == 0 || executor_obj == 0 || continuation == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopTask *task = (ScoopTask *)task_obj;
  ScoopExecutor *executor = (ScoopExecutor *)executor_obj;
  if (!task->lock_initialized || !scoop_executor_can_queue(executor)) {
    return 0;
  }

  scoop_pin_nullable_or_die(continuation);

  scoop_platform_sync_mutex_lock(&task->lock);
  if (task->state == (uint32_t)SCOOP_TASK_STATE_COMPLETED) {
    uint64_t result_word = task->result_word;
    void *result_gc_ref = task->result_gc_ref;
    scoop_platform_sync_mutex_unlock(&task->lock);
    return scoop_executor_enqueue_resume_transport_pinned(
        executor, continuation, result_word, result_gc_ref);
  }

  scoop_pin_nullable_or_die((void *)executor);

  ScoopTaskWaiter *node = (ScoopTaskWaiter *)malloc(sizeof(ScoopTaskWaiter));
  if (node == 0) {
    scoop_platform_sync_mutex_unlock(&task->lock);
    scoop_unpin_nullable((void *)executor);
    scoop_unpin_nullable(continuation);
    exit(3);
  }

  node->next = 0;
  node->executor = executor;
  node->continuation = continuation;

  if (task->waiters_tail == 0) {
    task->waiters_head = node;
    task->waiters_tail = node;
  } else {
    task->waiters_tail->next = node;
    task->waiters_tail = node;
  }

  scoop_platform_sync_mutex_unlock(&task->lock);
  return 1;
}
