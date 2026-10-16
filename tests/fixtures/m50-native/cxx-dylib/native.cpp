template <typename T> T doubled(T value) { return value + value; }

struct NativeError {
    int value;
};

extern "C" int native_answer(int value) {
    try {
        throw NativeError{value};
    } catch (const NativeError &error) {
        return doubled(error.value);
    }
}
