extern "C" void native_cleanup(int value);
extern "C" int native_cleanup_count(void);

extern "C" void native_throw(int value) { throw value; }

extern "C" int native_answer(int value) {
    try {
        native_cleanup(value);
    } catch (int caught) {
        return caught + native_cleanup_count();
    }
    return -1;
}
