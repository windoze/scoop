# Scoop Task Design

## Objective

This document defines the intended direction for `Task<T>` after the continuation
and state-machine work landed:

- minimize task-specific runtime / codegen surface;
- move most task logic from C runtime / LLVM special cases into Scoop code;
- keep `scoop.core` small and focused on language-facing async sugar targets;
- leave executor / wake / reactor / structured-concurrency work to a later stdlib
  stage.

The target model is:

- `Task<T>` remains the general-purpose async API for normal users;
- raw `Continuation<Resume, Answer, eff E>` remains the advanced control-flow API;
- core owns only the language-specific async surface and a tiny manually-drivable
  task abstraction;
- schedulers, run queues, wakeup registration, callback adapters, and reactor
  integration belong to stdlib rather than to `scoop.core`.

## Non-Goals

This document does not define:

- a public executor interface;
- a public wakeup / waker registration API;
- public `spawn` / `join` / structured concurrency;
- a reactor or async I/O callback framework;
- a new task-specific continuation ABI;
- a requirement that `Task` become fully lock-free.

## Design Principles

### 1. Task must be built on the generic continuation contract

`Task` should not introduce a second resume model next to
`Continuation.resume(...)`.

The only resume payload / answer transport should remain the generic
continuation path:

- suspension captures a continuation;
- resumption provides that continuation's payload;
- normal completion returns that continuation's answer.

`Task` is only a thin object-level wrapper that interprets a private continuation
answer carrier and maps it back to the public task-driving result.

### 2. Core should own only the async sugar target and minimal manual driving

`scoop.core` should stay small. It should contain only:

- the public `Task<T>` type;
- the public result type of a manual drive operation;
- the `Async.await` effect operation used by `await`;
- private internal helper types / functions needed as desugar targets.

It should not grow executor queues, wake handles, scheduler policies, or reactor
APIs.

### 3. Driving a task is not the same as waking a task

These are different layers:

- **wake / enqueue / readiness**: scheduler-facing information that a task is now
  worth trying again;
- **step**: one drive attempt on a task;
- **resume(payload)**: low-level continuation operation that feeds the payload
  required by a particular suspension point.

Because these layers are different, a general `Task.step(arg)` API is the wrong
  abstraction.

The payload belongs to the suspended continuation or to the awaited source that
  eventually produces the payload, not to the task-driver API.

### 4. Spurious drive attempts are allowed

An executor should try to drive tasks only when they are likely to make
progress, but an unnecessary drive attempt is not a semantic error.

If a task cannot make progress yet, `step()` should simply return `Pending`.

This is inefficient, but it is not forbidden.

### 5. Cross-thread driving should be supported

If one thread creates a task and another thread later drives it, that should be
valid.

The existing continuation contract already supports cross-thread resume by
reinstalling the captured handler stack on the resuming thread. The task layer
only needs enough synchronization to ensure that at most one thread owns a drive
attempt for a given task at a time.

## Proposed Core Surface

### Public surface

Target public surface:

```kotlin
package scoop.core

enum TaskStep<T> {
    Pending,
    Ready(val value: T),
}

class Task<T>

fun <T> Task<T>.step(): TaskStep<T>

effect Async {
    fun <T> await(task: Task<T>): T
}
```

The current `poll()` / `Poll<T>` naming is misleading, because Scoop's current
`poll()` actively drives the task rather than passively observing readiness.

This design does **not** preserve that naming for compatibility. The task design
is still in progress, there is no release history to maintain, and the surface
should be renamed directly:

- `step()` is the public driver;
- `TaskStep<T>` is the public result type;
- `poll()` and `Poll<T>` should be removed rather than kept as aliases.

## What Stays Special in the Compiler

To minimize surface while preserving language features, the compiler should keep
only the pieces that are truly language-specific:

- parsing / typechecking of `async { ... }`;
- parsing / typechecking of `async fun foo(): T`;
- parsing / typechecking of `await expr`;
- the typing rule that `await expr` lowers to `perform Async.await(expr)`;
- the rule that `async` captures the internal `/ Async` effect instead of
  exposing it on the caller signature;
- internal erasure of task-private await payloads to `Any`, if that remains the
  simplest lowering strategy.

Everything else should become ordinary Scoop code.

