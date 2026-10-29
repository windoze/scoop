"""Fixed workloads shared by measurement and independent correctness checks."""

CASES = [
    ("integer-mix", 1_000_000, 4096, "整数依赖链"),
    ("mandelbrot", 160, 24, "严格浮点 / 分支"),
    ("array-transform", 262_144, 1024, "浮点数组初始化、变换和归约"),
    ("branch-scan", 262_144, 1024, "整数数组与数据分支"),
    ("pointer-chase", 262_144, 1024, "依赖性随机访存"),
    ("quicksort", 32_768, 1024, "同一原地快速排序"),
    ("value-records", 131_072, 1024, "值记录数组 / Java record 对象数组"),
    ("reference-records", 131_072, 1024, "引用对象数组"),
    ("allocation-ring", 200_000, 4096, "逃逸对象分配与短存活集"),
    ("binary-trees", 10, 1, "递归对象图、分配和遍历"),
    ("interface-dispatch", 500_000, 4096, "四种动态接口接收者"),
    ("list-grow", 100_000, 1024, "动态容器增长 / Java Long 装箱"),
    ("hash-map", 100_000, 4096, "标准库整数哈希表"),
    ("decimal-format", 30_000, 1024, "整数十进制格式化"),
    ("string-build", 20_000, 1024, "字符串构建器"),
    ("unicode-scan", 100, 2, "Unicode 标量遍历"),
    ("integer-parse", 100_000, 1024, "标准库十进制解析"),
    ("parallel-1", 4_000_000, 8192, "固定工作量，1 worker"),
    ("parallel-4", 4_000_000, 8192, "固定工作量，4 workers"),
    ("parallel-8", 4_000_000, 8192, "固定工作量，8 workers"),
]


def xorshift(value):
    value ^= (value << 13) & 0xFFFFFFFF
    value ^= value >> 17
    return value ^ ((value << 5) & 0xFFFFFFFF)


def mix(n, seed):
    for _ in range(n):
        seed = xorshift(seed)
    return seed


def oracle(kind, n, seed):
    """Independent reference, using formulas or Python builtins where possible."""
    if kind == 0:
        return mix(n, seed)
    if kind == 1:
        total = 0
        for y in range(n):
            for x in range(n):
                cr = x * 3.0 / n - 2.0 + seed * 0.000001
                ci = y * 2.0 / n - 1.0
                zr = zi = 0.0
                step = 0
                while step < 64 and zr * zr + zi * zi <= 4.0:
                    zr, zi = zr * zr - zi * zi + cr, 2.0 * zr * zi + ci
                    step += 1
                total += step
        return total
    if kind == 2:
        return sum(((i & 1023) + seed) * 5 + (i & 255) * 8 for i in range(n))
    if kind in (3, 5):
        values = []
        for _ in range(n):
            seed = xorshift(seed)
            values.append(seed)
        if kind == 3:
            return sum((x & 1023) * (1 if x & 1 == 0 else -1) for x in values)
        return sum(x * (i % 257) for i, x in enumerate(sorted(x & 0x7FFFFFFF for x in values)))
    if kind == 4:
        return (8 * n * (n - 1) // 2) & 0x7FFFFFFF
    if kind in (6, 7):
        return 11 * n * (n - 1) // 2 + n * (3 * seed + 14)
    if kind == 8:
        return sum(i ^ (i + seed) for i in range(max(0, n - 1024))) + n - 1 + seed
    if kind == 9:
        from math import comb

        return sum(comb(12, k) * ((seed + i + 12 + k) & 1023) for i in range(n) for k in range(13))
    if kind == 10:
        x = seed
        for i in range(n):
            k = (i & 63) + seed
            if i & 3 == 0:
                x += k
            elif i & 3 == 1:
                x ^= k
            elif i & 3 == 2:
                x *= k
            else:
                x = (x >> 1) + k
            x &= 0x7FFFFFFF
        return x
    if kind == 11:
        return n * (n - 1) // 2 + n * seed
    if kind == 12:
        return sum(i // 65536 + 1 for i in range(n))
    if kind == 13:
        total = 0
        for _ in range(n):
            seed = (seed * 1664525 + 1013904223) & 0xFFFFFFFF
            total += len(str(seed))
        return total + ord(str(seed)[0])
    if kind == 14:
        text = "".join(f"{i ^ seed}," for i in range(n))
        return len(text) + ord(text[0])
    if kind == 15:
        return n * sum(map(ord, "abé雪🙂z" * 512 + str(seed)))
    if kind == 16:
        return sum((i & 255) * 1234567 + seed for i in range(n))
    if 17 <= kind <= 19:
        return sum(mix(n // 8, seed + shard) for shard in range(8))
    raise ValueError(kind)
