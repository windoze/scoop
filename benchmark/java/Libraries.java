import java.util.ArrayList;
import java.util.HashMap;

final class Libraries {
  static long growList(int n, long seed) {
    ArrayList<Long> list = new ArrayList<>();
    for (int i = 0; i < n; i++) list.add(i + seed);
    long sum = 0;
    for (int i = 0; i < n; i++) sum += list.get(i);
    Objects.escaped = list;
    return sum;
  }

  static long hashMap(int n, long seed) {
    HashMap<Long, Long> map = new HashMap<>();
    long sum = 0;
    for (int i = 0; i < n; i++) {
      long key = (i * 17L + seed) & 65535;
      map.put(key, map.getOrDefault(key, 0L) + 1);
      sum += map.get(key);
    }
    Objects.escaped = map;
    return sum;
  }

  static long decimal(int n, long seed) {
    String[] ring = new String[256];
    Objects.escaped = ring;
    int x = (int) seed;
    long sum = 0;
    for (int i = 0; i < n; i++) {
      x = x * 1664525 + 1013904223;
      String text = Long.toString(Integer.toUnsignedLong(x));
      ring[i & 255] = text;
      sum += text.length();
    }
    return sum + ring[(n - 1) & 255].charAt(0);
  }

  static long buildString(int n, long seed) {
    StringBuilder builder = new StringBuilder();
    for (int i = 0; i < n; i++) {
      builder.append(Long.toString(i ^ seed));
      builder.append(",");
    }
    String text = builder.toString();
    Objects.escaped = text;
    return text.length() + text.charAt(0);
  }

  static long unicode(int n, long seed) {
    StringBuilder builder = new StringBuilder();
    for (int i = 0; i < 512; i++) builder.append("abé雪🙂z");
    builder.append(Long.toString(seed));
    String text = builder.toString();
    long sum = 0;
    for (int i = 0; i < n; i++) {
      for (int at = 0; at < text.length();) {
        int c = text.codePointAt(at);
        sum += c;
        at += Character.charCount(c);
      }
    }
    return sum;
  }

  static long parseIntegers(int n, long seed) {
    String[] texts = new String[256];
    for (int i = 0; i < 256; i++) texts[i] = Long.toString(i * 1234567L + seed);
    long sum = 0;
    for (int i = 0; i < n; i++) sum += Long.parseLong(texts[i & 255]);
    return sum;
  }

  static long parallel(int n, long seed, int workers) throws InterruptedException {
    long[] results = new long[8];
    Thread[] threads = new Thread[workers];
    for (int worker = 0; worker < workers; worker++) {
      final int first = worker;
      threads[worker] = new Thread(() -> {
        for (int shard = first; shard < 8; shard += workers)
          results[shard] = Numeric.mix(n / 8, seed + shard);
      });
      threads[worker].start();
    }
    for (Thread thread : threads) thread.join();
    long sum = 0;
    for (long value : results) sum += value;
    return sum;
  }
}
