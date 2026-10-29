final class Numeric {
  static long mix(int n, long seed) {
    int x = (int) seed;
    for (int i = 0; i < n; i++) {
      x ^= x << 13;
      x ^= x >>> 17;
      x ^= x << 5;
    }
    return Integer.toUnsignedLong(x);
  }

  static long mandelbrot(int n, long seed) {
    long total = 0;
    for (int y = 0; y < n; y++) {
      for (int x = 0; x < n; x++) {
        double cr = x * 3.0 / n - 2.0 + seed * 0.000001;
        double ci = y * 2.0 / n - 1.0;
        double zr = 0.0, zi = 0.0;
        int step = 0;
        while (step < 64 && zr * zr + zi * zi <= 4.0) {
          double next = zr * zr - zi * zi + cr;
          zi = 2.0 * zr * zi + ci;
          zr = next;
          step++;
        }
        total += step;
      }
    }
    return total;
  }

  static long arrayTransform(int n, long seed) {
    double[] a = new double[n], b = new double[n], c = new double[n];
    for (int i = 0; i < n; i++) a[i] = ((i & 1023) + seed) * 0.25;
    for (int i = 0; i < n; i++) b[i] = (i & 255) * 0.5;
    for (int i = 0; i < n; i++) c[i] = a[i] * 1.25 + b[i];
    double sum = 0;
    for (int i = 0; i < n; i++) sum += c[i];
    return (long) (sum * 16.0);
  }

  static long branchScan(int n, long seed) {
    int[] a = new int[n];
    int state = (int) seed;
    for (int i = 0; i < n; i++) {
      state ^= state << 13;
      state ^= state >>> 17;
      state ^= state << 5;
      a[i] = state;
    }
    long sum = 0;
    for (int i = 0; i < n; i++) {
      int x = a[i];
      if ((x & 1) == 0)
        sum += x & 1023;
      else
        sum -= x & 1023;
    }
    return sum;
  }

  static long pointerChase(int n, long seed) {
    int[] next = new int[n];
    for (int i = 0; i < n; i++) next[i] = (int) ((i * 1664525L + 1013904223L) & (n - 1));
    int at = (int) (seed & (n - 1));
    long sum = 0;
    for (int i = 0; i < n * 8; i++) {
      at = next[at];
      sum += at;
    }
    return sum & 2147483647;
  }

  static void quicksort(int[] a, int low, int high) {
    int i = low, j = high, pivot = a[(low + high) / 2];
    while (i <= j) {
      while (a[i] < pivot) i++;
      while (a[j] > pivot) j--;
      if (i <= j) {
        int v = a[i];
        a[i] = a[j];
        a[j] = v;
        i++;
        j--;
      }
    }
    if (low < j)
      quicksort(a, low, j);
    if (i < high)
      quicksort(a, i, high);
  }

  static long sortValues(int n, long seed) {
    int[] a = new int[n];
    int state = (int) seed;
    for (int i = 0; i < n; i++) {
      state ^= state << 13;
      state ^= state >>> 17;
      state ^= state << 5;
      a[i] = state & 2147483647;
    }
    quicksort(a, 0, n - 1);
    long sum = 0;
    for (int i = 0; i < n; i++) sum += (long) a[i] * (i % 257);
    return sum;
  }
}
