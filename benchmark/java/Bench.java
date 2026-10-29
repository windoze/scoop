public final class Bench {
  static long config(String name, long fallback) {
    String value = System.getenv(name);
    return value == null ? fallback : Long.parseLong(value);
  }

  static long kernel(int kind, int n, long seed) throws InterruptedException {
    return switch (kind) {
      case 0 -> Numeric.mix(n, seed);
      case 1 -> Numeric.mandelbrot(n, seed);
      case 2 -> Numeric.arrayTransform(n, seed);
      case 3 -> Numeric.branchScan(n, seed);
      case 4 -> Numeric.pointerChase(n, seed);
      case 5 -> Numeric.sortValues(n, seed);
      case 6 -> Objects.valueRecords(n, seed);
      case 7 -> Objects.referenceRecords(n, seed);
      case 8 -> Objects.allocationRing(n, seed);
      case 9 -> Objects.trees(n, seed);
      case 10 -> Objects.dispatch(n, seed);
      case 11 -> Libraries.growList(n, seed);
      case 12 -> Libraries.hashMap(n, seed);
      case 13 -> Libraries.decimal(n, seed);
      case 14 -> Libraries.buildString(n, seed);
      case 15 -> Libraries.unicode(n, seed);
      case 16 -> Libraries.parseIntegers(n, seed);
      case 17 -> Libraries.parallel(n, seed, 1);
      case 18 -> Libraries.parallel(n, seed, 4);
      case 19 -> Libraries.parallel(n, seed, 8);
      default -> throw new IllegalArgumentException("unknown case");
    };
  }

  public static void main(String[] args) throws InterruptedException {
    int kind = (int) config("BENCH_CASE", 0);
    if (kind < 0) {
      System.out.println("ready");
      return;
    }
    int n = (int) config("BENCH_N", 1000);
    long seed = config("BENCH_SEED", 42);
    for (long i = 0; i < 8; i++) {
      System.out.printf(
          "{\"type\":\"check\",\"seed\":%d,\"value\":%d}%n", seed + i, kernel(kind, n, seed + i));
    }
    for (long sample = -1; sample < config("BENCH_SAMPLES", 5); sample++) {
      long duration =
          config(sample < 0 ? "BENCH_WARMUP_MS" : "BENCH_SAMPLE_MS", sample < 0 ? 500 : 200)
          * 1000000;
      long start = System.nanoTime();
      long calls = 0;
      long checksum = 0;
      while (System.nanoTime() - start < duration) {
        checksum ^= kernel(kind, n, seed + (calls & 7));
        calls++;
      }
      long elapsed = System.nanoTime() - start;
      System.out.printf(
          "{\"type\":\"sample\",\"index\":%d,\"elapsed_ns\":%d,\"calls\":%d,\"checksum\":%d}%n",
          sample, elapsed, calls, checksum);
    }
  }
}
