// Scoop C runtime: std `scoop.channels` (platform backend, early stage).
//
// TODO T1319d：
// - 为 sysroot 的 `scoop.channels`（unbounded mpsc channel）提供最小可执行实现；
// - 由 LLVM codegen 将 sysroot 表面直接映射到本文件导出的 C 符号；
// - 当前阶段只覆盖 host 平台（POSIX/pthread 通过 `runtime/c/platform` 收敛）。
//
// 设计约定（early stage）：
// - `Channel<T>` 在 sysroot 侧声明为 class（引用类型），这里实现为 “GC-managed 对象”
//   （以 `ScoopGcObjectHeader` 开头，并通过 `scoop_alloc` 分配）。
// - 元素使用 “u64 word” 作为 ABI 承载：整数/布尔按 u64 编码；引用/字符串按 ptr→u64 编码。
//   该 ABI 与 `scoop_array.c` 的 word array 对齐，便于编译器侧复用 `coerce_u64_word`。
// - 队列节点用 `malloc/free` 管理（避免要求 GC 扫描 node 链表）；unbounded 语义下可能泄漏，
//   但在 early stage 的 fixtures 回归范围内可接受。后续可接入 type descriptor 或 pinning 扩展。
// - `close()` 幂等：多次调用不崩溃；close 会唤醒阻塞在 `recv` 的线程。
// - `recv()` 为阻塞式：当队列为空且未 close 时阻塞；当队列为空且已 close 时返回 0。

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "platform/platform.h"
#include "scoop_gc.h"

// `scoop_alloc` / 线程注册 API 由 `scoop_runtime.c` 提供；这里仅做前置声明。
void *scoop_alloc(uint64_t size);
void scoop_thread_register(void);

typedef struct ScoopChannelsNode {
  uint64_t value;
  struct ScoopChannelsNode *next;
} ScoopChannelsNode;

typedef struct ScoopChannelsChannel {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex lock;
  ScoopPlatformCondVar cond;
  uint32_t closed;
  uint32_t _reserved_u32;
  ScoopChannelsNode *head;
  ScoopChannelsNode *tail;
} ScoopChannelsChannel;

void *scoop_channels_channel_create(void) {
  scoop_thread_register();

  ScoopChannelsChannel *ch =
      (ScoopChannelsChannel *)scoop_alloc((uint64_t)sizeof(ScoopChannelsChannel));
  if (ch == 0) {
    return 0;
  }

  if (!scoop_platform_sync_mutex_init(&ch->lock)) {
    return 0;
  }
  if (!scoop_platform_sync_condvar_init(&ch->cond)) {
    scoop_platform_sync_mutex_destroy(&ch->lock);
    return 0;
  }
  ch->closed = 0;
  ch->_reserved_u32 = 0;
  ch->head = 0;
  ch->tail = 0;
  return (void *)ch;
}

uint32_t scoop_channels_send_u64(void *channel_obj, uint64_t value) {
  if (channel_obj == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopChannelsChannel *ch = (ScoopChannelsChannel *)channel_obj;

  scoop_platform_sync_mutex_lock(&ch->lock);

  if (ch->closed) {
    scoop_platform_sync_mutex_unlock(&ch->lock);
    return 0;
  }

  ScoopChannelsNode *node = (ScoopChannelsNode *)malloc(sizeof(ScoopChannelsNode));
  if (node == 0) {
    // early stage：OOM 时保守失败（send 返回 false），避免崩溃。
    scoop_platform_sync_mutex_unlock(&ch->lock);
    return 0;
  }

  node->value = value;
  node->next = 0;

  if (ch->tail == 0) {
    ch->head = node;
    ch->tail = node;
  } else {
    ch->tail->next = node;
    ch->tail = node;
  }

  scoop_platform_sync_condvar_signal(&ch->cond);
  scoop_platform_sync_mutex_unlock(&ch->lock);
  return 1;
}

uint32_t scoop_channels_recv_u64(void *channel_obj, uint64_t *out_value) {
  if (channel_obj == 0 || out_value == 0) {
    return 0;
  }

  scoop_thread_register();

  ScoopChannelsChannel *ch = (ScoopChannelsChannel *)channel_obj;

  scoop_platform_sync_mutex_lock(&ch->lock);

  while (ch->head == 0 && !ch->closed) {
    scoop_platform_sync_condvar_wait(&ch->cond, &ch->lock);
  }

  ScoopChannelsNode *node = ch->head;
  if (node == 0) {
    // 队列为空：
    // - 若已 close：不会再有值
    // - 若未 close：理论上不会到这里（while 会等待）
    scoop_platform_sync_mutex_unlock(&ch->lock);
    return 0;
  }

  ch->head = node->next;
  if (ch->head == 0) {
    ch->tail = 0;
  }

  scoop_platform_sync_mutex_unlock(&ch->lock);

  *out_value = node->value;
  free(node);
  return 1;
}

void scoop_channels_close(void *channel_obj) {
  if (channel_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopChannelsChannel *ch = (ScoopChannelsChannel *)channel_obj;

  scoop_platform_sync_mutex_lock(&ch->lock);
  ch->closed = 1;
  scoop_platform_sync_condvar_broadcast(&ch->cond);
  scoop_platform_sync_mutex_unlock(&ch->lock);
}
