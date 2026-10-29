int __popcountdi2(unsigned long long value) {
    int count = 0;
    while (value != 0) {
        count += (int)(value & 1);
        value >>= 1;
    }
    return count;
}