In particular, the compiler should stop owning:

- a task-specific object layout;
- a task-specific poll runtime ABI;
- task-specific ready / pending runtime constructors;
- task-specific join / from-result runtime helpers;
- task-specific LLVM codegen branches beyond normal function / method calls.

## What Moved Out of Runtime / Codegen

The old task-specific runtime / codegen ABI has been removed:

- `scoop_task_create`
- `scoop_task_poll`
- `scoop_task_step_ready`
- `scoop_task_step_pending`
- `scoop_task_from_result`
- `scoop_task_join`

The remaining internal helper names are ordinary Scoop definitions such as
`__task_create(...)`, `__task_step_ready(...)`, `__task_step_pending(...)`,
`__task_from_result(...)`, and `__task_join(...)`. They are no longer runtime
intrinsics or LLVM special cases.

## Internal Scoop-Level Task Model

The current internal model is:

### Internal drive result

```kotlin
package scoop.core

enum __TaskStepResult<T> {
    Pending(
        val awaited: Task<(Int, Any)>,
        val continuation: Continuation<(Int, Any), __TaskStepResult<T>>
    ),
    Ready(val value: (Int, Any)),
}
```

Notes:

- This is the private continuation answer carrier.
- It replaces the removed task-only C runtime carrier.
- The compiler still erases internal resume payloads through the `(Int, Any)`
  transport pair `__task_transport_pack(...)` / `__task_transport_unpack(...)`,
  but the answer carrier stays explicit.

### Internal task state

```kotlin
package scoop.core

enum __TaskState<T> {
    Created(val start: () -> __TaskStepResult<T>),
    Running,
    Waiting(
        val awaited: Task<(Int, Any)>,
        val continuation: Continuation<(Int, Any), __TaskStepResult<T>>
    ),
    Completed(val value: (Int, Any)),
}
```

### Task object

```kotlin
package scoop.core

class Task<T>(
    val __lock: Mutex,
    var __state: __TaskState<T>
)
```

The important point is that the authoritative task state is now a normal
Scoop-level object model rather than a private C runtime struct.

## Step Algorithm

The canonical public operation is:

```kotlin
fun <T> Task<T>.step(): TaskStep<T>
```

The intended algorithm is:

1. Lock the task.
2. Read current state.
3. If state is `Completed(value)`, unlock and return `Ready(value)`.
4. If state is `Running`, unlock and return `Pending`.
5. If state is `Created(start)`, replace state with `Running`, detach `start`,
   unlock, run `start()`, then publish the resulting next state.
6. If state is `Waiting(awaited, continuation)`, replace state with `Running`,
   detach both values, unlock, drive `awaited.step()`, and:
   - if awaited returns `Pending`, restore `Waiting(awaited, continuation)` and
     return `Pending`;
   - if awaited returns `Ready(valueTransport)`, call
     `continuation.resume(valueTransport)`, obtain the next private
     `__TaskStepResult<T>`, and publish it.
7. Publishing a private driver step means:
   - `Ready(value)` -> set task state to `Completed(value)`;
   - `Pending(awaited2, continuation2)` -> set task state to
     `Waiting(awaited2, continuation2)`.
8. After publishing:
   - if task became `Completed(value)`, return `Ready(value)`;
   - if task became `Waiting(...)`, return `Pending`.

Important invariants:

- no lock is held while running user code or while resuming a continuation;
- only the thread that changed the state to `Running` owns the detached closure /
  continuation for that drive attempt;
- `Completed` is sticky and cached;
- a task must never duplicate or re-use a consumed continuation.

## Why `step()` Takes No Argument

`step(arg)` is not the right core abstraction.

Reasons:

- a task may advance without any new external data at all, for example when it is
  first started;
- a task waiting on another task obtains its resume payload from that awaited
  task's completion transport result;
- different suspension points need different payload types, so a public
  `step(arg)` would either leak low-level continuation typing into `Task` or
  force everything into `Any` / sum-type plumbing at the wrong layer;
- the correct low-level payload API already exists as
  `Continuation<Resume, Answer>.resume(payload)`.

Therefore:

- `Task.step()` is a drive attempt, not a payload sink;
- low-level payload delivery belongs to continuation resume and to awaited source
  completion records;
