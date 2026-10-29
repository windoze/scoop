# 生成代码与采样摘录

这些观察来自单独的 release + line tables 可执行文件，不用于正式耗时表。
完整符号化 `sample` 文本保存在 [profiles.json](profiles.json)；原始文件、独立调试文件、
完整汇编和符号映射位于忽略的 `benchmark/logs/`、`benchmark/build/profile/`。

复跑构建和采样：

```sh
~/.scoop/bin/scoop build benchmark/scoop --release --keep-symbols \
  --debug-info line-tables --emit symbols --dump-dir benchmark/build/profile/dumps \
  --target-dir benchmark/build/profile/target --cache-dir benchmark/build/profile/cache \
  -o benchmark/build/scoop-profile
python3 benchmark/profile.py
```

每项 `sample` 采集 3 秒、间隔 1 ms。符号化复用仓库的 `tests/benchmarks/symbolize.py`，
保留原始线程、调用树与折叠帧，不把后台 GC 线程的等待算作 CPU 消耗。

## 条件归约

Scoop `branchScan` 内联到 `main`，归约热路径：

```asm
100043ecc: ldr   w9, [x9, #0x18]
100043ed0: and   x10, x9, #0x3fe
100043ed4: and   x11, x9, #0x3ff
100043ed8: tst   w9, #0x1
100043edc: add   x10, x28, x10
100043ee0: sub   x11, x28, x11
100043ee4: csel  x28, x10, x11, eq
```

Go 同一路径（`go tool objdump -s main.branchScan`）：

```asm
numeric.go:62  0x10009f78c  MOVWU (R2)(R3<<2), R4
numeric.go:63  0x10009f790  TBNZ  $0, R4, 4(PC)
numeric.go:64  0x10009f794  AND   $1023, R4, R4
numeric.go:64  0x10009f798  ADD   R4, R1, R1
numeric.go:64  0x10009f79c  JMP   -7(PC)
numeric.go:66  0x10009f7a0  AND   $1023, R4, R4
numeric.go:66  0x10009f7a4  SUB   R4, R1, R1
```

两者输入都由相同的 xorshift 产生。这说明真实存在无数据分支选择与条件跳转的区别；
本次没有 PMU 计数，不能精确拆分其对最终时间差的贡献。

## 动态接口调用

Scoop 调用循环中的一段：

```asm
100043c50: ldar w8, [x27]
100043c54: cbz  w8, 0x100043c64
100043c58: str  x25, [sp, #0xe0]
100043c5c: bl   _scoop_rt_safepoint
100043c60: ldr  x25, [sp, #0xe0]
100043c6c: ldr  x9, [x25, #0x10]
100043c70: and  x8, x19, #0x3f
100043c74: cmp  x8, x9
100043c78: b.ge 0x100044208
100043c7c: add  x8, x25, x8, lsl #4
100043c80: mov  x1, x28
100043c84: ldp  x0, x9, [x8, #0x18]
100043c88: ldr  x8, [x9]
100043c8c: stp  x25, x0, [sp, #0xd8]
100043c90: blr  x8
100043c94: ldr  x25, [sp, #0xd8]
```

`Add.apply` 的全部六条指令：

```asm
stp x29, x30, [sp, #-0x10]!
mov x29, sp
ldr x8, [x0, #0x10]
add x0, x8, x1
ldp x29, x30, [sp], #0x10
ret
```

采样调用树覆盖 `dispatch` 和四个 `apply` 方法；八次 kernel 的 GC 诊断为零次收集。
因此该项不能用“GC 暂停”解释。这里没有禁用 poll、根恢复或建帧来制造更快的比较结果。

## Unicode、解析和 HashMap

采样直接显示的调用关系如下，数值百分比不从 `<deduplicated_symbol>` 折叠行推算：

```text
unicode
  StringIterator.next
    scoop_rt_string_character_at_byte_leaf

parseIntegers
  toLong
    parseSignedInteger
      parseIntegerMagnitude
        String.iterator
        StringIterator.next
          scoop_rt_string_character_at_byte_leaf
        textDigit

hashMap
  HashMap.locate
  HashMap.insertAbsent
    HashMap.ensureRoom
      HashStorage.rehashed
        HashStorage.<init>
          scoop_runtime_alloc_slow_impl
            __bzero / collection and VM reclamation
```

源码位置：

- [StringIterator](../../../sysroot/src/scoop.core/src/string_iterator.scoop)：保存 `String` 引用与字节游标，逐次产生 `Option<Char>`。
- [整数解析](../../../sysroot/src/scoop.std/src/text/integer_parse.scoop)：每次解析先取得 iterator，逐字符调用 `textDigit` 和 `next`。
- [HashMap](../../../sysroot/src/scoop.std/src/collections/hash_map.scoop) 与 [HashStorage](../../../sysroot/src/scoop.std/src/collections/hash_storage.scoop)：查询、增长、状态数组与 entry 数组。

八次完整 kernel 的独立诊断：

| 负载 | 总分配 bytes | minor / full | GC 暂停 ms |
|---|---:|---:|---:|
| 动态接口调用 | 21,016 | 0 / 0 | 0 |
| Unicode 遍历 | 208,344 | 0 / 0 | 0 |
| 10 万次整数解析 × 8 | 25,693,336 | 24 / 0 | 0.116 |
| 哈希表 × 8 | 58,729,368 | 0 / 4 | 1.128 |
| 引用记录 × 8 | 92,274,968 | 6 / 3 | 5.151 |

分配量包含进程内少量固定初始化。GC 统计、采样和普通计时各自独立执行；诊断结果用于
解释路径，不替代无诊断开销的正式结果。
