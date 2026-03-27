// Scoop C runtime: Task<T> / executor primitives (early stage).
//
// 说明：
// - 该文件实现 spec §5.7 所需的最小 executor 运行期原语（TODO T0917）：
//   - `ScoopExecutor`：一个最小队列，支持入队 continuation + resume(u64 payload)
//   - `ScoopTaskU64`：任务状态机 + completion 回调（完成后恢复等待者 continuation）
//   - 可选显式 start：把 task body 入队到 executor 运行并完成
// - 该实现是 cooperative、单队列、无取消：更复杂调度/并行/取消留给后续任务扩展。

#include <stdint.h>
#include <stddef.h>
#include <inttypes.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc.h"

// `scoop_runtime_init` / `scoop_continuation_resume_u64` 由 `scoop_runtime.c` 提供。
void scoop_runtime_init(void);
void scoop_continuation_resume_u64(void *continuation, uint64_t resume_value);

typedef enum ScoopTaskStateU32 {
  SCOOP_TASK_STATE_CREATED = 0u,
  SCOOP_TASK_STATE_SCHEDULED = 1u,
  SCOOP_TASK_STATE_RUNNING = 2u,
  SCOOP_TASK_STATE_COMPLETED = 3u,
} ScoopTaskStateU32;

typedef struct ScoopExecutor ScoopExecutor;
typedef struct ScoopTaskU64 ScoopTaskU64;

typedef uint64_t (*ScoopTaskBodyU64Fn)(void *ctx);

typedef struct ScoopExecutorJob {
  struct ScoopExecutorJob *next;
  uint32_t kind;
  uint32_t _reserved_u32;
  union {
    struct {
      void *continuation;
      uint64_t resume_value;
    } resume_u64;
    struct {
      ScoopTaskU64 *task;
    } run_task;
  } as;
} ScoopExecutorJob;

typedef struct ScoopExecutor {
  pthread_mutex_t lock;
  ScoopExecutorJob *head;
  ScoopExecutorJob *tail;
  uint64_t pending_count;
} ScoopExecutor;

typedef struct ScoopTaskWaiter {
  struct ScoopTaskWaiter *next;
  ScoopExecutor *executor;
  void *continuation;
} ScoopTaskWaiter;

typedef struct ScoopTaskU64 {
  pthread_mutex_t lock;
  ScoopTaskStateU32 state;
  uint32_t _reserved_u32;
  ScoopTaskBodyU64Fn body_fn;
  void *body_ctx;
  uint64_t result_u64;
  ScoopTaskWaiter *waiters_head;
  ScoopTaskWaiter *waiters_tail;
} ScoopTaskU64;

enum {
  SCOOP_EXECUTOR_JOB_RESUME_U64 = 1u,
  SCOOP_EXECUTOR_JOB_RUN_TASK_U64 = 2u,
};

static void scoop_executor_enqueue_job(ScoopExecutor *executor, ScoopExecutorJob *job) {
  if (executor == 0 || job == 0) {
    return;
  }

  (void)pthread_mutex_lock(&executor->lock);
  job->next = 0;
  if (executor->tail == 0) {
    executor->head = job;
    executor->tail = job;
  } else {
    executor->tail->next = job;
    executor->tail = job;
  }
  executor->pending_count += 1;
  (void)pthread_mutex_unlock(&executor->lock);
}

static ScoopExecutorJob *scoop_executor_try_pop_job(ScoopExecutor *executor) {
  if (executor == 0) {
    return 0;
  }

  (void)pthread_mutex_lock(&executor->lock);
  ScoopExecutorJob *job = executor->head;
  if (job == 0) {
    (void)pthread_mutex_unlock(&executor->lock);
    return 0;
  }

  executor->head = job->next;
  if (executor->head == 0) {
    executor->tail = 0;
  }
  if (executor->pending_count > 0) {
    executor->pending_count -= 1;
  }
  (void)pthread_mutex_unlock(&executor->lock);

  job->next = 0;
  return job;
}

static void scoop_executor_enqueue_resume_u64_pinned(ScoopExecutor *executor,
                                                     void *continuation,
                                                     uint64_t resume_value) {
  if (executor == 0 || continuation == 0) {
    return;
  }

  ScoopExecutorJob *job = (ScoopExecutorJob *)malloc(sizeof(ScoopExecutorJob));
  if (job == 0) {
    // OOM：executor/job 是调度关键路径；early stage 直接 fail-fast。
    exit(3);
  }

  job->next = 0;
  job->kind = SCOOP_EXECUTOR_JOB_RESUME_U64;
  job->_reserved_u32 = 0;
  job->as.resume_u64.continuation = continuation;
  job->as.resume_u64.resume_value = resume_value;
  scoop_executor_enqueue_job(executor, job);
}

uint64_t scoop_executor_create(void) {
  scoop_runtime_init();

  ScoopExecutor *executor = (ScoopExecutor *)malloc(sizeof(ScoopExecutor));
  if (executor == 0) {
    return 0;
  }

  (void)memset(executor, 0, sizeof(ScoopExecutor));
  (void)pthread_mutex_init(&executor->lock, 0);
  executor->head = 0;
  executor->tail = 0;
  executor->pending_count = 0;
  return (uint64_t)(uintptr_t)executor;
}

