//! GC microbench（early stage）。
//!
//! 目标（TODO T1406d）：
//! - 提供一个“可重复运行、可比较”的最小基准工具；
//! - 覆盖两类指标：
//!   1) 分配吞吐（alloc throughput）
//!   2) 碎片化（reserved bytes vs live bytes）
//! - 结果用于本地对比 baseline vs Immix；不做跨机器阈值 gating（避免不稳定）。
//!
//! 用法示例：
//! - baseline：
//!   `cargo run -p scoop_runtime --release --bin gc_microbench --no-default-features --features gc-baseline -- throughput`
//! - immix：
//!   `cargo run -p scoop_runtime --release --bin gc_microbench --no-default-features --features gc-immix -- fragmentation`

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use scoop_runtime::gc_backend::GC_BACKEND;

// C runtime 提供的最小 ABI（用于 microbench；不对外暴露 Rust API）。
//
// 注意：这些符号受 `runtime/c/scoop_runtime_api.h` allowlist 审计约束。
unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut c_void;
    fn scoop_gc_collect();

    fn scoop_pin(obj: *mut c_void) -> u32;
    fn scoop_unpin(obj: *mut c_void) -> u32;

    fn scoop_gc_debug_heap_bytes_allocated() -> u64;
    fn scoop_gc_debug_heap_bytes_freed() -> u64;
    fn scoop_gc_debug_heap_bytes_reserved() -> u64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Throughput,
    Fragmentation,
}

impl Scenario {
    fn as_str(self) -> &'static str {
        match self {
            Scenario::Throughput => "throughput",
            Scenario::Fragmentation => "fragmentation",
        }
    }
}

#[derive(Debug, Clone)]
struct Args {
    scenario: Scenario,

    // 通用参数
    object_size: u64,
    json: bool,
    output: Option<PathBuf>,

    // throughput 参数
    threads: u32,
    rounds: u32,
    batch: u32,

    // fragmentation 参数
    initial: u32,
    pin_stride: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            scenario: Scenario::Throughput,
            object_size: 256,
            json: false,
            output: None,
            threads: 1,
            rounds: 50,
            batch: 50_000,
            initial: 200_000,
            pin_stride: 100,
        }
    }
}

fn print_help() {
    eprintln!(
        r#"gc_microbench（TODO T1406d）

用法：
  gc_microbench <throughput|fragmentation> [options]

通用 options：
  --object-size <bytes>      单次分配 size（包含对象头）
  --json                     以 JSON 输出（单行）
  --output <path>            额外把输出写入文件（仍会输出到 stdout）

throughput options：
  --threads <n>              并发线程数（默认 1；>1 时用于对比锁争用优化）
  --rounds <n>               轮数（每轮会执行一次 GC）
  --batch <n>                每轮每线程分配对象数（threads>1 时为“每线程”）

fragmentation options：
  --initial <n>              初始总分配对象数
  --pin-stride <n>           每 N 个对象 pin 1 个（形成“稀疏存活”）

示例：
  # 吞吐：baseline vs immix（建议用 tools/gc_microbench.sh 一键跑）
  cargo run -p scoop_runtime --release --bin gc_microbench --no-default-features --features gc-baseline -- throughput
  cargo run -p scoop_runtime --release --bin gc_microbench --no-default-features --features gc-immix -- throughput

  # 碎片化：稀疏存活（pinned）导致 reserved bytes 升高（non-moving Immix 的典型现象）
  cargo run -p scoop_runtime --release --bin gc_microbench --no-default-features --features gc-immix -- fragmentation --initial 200000 --pin-stride 100
"#
    );
}

fn parse_u64(name: &str, value: &str) -> u64 {
    value.parse::<u64>().unwrap_or_else(|_| {
        eprintln!("参数解析失败：{name}={value}");
        std::process::exit(2);
    })
}

fn parse_u32(name: &str, value: &str) -> u32 {
    value.parse::<u32>().unwrap_or_else(|_| {
        eprintln!("参数解析失败：{name}={value}");
        std::process::exit(2);
    })
}

