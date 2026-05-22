// Scoop C runtime: object singleton once / guard primitive (early stage).
//
// P6-T03：该 primitive 只服务 `object` / `companion object` 单例初始化；
// top-level eager init 使用编译器私有 guard，不复用 runtime once 路径。
//
// 设计目标（early stage）：
// - 单进程内 once：同一 guard 只允许一个线程执行初始化逻辑；
// - 其它线程在初始化进行中会等待，直到初始化完成；
// - 允许“递归初始化”的最小语义：若初始化线程在 init 过程中再次触发同一 guard，
//   则直接视为“已在初始化中”，避免自旋死锁（与旧的单线程 bool guard 行为一致）。
//
// 说明：
// - 该实现使用 clang/GCC 的 `__atomic_*` builtin，对普通 `uint64_t` 内存做原子操作，
//   避免把 guard 声明为 C11 `_Atomic` 类型导致跨语言/跨模块 ABI 表述复杂化。
// - guard 的布局：一个 `uint64_t` word，低 2 bit 表示状态，其余 bit 存 owner thread id。
//
//   state:
//     0 = uninitialized
//     1 = initializing
//     2 = initialized
//
//   word = (owner_id << 2) | state
//
// - owner_id 仅在 state==1 时有效，用于检测“同线程重入”。

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "platform/platform.h"

enum {
  SCOOP_ONCE_STATE_UNINITIALIZED = 0u,
  SCOOP_ONCE_STATE_INITIALIZING = 1u,
  SCOOP_ONCE_STATE_INITIALIZED = 2u,
};

static inline uint32_t scoop_once_state(uint64_t word) {
  return (uint32_t)(word & 0x3ull);
}

static inline uint64_t scoop_once_owner(uint64_t word) {
  return word >> 2;
}

// 把当前线程标识压缩/哈希为一个非 0 的 62-bit id（用于 owner 判定）。
static uint64_t scoop_once_current_thread_id62(void) {
  ScoopPlatformThread self = scoop_platform_thread_self();
  uint64_t id = 0;

  if (sizeof(ScoopPlatformThread) <= sizeof(id)) {
    // 直接拷贝低位字节（thread handle 常见为指针/整数）。
    (void)memcpy(&id, &self, sizeof(ScoopPlatformThread));
  } else {
    // 罕见平台：thread handle 不是 pointer-sized，做一个最小 FNV-1a hash。
    const uint8_t *p = (const uint8_t *)&self;
    uint64_t h = 1469598103934665603ull;
    for (size_t i = 0; i < sizeof(ScoopPlatformThread); i++) {
      h ^= (uint64_t)p[i];
      h *= 1099511628211ull;
    }
    id = h;
  }

  // 压缩为 62-bit 并确保非 0（0 用于表示“未知 owner”）。
  id &= ((1ull << 62) - 1ull);
  if (id == 0) {
    id = 1;
  }
  return id;
}

// 为动态链接场景提供“canonical guard”：
//
// - 当同一逻辑单例被编译进多个动态库时，可能出现多个同名 guard（不同地址）。
// - 通过 `dlsym(RTLD_DEFAULT, symbol_name)` 选取“进程内 canonical 的那一个”guard 地址，
//   让所有动态库最终对同一 guard 做原子状态机操作，从而保证 init 只执行一次。
//
// 约束/注意：
// - `symbol_name` 必须是 **guard 的符号名**（即 codegen 的 global 名称），并具有 default 可见性；
// - 在 macOS 上，RTLD_DEFAULT 不会搜索通过 `dlopen(..., RTLD_LOCAL)` 加载的 image；
//   因此若使用 dlopen 插件模型，需要确保插件以 RTLD_GLOBAL 加载（或改为显式 handle 查找）。
uint64_t *scoop_once_guard_canonicalize(const char *symbol_name,
                                       uint64_t *fallback_guard_word) {
  if (fallback_guard_word == 0) {
    return 0;
  }

  if (symbol_name == 0 || symbol_name[0] == '\0') {
    return fallback_guard_word;
  }

  void *addr = scoop_platform_dynlib_lookup_symbol_default(symbol_name);
  if (addr == 0) {
    return fallback_guard_word;
  }

  return (uint64_t *)addr;
}

// 尝试进入 once 初始化区间：
// - 返回 1：调用方获得初始化权，应执行 init 并在结束时调用 `scoop_once_end`；
// - 返回 0：已初始化或正在由其它线程初始化；调用方不应再次执行 init。
uint32_t scoop_once_begin(uint64_t *guard_word) {
  if (guard_word == 0) {
    return 0;
  }

  uint64_t tid = scoop_once_current_thread_id62();

  for (;;) {
    uint64_t cur = __atomic_load_n(guard_word, __ATOMIC_ACQUIRE);
    uint32_t state = scoop_once_state(cur);

    if (state == SCOOP_ONCE_STATE_INITIALIZED) {
      return 0;
    }

    if (state == SCOOP_ONCE_STATE_UNINITIALIZED) {
      uint64_t desired = (tid << 2) | (uint64_t)SCOOP_ONCE_STATE_INITIALIZING;
      uint64_t expected = cur;
      if (__atomic_compare_exchange_n(guard_word,
                                      &expected,
                                      desired,
                                      0,
                                      __ATOMIC_ACQ_REL,
                                      __ATOMIC_ACQUIRE)) {
        return 1;
      }
      // CAS 失败：重试。
      continue;
    }

    // state == INITIALIZING：检查是否同线程重入，避免自旋死锁。
    uint64_t owner = scoop_once_owner(cur);
    if (owner == tid) {
      return 0;
    }

    // 其它线程正在初始化：等待其完成。
    while (scoop_once_state(__atomic_load_n(guard_word, __ATOMIC_ACQUIRE)) ==
           SCOOP_ONCE_STATE_INITIALIZING) {
      scoop_platform_thread_yield();
    }
    // 回到循环：看最终状态。
  }
}

void scoop_once_end(uint64_t *guard_word) {
  if (guard_word == 0) {
    return;
  }

  // release：确保 init 期间写入的对象存储在其它线程观测到 initialized 后可见。
  __atomic_store_n(guard_word, (uint64_t)SCOOP_ONCE_STATE_INITIALIZED, __ATOMIC_RELEASE);
}
