extern int __popcountdi2(unsigned long long value);

int scoop_test_native_popcount(unsigned long long value) {
    return __popcountdi2(value);
}