fn parse_args() -> Args {
    let mut args = Args::default();

    let mut it = std::env::args().skip(1);
    let Some(first) = it.next() else {
        print_help();
        std::process::exit(2);
    };

    match first.as_str() {
        "-h" | "--help" => {
            print_help();
            std::process::exit(0);
        }
        "throughput" => args.scenario = Scenario::Throughput,
        "fragmentation" => args.scenario = Scenario::Fragmentation,
        other => {
            eprintln!("未知场景：{other}");
            print_help();
            std::process::exit(2);
        }
    }

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--object-size" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--object-size <bytes>");
                    std::process::exit(2);
                };
                args.object_size = parse_u64("object_size", &v);
            }
            "--json" => args.json = true,
            "--output" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--output <path>");
                    std::process::exit(2);
                };
                args.output = Some(PathBuf::from(v));
            }
            "--rounds" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--rounds <n>");
                    std::process::exit(2);
                };
                args.rounds = parse_u32("rounds", &v);
            }
            "--threads" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--threads <n>");
                    std::process::exit(2);
                };
                args.threads = parse_u32("threads", &v);
            }
            "--batch" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--batch <n>");
                    std::process::exit(2);
                };
                args.batch = parse_u32("batch", &v);
            }
            "--initial" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--initial <n>");
                    std::process::exit(2);
                };
                args.initial = parse_u32("initial", &v);
            }
            "--pin-stride" => {
                let Some(v) = it.next() else {
                    eprintln!("缺少参数：--pin-stride <n>");
                    std::process::exit(2);
                };
                args.pin_stride = parse_u32("pin_stride", &v);
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("未知参数：{other}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    args
}

/// 读取 GC 统计：allocated/freed/reserved，并派生 live bytes。
fn read_gc_bytes() -> (u64, u64, u64, u64) {
    unsafe {
        let allocated = scoop_gc_debug_heap_bytes_allocated();
        let freed = scoop_gc_debug_heap_bytes_freed();
        let reserved = scoop_gc_debug_heap_bytes_reserved();
        let live = allocated.saturating_sub(freed);
        (allocated, freed, live, reserved)
    }
}

fn fmt_duration_ms(d: Duration) -> u128 {
    d.as_millis()
}

fn run_throughput(args: &Args) -> BenchResult {
    // 说明：不 pin 任何对象，使每轮 GC 都能把上一轮的对象全部回收（基准更稳定）。
    let threads = args.threads.max(1);
    let rounds = args.rounds.max(1);
    let batch = args.batch.max(1);

    let start_gc = read_gc_bytes();

    let mut allocations: u64 = 0;
    let mut bytes: u64 = 0;

    let t0 = Instant::now();

    if threads == 1 {
        for _ in 0..rounds {
            for _ in 0..batch {
                let p = unsafe { scoop_alloc(args.object_size) };
                if p.is_null() {
                    eprintln!("OOM：scoop_alloc(size={}) 返回 NULL", args.object_size);
                    break;
                }
                allocations += 1;
                bytes = bytes.saturating_add(args.object_size);
            }

            unsafe { scoop_gc_collect() };
        }
    } else {
        // 多线程吞吐：在 worker 线程持续分配的同时，主线程周期性触发 GC。
        //
        // 注意：
        // - Immix backend 的 stop-the-world 为协作式：线程只有在进入 `scoop_alloc` 的 safepoint
        //   才会 park；因此这里避免使用会“长时间阻塞线程”的 barrier 同步。
        let stop = Arc::new(AtomicBool::new(false));
        let oom = Arc::new(AtomicBool::new(false));
        let total_allocs = Arc::new(AtomicU64::new(0));
        let total_bytes = Arc::new(AtomicU64::new(0));

        let mut handles = Vec::new();
        for _ in 0..threads {
            let stop = stop.clone();
            let oom = oom.clone();
            let total_allocs = total_allocs.clone();
            let total_bytes = total_bytes.clone();
            let object_size = args.object_size;

            handles.push(std::thread::spawn(move || unsafe {
                scoop_thread_register();

                let mut i: u64 = 0;
                while !stop.load(Ordering::Relaxed) && !oom.load(Ordering::Relaxed) {
                    let p = scoop_alloc(object_size);
                    if p.is_null() {
                        oom.store(true, Ordering::Relaxed);
                        break;
                    }

                    total_allocs.fetch_add(1, Ordering::Relaxed);
                    total_bytes.fetch_add(object_size, Ordering::Relaxed);

                    i += 1;
                    if (i % 1024) == 0 {
                        std::thread::yield_now();
                    }
                }

                scoop_thread_unregister();
            }));
        }

        let per_round_target = (threads as u64) * (batch as u64);
        let mut last_target: u64 = 0;
        for _ in 0..rounds {
            let target = last_target.saturating_add(per_round_target);
            while total_allocs.load(Ordering::Relaxed) < target && !oom.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            if oom.load(Ordering::Relaxed) {
                break;
            }

            unsafe { scoop_gc_collect() };
            last_target = target;
        }

        stop.store(true, Ordering::Relaxed);
        for h in handles {
            let _ = h.join();
        }

        allocations = total_allocs.load(Ordering::Relaxed);
        bytes = total_bytes.load(Ordering::Relaxed);
    }

    let elapsed = t0.elapsed();

    // 结束前再 collect 一次，尽量把 live 降到 0（避免影响碎片化指标）。
    unsafe { scoop_gc_collect() };
    let end_gc = read_gc_bytes();

    let secs = elapsed.as_secs_f64().max(1e-9);
    let allocs_per_sec = (allocations as f64) / secs;
    let bytes_per_sec = (bytes as f64) / secs;

    BenchResult {
        backend: format!("{:?}", GC_BACKEND),
        scenario: Scenario::Throughput,
        object_size: args.object_size,
        elapsed_ms: fmt_duration_ms(elapsed),
        allocations: Some(allocations),
        bytes: Some(bytes),
        allocs_per_sec: Some(allocs_per_sec),
        bytes_per_sec: Some(bytes_per_sec),
        pinned: None,
        start_gc,
        end_gc,
        params: BenchParams::Throughput {
            threads,
            rounds,
            batch,
        },
    }
}

fn run_fragmentation(args: &Args) -> BenchResult {
    let initial = args.initial.max(1);
    let pin_stride = args.pin_stride.max(1);

    let start_gc = read_gc_bytes();

    // 通过“稀疏存活（pinned）”把 live 对象分散到大量 blocks 中：
    // - baseline：对象逐个 malloc，GC 后 reserved≈live
    // - Immix（non-moving）：block 不能因为少量 live 对象而整体回收，reserved 可能远大于 live
    let mut pinned: Vec<*mut c_void> = Vec::new();
    pinned.reserve((initial / pin_stride).max(1) as usize);

    for i in 0..initial {
        let p = unsafe { scoop_alloc(args.object_size) };
        if p.is_null() {
            eprintln!("OOM：scoop_alloc(size={}) 返回 NULL（i={i}）", args.object_size);
            break;
        }

        if i % pin_stride == 0 {
            let ok = unsafe { scoop_pin(p) };
            if ok == 1 {
                pinned.push(p);
            } else {
                eprintln!("pin 失败：i={i}");
            }
        }
    }

    // 触发一次 GC，让未 pin 的对象变为 dead（形成 holes / 触发 sweep）。
    unsafe { scoop_gc_collect() };

    let end_gc = read_gc_bytes();
    let pinned_count = pinned.len() as u64;

    // 清理：unpin + collect，避免把 pinned roots 泄漏到进程结束（便于复用该 binary 做更多实验）。
    for p in pinned.drain(..) {
        let _ = unsafe { scoop_unpin(p) };
    }
    unsafe { scoop_gc_collect() };

    BenchResult {
        backend: format!("{:?}", GC_BACKEND),
        scenario: Scenario::Fragmentation,
        object_size: args.object_size,
        elapsed_ms: 0,
        allocations: None,
        bytes: None,
        allocs_per_sec: None,
        bytes_per_sec: None,
        pinned: Some(pinned_count),
        start_gc,
        end_gc,
        params: BenchParams::Fragmentation { initial, pin_stride },
    }
}

#[derive(Debug, Clone)]
enum BenchParams {
    Throughput { threads: u32, rounds: u32, batch: u32 },
    Fragmentation { initial: u32, pin_stride: u32 },
}

#[derive(Debug, Clone)]
struct BenchResult {
    backend: String,
    scenario: Scenario,
    object_size: u64,
    elapsed_ms: u128,

    allocations: Option<u64>,
    bytes: Option<u64>,
    allocs_per_sec: Option<f64>,
    bytes_per_sec: Option<f64>,
    pinned: Option<u64>,

    // (allocated, freed, live, reserved)
    start_gc: (u64, u64, u64, u64),
    end_gc: (u64, u64, u64, u64),

    params: BenchParams,
}

impl BenchResult {
    fn to_human_text(&self) -> String {
        let (a0, f0, l0, r0) = self.start_gc;
        let (a1, f1, l1, r1) = self.end_gc;

        let mut out = String::new();
        out.push_str(&format!(
            "backend={} scenario={} object_size={}\n",
            self.backend,
            self.scenario.as_str(),
            self.object_size
        ));

        match self.params {
            BenchParams::Throughput {
                threads,
                rounds,
                batch,
            } => {
                out.push_str(&format!(
                    "params: threads={} rounds={} batch={}\n",
                    threads, rounds, batch
                ));
            }
            BenchParams::Fragmentation { initial, pin_stride } => {
                out.push_str(&format!(
                    "params: initial={} pin_stride={} (pin ~ 1/{})\n",
                    initial, pin_stride, pin_stride
                ));
            }
        }

        if let Some(pinned) = self.pinned {
            out.push_str(&format!("pinned={}\n", pinned));
        }

        if let (Some(allocs), Some(bytes), Some(aps), Some(bps)) =
            (self.allocations, self.bytes, self.allocs_per_sec, self.bytes_per_sec)
        {
            out.push_str(&format!(
                "throughput: allocs={} bytes={} elapsed_ms={} allocs/s={:.2} bytes/s={:.2}\n",
                allocs, bytes, self.elapsed_ms, aps, bps
            ));
        }

        out.push_str(&format!(
            "gc_start: allocated={} freed={} live={} reserved={}\n",
            a0, f0, l0, r0
        ));
        out.push_str(&format!(
            "gc_end:   allocated={} freed={} live={} reserved={}\n",
            a1, f1, l1, r1
        ));

        if l1 > 0 {
            let ratio = (r1 as f64) / (l1 as f64);
            out.push_str(&format!("fragmentation_estimate: reserved/live={:.2}\n", ratio));
        }

        out
    }

    fn to_json_line(&self) -> String {
        // 说明：避免引入 serde 依赖；这里手写一个稳定的单行 JSON 便于记录/对比。
        let (a0, f0, l0, r0) = self.start_gc;
        let (a1, f1, l1, r1) = self.end_gc;

        let params_json = match self.params {
            BenchParams::Throughput {
                threads,
                rounds,
                batch,
            } => format!("{{\"threads\":{},\"rounds\":{},\"batch\":{}}}", threads, rounds, batch),
            BenchParams::Fragmentation { initial, pin_stride } => {
                format!("{{\"initial\":{},\"pin_stride\":{}}}", initial, pin_stride)
            }
        };

        let mut out = String::new();
        out.push_str("{");
        out.push_str(&format!("\"backend\":\"{}\",", self.backend));
        out.push_str(&format!("\"scenario\":\"{}\",", self.scenario.as_str()));
        out.push_str(&format!("\"object_size\":{},", self.object_size));
        out.push_str(&format!("\"elapsed_ms\":{},", self.elapsed_ms));

        if let Some(v) = self.allocations {
            out.push_str(&format!("\"allocations\":{},", v));
        }
        if let Some(v) = self.bytes {
            out.push_str(&format!("\"bytes\":{},", v));
        }
        if let Some(v) = self.allocs_per_sec {
            out.push_str(&format!("\"allocs_per_sec\":{},", v));
        }
        if let Some(v) = self.bytes_per_sec {
            out.push_str(&format!("\"bytes_per_sec\":{},", v));
        }
        if let Some(v) = self.pinned {
            out.push_str(&format!("\"pinned\":{},", v));
        }

        out.push_str(&format!(
            "\"gc_start\":{{\"allocated\":{},\"freed\":{},\"live\":{},\"reserved\":{}}},",
            a0, f0, l0, r0
        ));
        out.push_str(&format!(
            "\"gc_end\":{{\"allocated\":{},\"freed\":{},\"live\":{},\"reserved\":{}}},",
            a1, f1, l1, r1
        ));

        if l1 > 0 {
            let ratio = (r1 as f64) / (l1 as f64);
            out.push_str(&format!("\"fragmentation_reserved_over_live\":{},", ratio));
        }

        out.push_str(&format!("\"params\":{}", params_json));
        out.push_str("}\n");
        out
    }
}

fn emit_output(args: &Args, result: &BenchResult) {
    let text = if args.json {
        result.to_json_line()
    } else {
        result.to_human_text()
    };

    print!("{text}");

    if let Some(path) = &args.output {
        if let Err(err) = std::fs::write(path, &text) {
            eprintln!("写入失败：path={} err={}", path.display(), err);
        }
    }
}

fn main() {
    let args = parse_args();

    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        // 以尽量干净的状态开始：避免之前的分配影响 microbench 的 live/reserved 统计。
        scoop_gc_collect();
    }

    let result = match args.scenario {
        Scenario::Throughput => run_throughput(&args),
        Scenario::Fragmentation => run_fragmentation(&args),
    };

    emit_output(&args, &result);

    unsafe {
        // microbench 结束前再 collect，避免把对象留在 heap 链表影响后续同进程实验。
        scoop_gc_collect();
        scoop_thread_unregister();
    }
}
