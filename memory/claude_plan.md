# Current Task: T0116 — 核心库 hardcoded 类型限制清单（跟踪任务）

## Status: COMPLETED

## Summary
Audited all 8 hardcoded type limitation items against the current codebase:
1. Set Int-only → T1818
2. Map Int-only → T1818
3. Collection ops Int-only → T1822
4. Scope functions Int-only → T1822
5. Task<T> Int-only → deferred (awaiting T0124-T0128 generics)
6. print/println limited → T0131 (where constraints); Float deferred
7. Hashable hash() default 0 → Int/String done (T1817), others deferred
8. MutableArray COW → design decision (value semantics consistency)

All items now have task links or explicit "design decision/deferred" annotations with file location references.
