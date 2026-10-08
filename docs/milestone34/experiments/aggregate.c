/* Compile with the target C compiler to inspect its actual ABI carriers. */
struct Integers {
  unsigned long long left;
  unsigned long long right;
};

struct Mixed {
  double real;
  unsigned long long integer;
};

struct Floats {
  float first;
  float second;
};

struct Hfa {
  double a;
  double b;
  double c;
  double d;
};

struct Integers integers(struct Integers value) { return value; }
struct Mixed mixed(struct Mixed value) { return value; }
struct Floats floats(struct Floats value) { return value; }
struct Hfa hfa(struct Hfa value) { return value; }

extern struct Mixed exhausted(unsigned long long, unsigned long long,
                              unsigned long long, unsigned long long,
                              unsigned long long, unsigned long long,
                              struct Mixed);

struct Mixed call_exhausted(struct Mixed value) {
  return exhausted(1, 2, 3, 4, 5, 6, value);
}
