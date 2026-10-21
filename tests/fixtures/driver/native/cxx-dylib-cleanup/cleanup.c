static int cleanup_calls;

static void record_cleanup(int *value) { cleanup_calls += *value; }

extern void native_throw(int value);

void native_cleanup(int value) {
    int token __attribute__((cleanup(record_cleanup))) = value;
    native_throw(value);
}

int native_cleanup_count(void) { return cleanup_calls; }
