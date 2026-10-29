final class Objects {
  record Particle(long x, long y, long z) {}
  static final class ParticleRef {
    final long x, y, z;
    ParticleRef(long x, long y, long z) {
      this.x = x;
      this.y = y;
      this.z = z;
    }
  }
  record Box(long a, long b) {}
  static Object escaped;

  static long valueRecords(int n, long seed) {
    Particle[] a = new Particle[n];
    for (int i = 0; i < n; i++) a[i] = new Particle(i + seed, i * 2L, i * 3L);
    for (int i = 0; i < n; i++) {
      Particle p = a[i];
      a[i] = new Particle(p.y + 1, p.z + 2, p.x + 3);
    }
    long sum = 0;
    for (int i = 0; i < n; i++) {
      Particle p = a[i];
      sum += p.x + p.y * 2 + p.z * 3;
    }
    escaped = a;
    return sum;
  }

  static long referenceRecords(int n, long seed) {
    ParticleRef[] a = new ParticleRef[n];
    for (int i = 0; i < n; i++) a[i] = new ParticleRef(i + seed, i * 2L, i * 3L);
    for (int i = 0; i < n; i++) {
      ParticleRef p = a[i];
      a[i] = new ParticleRef(p.y + 1, p.z + 2, p.x + 3);
    }
    long sum = 0;
    for (int i = 0; i < n; i++) {
      ParticleRef p = a[i];
      sum += p.x + p.y * 2 + p.z * 3;
    }
    escaped = a;
    return sum;
  }

  static long allocationRing(int n, long seed) {
    Box zero = new Box(0, 0);
    Box[] ring = new Box[1024];
    java.util.Arrays.fill(ring, zero);
    escaped = ring;
    long sum = 0;
    for (int i = 0; i < n; i++) {
      int slot = i & 1023;
      Box old = ring[slot];
      sum += old.a ^ old.b;
      ring[slot] = new Box(i, i + seed);
    }
    return sum + ring[(n - 1) & 1023].b;
  }

  record Tree(Tree left, Tree right, long value) {}
  static Tree makeTree(int depth, long seed) {
    if (depth == 0)
      return new Tree(null, null, seed & 1023);
    return new Tree(makeTree(depth - 1, seed + 1), makeTree(depth - 1, seed + 2), 0);
  }
  static long checkTree(Tree tree) {
    if (tree.left == null)
      return tree.value;
    return checkTree(tree.left) + checkTree(tree.right);
  }
  static long trees(int n, long seed) {
    long sum = 0;
    for (int i = 0; i < n; i++) {
      Tree root = makeTree(12, seed + i);
      escaped = root;
      sum += checkTree(root);
    }
    return sum;
  }

  interface Operation {
    long apply(long x);
  }
  record Add(long k) implements Operation {
    public long apply(long x) {
      return x + k;
    }
  }
  record Xor(long k) implements Operation {
    public long apply(long x) {
      return x ^ k;
    }
  }
  record Multiply(long k) implements Operation {
    public long apply(long x) {
      return x * k;
    }
  }
  record Shift(long k) implements Operation {
    public long apply(long x) {
      return (x >> 1) + k;
    }
  }
  static long dispatch(int n, long seed) {
    Operation[] ops = new Operation[64];
    for (int i = 0; i < 64; i++) {
      long k = i + seed;
      ops[i] = switch (i & 3) {
        case 0 -> new Add(k);
        case 1 -> new Xor(k);
        case 2 -> new Multiply(k);
        default -> new Shift(k);
      };
    }
    long x = seed;
    for (int i = 0; i < n; i++) x = ops[i & 63].apply(x) & 2147483647;
    return x;
  }
}