void scoop_executor_destroy(uint64_t executor_handle) {
  if (executor_handle == 0) {
    return;
  }

  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;

  // 说明：early stage 为避免泄漏 pinned roots，这里会把队列 drain 掉并做对应 unpin；
  // 但不会执行 job（即不会 resume continuation / run task）。
  ScoopExecutorJob *job = 0;
  while ((job = scoop_executor_try_pop_job(executor)) != 0) {
    if (job->kind == SCOOP_EXECUTOR_JOB_RESUME_U64) {
      (void)scoop_unpin(job->as.resume_u64.continuation);
    }
    free(job);
  }

  (void)pthread_mutex_destroy(&executor->lock);
  free(executor);
}

uint64_t scoop_executor_debug_pending_count(uint64_t executor_handle) {
  if (executor_handle == 0) {
    return 0;
  }

  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;
  (void)pthread_mutex_lock(&executor->lock);
  uint64_t n = executor->pending_count;
  (void)pthread_mutex_unlock(&executor->lock);
  return n;
}

void scoop_executor_enqueue_resume_u64(uint64_t executor_handle,
                                      void *continuation,
                                      uint64_t resume_value) {
  if (executor_handle == 0 || continuation == 0) {
    return;
  }

  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;

  // 说明：pin 失败意味着 continuation 不是由 `scoop_alloc` 分配/登记的 GC 对象；
  // 这通常表示编译器/runtime 之间的 ABI 假设被破坏：early stage 直接 fail-fast。
  if (!scoop_pin(continuation)) {
    exit(3);
  }

  scoop_executor_enqueue_resume_u64_pinned(executor, continuation, resume_value);
}

static void scoop_task_u64_run_body_on_executor(ScoopTaskU64 *task, ScoopExecutor *executor);

uint64_t scoop_executor_run_next(uint64_t executor_handle) {
  if (executor_handle == 0) {
    return 0;
  }

  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;
  ScoopExecutorJob *job = scoop_executor_try_pop_job(executor);
  if (job == 0) {
    return 0;
  }

  if (job->kind == SCOOP_EXECUTOR_JOB_RESUME_U64) {
    void *continuation = job->as.resume_u64.continuation;
    uint64_t value = job->as.resume_u64.resume_value;
    scoop_continuation_resume_u64(continuation, value);
    (void)scoop_unpin(continuation);
    free(job);
    return 1;
  }

  if (job->kind == SCOOP_EXECUTOR_JOB_RUN_TASK_U64) {
    ScoopTaskU64 *task = job->as.run_task.task;
    free(job);
    scoop_task_u64_run_body_on_executor(task, executor);
    return 1;
  }

  // unknown job kind：早期阶段直接忽略（更安全：避免崩溃）。
  free(job);
  return 1;
}

uint64_t scoop_executor_run_until_idle(uint64_t executor_handle, uint64_t max_steps) {
  if (executor_handle == 0) {
    return 0;
  }

  // max_steps==0 视为 “不设上限”，避免调用方必须手动传 UINT64_MAX。
  uint64_t limit = max_steps == 0 ? UINT64_MAX : max_steps;
  uint64_t ran = 0;

  while (ran < limit) {
    if (!scoop_executor_run_next(executor_handle)) {
      break;
    }
    ran++;
  }

  return ran;
}

uint64_t scoop_task_u64_create(ScoopTaskBodyU64Fn body_fn, void *body_ctx) {
  scoop_runtime_init();

  ScoopTaskU64 *task = (ScoopTaskU64 *)malloc(sizeof(ScoopTaskU64));
  if (task == 0) {
    return 0;
  }

  (void)memset(task, 0, sizeof(ScoopTaskU64));
  (void)pthread_mutex_init(&task->lock, 0);
  task->state = SCOOP_TASK_STATE_CREATED;
  task->_reserved_u32 = 0;
  task->body_fn = body_fn;
  task->body_ctx = body_ctx;
  task->result_u64 = 0;
  task->waiters_head = 0;
  task->waiters_tail = 0;

  return (uint64_t)(uintptr_t)task;
}