- future wake / readiness systems should only tell the executor "this task is
  worth trying again", not "here is the payload for `Task.step(...)`".

## Synchronization Design

To support cross-thread task driving, the task object needs synchronization even
if executor / wake / reactor remain deferred.

### Required guarantees

The task layer should guarantee:

- multiple threads may call `step()` on the same task;
- at most one thread may actively drive a given task at a time;
- if a thread finds the task already being driven, it gets `Pending` rather than
  blocking or corrupting state;
- if a task is already completed, any thread may read the cached result;
- if a waiting task is driven on a different thread, the captured continuation is
  resumed on that new thread.

### Minimal synchronization substrate

The minimal substrate is a per-task mutex.

`CondVar` is **not** required for the core task API:

- `step()` is non-blocking at the task level;
- no public `join()` is part of the core task design;
- no public executor wait queue is in scope yet.

`CondVar` can be introduced later in stdlib-level `join`, schedulers, or reactor
adapters.

### Preferred implementation source

Preferred direction:

- reuse the existing sync runtime surface already backing `scoop.sync`
  (`Mutex`, later `CondVar` when needed);
- do **not** add a new task-specific lock runtime.

The current implementation directly reuses `scoop.sync.Mutex`; there is no
separate task-only lock type.

## Cross-Thread Resume and GC

Cross-thread task driving relies on the existing continuation contract, not on a
new task-specific mechanism.

The required properties are:

- captured continuations remain one-shot;
- resuming from another thread reinstalls the captured handler stack on that
  thread for the duration of resumed execution;
- if resumed code suspends again, it produces a fresh continuation;
- task state stores strong references to its detached closure / awaited task /
  continuation so that GC can trace them;
- state transitions are synchronized so that two threads cannot both believe they
  own the same continuation.

This means task-level synchronization is about exclusive ownership of the current
drive attempt, while continuation-level semantics still define what it means to
resume on another thread.

## What Does *Not* Belong in Core

The following should remain deferred and should live in stdlib, not in
`scoop.core`:

- `Executor`
- work queues
- wakeup registration
- reactor integration
- callback completion adapters
- public `spawn`
- public `join`
- scheduling policy (FIFO, round-robin, work-stealing, etc.)

Future stdlib layers may look like:

- `scoop.task` for executor traits / queues / spawn helpers;
- `scoop.io` or similar for reactor / callback / OS integration.

The core task API only needs to be manually drivable.

## Wake Tokens

When a later stdlib reactor / callback layer needs a long-lived wake token, that
token should not be a pinned task reference.

It should normally be a stable GC handle token, typically `GcHandle.raw`,
round-tripped through native registration state.

That wake token belongs to stdlib-level scheduling / registration logic, not to
the core `Task` API.

## Recommended Migration Plan

### Phase 1: Surface cleanup

- This phase defines the current public `scoop.core` task surface.
- Rename `Poll<T>` to `TaskStep<T>`.
- Remove `poll()` and keep `step()` as the only public drive operation.
- Update docs, sysroot surface, lowering targets, and tests to use the renamed
  surface directly.

### Phase 2: Move internal driver model into Scoop

- Re-express task state and private driver result as ordinary Scoop types.
- Re-express task creation / step-ready / step-pending as ordinary Scoop helper
  code or direct constructors.
- Keep the compiler's `async` / `await` lowering, but retarget it to ordinary
  Scoop helper definitions instead of runtime intrinsics.

### Phase 3: Delete task-specific runtime / codegen ABI

- Remove task-only codegen branches.
- Remove task-only runtime symbols and `runtime/c/scoop_task.c`.
- Keep only generic continuation, GC, thread, and sync runtime layers.

### Phase 4: Later stdlib work

- design executor traits / APIs;
- design wakeup / enqueue contracts;
- design reactor and callback completion integration;
- design public `spawn` / `join` on top of the minimal task core.

## Summary

The intended end state is:

- `Task` is mostly implemented in Scoop;
- compiler special handling is limited to async sugar and private lowering glue;
- runtime special handling is limited to generic continuation / GC / thread /
  sync infrastructure;
- the public core task API is small: `Task<T>`, `TaskStep<T>`, `step()`, and
  `Async.await`;
- executor / wake / reactor stay out of `scoop.core` and are designed later in
  stdlib.