void scoop_task_u64_destroy(uint64_t task_handle) {
  if (task_handle == 0) {
    return;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;

  (void)pthread_mutex_lock(&task->lock);
  ScoopTaskWaiter *waiters = task->waiters_head;
  task->waiters_head = 0;
  task->waiters_tail = 0;
  (void)pthread_mutex_unlock(&task->lock);

  // destroy 时若仍有 waiters，说明使用方未完成/未 drain；这里做 best-effort 清理：
  // - unpin 以避免 pinned roots 泄漏
  // - free waiter nodes
  while (waiters != 0) {
    ScoopTaskWaiter *next = waiters->next;
    if (waiters->continuation != 0) {
      (void)scoop_unpin(waiters->continuation);
    }
    free(waiters);
    waiters = next;
  }

  (void)pthread_mutex_destroy(&task->lock);
  free(task);
}

uint32_t scoop_task_u64_state(uint64_t task_handle) {
  if (task_handle == 0) {
    return SCOOP_TASK_STATE_CREATED;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;
  (void)pthread_mutex_lock(&task->lock);
  uint32_t state = (uint32_t)task->state;
  (void)pthread_mutex_unlock(&task->lock);
  return state;
}

uint64_t scoop_task_u64_result(uint64_t task_handle) {
  if (task_handle == 0) {
    return 0;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;
  (void)pthread_mutex_lock(&task->lock);
  uint64_t v = task->result_u64;
  (void)pthread_mutex_unlock(&task->lock);
  return v;
}

uint32_t scoop_task_u64_try_start(uint64_t task_handle, uint64_t executor_handle) {
  if (task_handle == 0 || executor_handle == 0) {
    return 0;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;
  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;

  (void)pthread_mutex_lock(&task->lock);
  if (task->state != SCOOP_TASK_STATE_CREATED) {
    (void)pthread_mutex_unlock(&task->lock);
    return 0;
  }
  if (task->body_fn == 0) {
    // 没有 body 的 task 只能由外部驱动完成（例如 I/O completion）。
    (void)pthread_mutex_unlock(&task->lock);
    return 0;
  }
  task->state = SCOOP_TASK_STATE_SCHEDULED;
  (void)pthread_mutex_unlock(&task->lock);

  ScoopExecutorJob *job = (ScoopExecutorJob *)malloc(sizeof(ScoopExecutorJob));
  if (job == 0) {
    exit(3);
  }
  job->next = 0;
  job->kind = SCOOP_EXECUTOR_JOB_RUN_TASK_U64;
  job->_reserved_u32 = 0;
  job->as.run_task.task = task;
  scoop_executor_enqueue_job(executor, job);

  return 1;
}

uint32_t scoop_task_u64_complete(uint64_t task_handle, uint64_t value) {
  if (task_handle == 0) {
    return 0;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;

  (void)pthread_mutex_lock(&task->lock);
  if (task->state == SCOOP_TASK_STATE_COMPLETED) {
    // one-shot：重复 complete 为运行期错误（与 continuation resume/join 对齐）。
    (void)pthread_mutex_unlock(&task->lock);
    exit(3);
  }

  task->state = SCOOP_TASK_STATE_COMPLETED;
  task->result_u64 = value;

  ScoopTaskWaiter *waiters = task->waiters_head;
  task->waiters_head = 0;
  task->waiters_tail = 0;
  (void)pthread_mutex_unlock(&task->lock);

  // 把 waiters 的 continuation 入队到对应 executor。
  ScoopTaskWaiter *it = waiters;
  while (it != 0) {
    ScoopTaskWaiter *next = it->next;
    scoop_executor_enqueue_resume_u64_pinned(it->executor, it->continuation, value);
    free(it);
    it = next;
  }

  return 1;
}

uint32_t scoop_task_u64_on_complete_resume_u64(uint64_t task_handle,
                                               uint64_t executor_handle,
                                               void *continuation) {
  if (task_handle == 0 || executor_handle == 0 || continuation == 0) {
    return 0;
  }

  ScoopTaskU64 *task = (ScoopTaskU64 *)(uintptr_t)task_handle;
  ScoopExecutor *executor = (ScoopExecutor *)(uintptr_t)executor_handle;

  if (!scoop_pin(continuation)) {
    exit(3);
  }

  (void)pthread_mutex_lock(&task->lock);
  if (task->state == SCOOP_TASK_STATE_COMPLETED) {
    uint64_t value = task->result_u64;
    (void)pthread_mutex_unlock(&task->lock);
    scoop_executor_enqueue_resume_u64_pinned(executor, continuation, value);
    return 1;
  }

  ScoopTaskWaiter *node = (ScoopTaskWaiter *)malloc(sizeof(ScoopTaskWaiter));
  if (node == 0) {
    (void)pthread_mutex_unlock(&task->lock);
    (void)scoop_unpin(continuation);
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

  (void)pthread_mutex_unlock(&task->lock);
  return 1;
}

static void scoop_task_u64_run_body_on_executor(ScoopTaskU64 *task, ScoopExecutor *executor) {
  (void)executor;
  if (task == 0) {
    return;
  }

  ScoopTaskBodyU64Fn body_fn = 0;
  void *body_ctx = 0;

  (void)pthread_mutex_lock(&task->lock);
  if (task->state == SCOOP_TASK_STATE_COMPLETED) {
    (void)pthread_mutex_unlock(&task->lock);
    return;
  }
  task->state = SCOOP_TASK_STATE_RUNNING;
  body_fn = task->body_fn;
  body_ctx = task->body_ctx;
  (void)pthread_mutex_unlock(&task->lock);

  uint64_t value = 0;
  if (body_fn != 0) {
    value = body_fn(body_ctx);
  }

  (void)scoop_task_u64_complete((uint64_t)(uintptr_t)task, value);
}

