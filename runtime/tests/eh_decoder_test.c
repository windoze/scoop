#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eh_internal.h"

typedef struct TestBuffer {
    uint8_t bytes[512];
    size_t length;
} TestBuffer;

typedef struct TestCallSite {
    uint64_t start;
    uint64_t length;
    uint64_t landing_pad;
    uint64_t action;
} TestCallSite;

#define CHECK(condition)                                                       \
    do {                                                                       \
        if (!(condition)) {                                                    \
            fprintf(stderr, "eh_decoder_test:%d: check failed: %s\n",          \
                    __LINE__, #condition);                                     \
            exit(1);                                                           \
        }                                                                      \
    } while (0)

static void push_byte(TestBuffer *buffer, uint8_t byte) {
    CHECK(buffer->length < sizeof buffer->bytes);
    buffer->bytes[buffer->length++] = byte;
}

static void push_bytes(TestBuffer *buffer, const uint8_t *bytes,
                       size_t length) {
    CHECK(length <= sizeof buffer->bytes - buffer->length);
    if (length != 0) {
        memcpy(buffer->bytes + buffer->length, bytes, length);
    }
    buffer->length += length;
}

static void push_uleb128(TestBuffer *buffer, uint64_t value) {
    do {
        uint8_t byte = (uint8_t)(value & UINT64_C(0x7f));
        value >>= 7;
        if (value != 0) {
            byte |= UINT8_C(0x80);
        }
        push_byte(buffer, byte);
    } while (value != 0);
}

static TestBuffer encode_call_sites(const TestCallSite *sites, size_t count) {
    TestBuffer result = {0};
    for (size_t index = 0; index < count; index++) {
        push_uleb128(&result, sites[index].start);
        push_uleb128(&result, sites[index].length);
        push_uleb128(&result, sites[index].landing_pad);
        push_uleb128(&result, sites[index].action);
    }
    return result;
}

static TestBuffer make_lsda(const TestCallSite *sites, size_t site_count,
                            const uint8_t *actions, size_t action_length,
                            const uint8_t type_entry[4]) {
    TestBuffer call_sites = encode_call_sites(sites, site_count);
    TestBuffer result = {0};
    push_byte(&result, SCOOP_EH_DW_PE_OMIT);
    push_byte(&result, SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT);
    size_t type_offset_byte = result.length;
    push_byte(&result, 0);
    size_t after_type_offset = result.length;
    push_byte(&result, SCOOP_EH_DW_PE_ULEB128);
    push_uleb128(&result, call_sites.length);
    push_bytes(&result, call_sites.bytes, call_sites.length);
    push_bytes(&result, actions, action_length);
    push_bytes(&result, type_entry, 4);
    size_t type_delta = result.length - after_type_offset;
    CHECK(type_delta < UINT8_C(0x80));
    result.bytes[type_offset_byte] = (uint8_t)type_delta;
    return result;
}

static ScoopEhDecodeResult decode(const TestBuffer *buffer,
                                  uintptr_t region_start,
                                  uintptr_t instruction_pointer) {
    return scoop_eh_decode_lsda(
        (ScoopEhByteRange){.bytes = buffer->bytes, .length = buffer->length},
        region_start, instruction_pointer);
}

static void expect_result(ScoopEhDecodeResult result, ScoopEhDecodeKind kind,
                          uintptr_t landing_pad, uint32_t selector) {
    if (result.kind != kind || result.landing_pad != landing_pad ||
        result.selector != selector || result.reason != SCOOP_EH_REASON_NONE) {
        fprintf(stderr,
                "unexpected decode result: kind=%d landing=%#" PRIxPTR
                " selector=%" PRIu32 " reason=%s offset=%zu\n",
                (int)result.kind, result.landing_pad, result.selector,
                scoop_eh_decode_reason_name(result.reason),
                result.error_offset);
        exit(1);
    }
}

static void expect_failure(ScoopEhDecodeResult result, ScoopEhDecodeKind kind,
                           ScoopEhDecodeReason reason) {
    if (result.kind != kind || result.reason != reason) {
        fprintf(stderr,
                "unexpected decode failure: kind=%d reason=%s offset=%zu; "
                "expected kind=%d reason=%s\n",
                (int)result.kind, scoop_eh_decode_reason_name(result.reason),
                result.error_offset, (int)kind,
                scoop_eh_decode_reason_name(reason));
        exit(1);
    }
}

static void test_closed_results(void) {
    static const uint8_t null_type[4] = {0};
    static const uint8_t catch_action[] = {1, 0};
    const uintptr_t region = (uintptr_t)UINT64_C(0x1000);

    TestCallSite no_action_site = {0, 4, 0, 0};
    TestBuffer no_action = make_lsda(&no_action_site, 1, NULL, 0, null_type);
    expect_result(decode(&no_action, region, region + 1),
                  SCOOP_EH_DECODE_NO_ACTION, 0, 0);

    TestCallSite cleanup_site = {0, 4, 32, 0};
    TestBuffer cleanup = make_lsda(&cleanup_site, 1, NULL, 0, null_type);
    expect_result(decode(&cleanup, region, region + 1), SCOOP_EH_DECODE_CLEANUP,
                  region + 32, 0);

    TestCallSite catch_site = {0, 4, 48, 1};
    TestBuffer catch_all =
        make_lsda(&catch_site, 1, catch_action, sizeof catch_action, null_type);
    expect_result(decode(&catch_all, region, region + 1),
                  SCOOP_EH_DECODE_CATCH_ALL, region + 48, 1);
}

static void test_multiple_ranges_and_ip_boundaries(void) {
    static const uint8_t null_type[4] = {0};
    static const uint8_t catch_action[] = {1, 0};
    static const TestCallSite sites[] = {
        {0, 4, 16, 0},
        {8, 3, 24, 1},
        {20, 2, 0, 0},
    };
    const uintptr_t region = (uintptr_t)UINT64_C(0x2000);
    TestBuffer lsda =
        make_lsda(sites, 3, catch_action, sizeof catch_action, null_type);

    expect_result(decode(&lsda, region, region + 1), SCOOP_EH_DECODE_CLEANUP,
                  region + 16, 0);
    expect_result(decode(&lsda, region, region + 4), SCOOP_EH_DECODE_CLEANUP,
                  region + 16, 0);
    expect_result(decode(&lsda, region, region + 5), SCOOP_EH_DECODE_NO_ACTION,
                  0, 0);
    expect_result(decode(&lsda, region, region + 9), SCOOP_EH_DECODE_CATCH_ALL,
                  region + 24, 1);
    expect_result(decode(&lsda, region, region + 11), SCOOP_EH_DECODE_CATCH_ALL,
                  region + 24, 1);
    expect_result(decode(&lsda, region, region + 12), SCOOP_EH_DECODE_NO_ACTION,
                  0, 0);
    expect_result(decode(&lsda, region, region + 21), SCOOP_EH_DECODE_NO_ACTION,
                  0, 0);
    expect_result(decode(&lsda, region, region + 22), SCOOP_EH_DECODE_NO_ACTION,
                  0, 0);
}

static void test_cleanup_without_type_table(void) {
    const uintptr_t region = (uintptr_t)UINT64_C(0x2000);
    TestBuffer cleanup = {
        .bytes = {0xff, 0xff, 0x01, 8, 0, 5, 32, 0, 5, 3, 0, 0},
        .length = 12,
    };
    expect_result(decode(&cleanup, region, region + 5), SCOOP_EH_DECODE_CLEANUP,
                  region + 32, 0);
    expect_result(decode(&cleanup, region, region + 6),
                  SCOOP_EH_DECODE_NO_ACTION, 0, 0);
    cleanup.bytes[7] = 1;
    expect_failure(decode(&cleanup, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_ACTION_OUT_OF_RANGE);
    cleanup.bytes[7] = 0;
    cleanup.length--;
    expect_failure(decode(&cleanup, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);
}

static void test_multibyte_leb128(void) {
    static const uint8_t null_type[4] = {0};
    /* Non-canonical multi-byte encodings of SLEB128 1 and 0 are valid. */
    static const uint8_t catch_action[] = {0x81, 0x00, 0x80, 0x00};
    const uintptr_t region = (uintptr_t)UINT64_C(0x3000);
    TestCallSite site = {130, 130, 300, 1};
    TestBuffer lsda =
        make_lsda(&site, 1, catch_action, sizeof catch_action, null_type);
    CHECK(lsda.bytes[5] == UINT8_C(0x82));
    CHECK(lsda.bytes[6] == UINT8_C(0x01));
    expect_result(decode(&lsda, region, region + 131),
                  SCOOP_EH_DECODE_CATCH_ALL, region + 300, 1);
    expect_result(decode(&lsda, region, region + 260),
                  SCOOP_EH_DECODE_CATCH_ALL, region + 300, 1);
    expect_result(decode(&lsda, region, region + 261),
                  SCOOP_EH_DECODE_NO_ACTION, 0, 0);
}

static void test_truncation_and_bounded_range(void) {
    const uintptr_t region = (uintptr_t)UINT64_C(0x4000);
    static const uint8_t one_byte[] = {SCOOP_EH_DW_PE_OMIT};
    static const uint8_t truncated_uleb[] = {
        SCOOP_EH_DW_PE_OMIT,
        SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT,
        0x80,
    };
    expect_failure(
        scoop_eh_decode_lsda((ScoopEhByteRange){.bytes = NULL, .length = 0},
                             region, region + 1),
        SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_NULL_RANGE);
    expect_failure(
        scoop_eh_decode_lsda((ScoopEhByteRange){.bytes = one_byte, .length = 0},
                             region, region + 1),
        SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);
    expect_failure(
        scoop_eh_decode_lsda(
            (ScoopEhByteRange){.bytes = one_byte, .length = sizeof one_byte},
            region, region + 1),
        SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);
    expect_failure(scoop_eh_decode_lsda(
                       (ScoopEhByteRange){.bytes = truncated_uleb,
                                          .length = sizeof truncated_uleb},
                       region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);

    static const uint8_t null_type[4] = {0};
    static const uint8_t catch_action[] = {1, 0};
    TestCallSite site = {0, 4, 16, 1};
    TestBuffer valid =
        make_lsda(&site, 1, catch_action, sizeof catch_action, null_type);
    ScoopEhByteRange shortened = {
        .bytes = valid.bytes,
        .length = valid.length - 1,
    };
    expect_failure(scoop_eh_decode_lsda(shortened, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_TYPE_TABLE_OUT_OF_RANGE);

    TestBuffer table_truncated = valid;
    table_truncated.bytes[4] = UINT8_C(0x40);
    expect_failure(decode(&table_truncated, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);
}

static void test_integer_and_pointer_overflow(void) {
    const uintptr_t region = (uintptr_t)UINT64_C(0x5000);
    TestBuffer uleb_overflow = {0};
    push_byte(&uleb_overflow, SCOOP_EH_DW_PE_OMIT);
    push_byte(&uleb_overflow, SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT);
    for (unsigned index = 0; index < 9; index++) {
        push_byte(&uleb_overflow, UINT8_C(0x80));
    }
    push_byte(&uleb_overflow, UINT8_C(0x02));
    expect_failure(decode(&uleb_overflow, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_INTEGER_OVERFLOW);

    static const uint8_t null_type[4] = {0};
    TestCallSite overflowing_range = {UINT64_MAX, 1, 0, 0};
    TestBuffer range_lsda =
        make_lsda(&overflowing_range, 1, NULL, 0, null_type);
    expect_failure(decode(&range_lsda, 0, 1), SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_INTEGER_OVERFLOW);

    TestCallSite address_overflow = {8, 1, 0, 0};
    TestBuffer address_lsda =
        make_lsda(&address_overflow, 1, NULL, 0, null_type);
    expect_failure(decode(&address_lsda, UINTPTR_MAX - 4, UINTPTR_MAX - 3),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_POINTER_OVERFLOW);

    TestCallSite landing_overflow = {0, 1, 16, 0};
    TestBuffer landing_lsda =
        make_lsda(&landing_overflow, 1, NULL, 0, null_type);
    expect_failure(decode(&landing_lsda, UINTPTR_MAX - 8, UINTPTR_MAX - 7),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_POINTER_OVERFLOW);

    static const uint8_t sleb_overflow[] = {
        0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01, 0x00,
    };
    TestCallSite catch_site = {0, 1, 16, 1};
    TestBuffer action_lsda = make_lsda(&catch_site, 1, sleb_overflow,
                                       sizeof sleb_overflow, null_type);
    expect_failure(decode(&action_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_INTEGER_OVERFLOW);
}

static void test_unsupported_encodings(void) {
    static const uint8_t null_type[4] = {0};
    TestCallSite site = {0, 1, 16, 0};
    TestBuffer valid = make_lsda(&site, 1, NULL, 0, null_type);
    const uintptr_t region = (uintptr_t)UINT64_C(0x6000);

    TestBuffer bad = valid;
    bad.bytes[0] = 0;
    expect_failure(decode(&bad, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_LPSTART_ENCODING);
    bad = valid;
    bad.bytes[1] = 0x03;
    expect_failure(decode(&bad, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_TTYPE_ENCODING);
    bad = valid;
    bad.bytes[3] = 0x03;
    expect_failure(decode(&bad, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_CALL_SITE_ENCODING);
}

static void test_action_bounds_cycles_and_chains(void) {
    static const uint8_t null_type[4] = {0};
    const uintptr_t region = (uintptr_t)UINT64_C(0x7000);
    TestCallSite site = {0, 1, 16, 1};

    TestBuffer missing = make_lsda(&site, 1, NULL, 0, null_type);
    expect_failure(decode(&missing, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_ACTION_OUT_OF_RANGE);

    static const uint8_t truncated[] = {1};
    TestBuffer truncated_lsda =
        make_lsda(&site, 1, truncated, sizeof truncated, null_type);
    expect_failure(decode(&truncated_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TRUNCATED);

    /* The next displacement is relative to the start of its SLEB field. */
    static const uint8_t cycle[] = {1, 0x7f};
    TestBuffer cycle_lsda = make_lsda(&site, 1, cycle, sizeof cycle, null_type);
    expect_failure(decode(&cycle_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_ACTION_CYCLE);

    static const uint8_t chain[] = {1, 1, 1, 0};
    TestBuffer chain_lsda = make_lsda(&site, 1, chain, sizeof chain, null_type);
    expect_failure(decode(&chain_lsda, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_ACTION_CHAIN);

    TestCallSite offset_site = {0, 1, 16, 9};
    static const uint8_t action[] = {1, 0};
    TestBuffer offset_lsda =
        make_lsda(&offset_site, 1, action, sizeof action, null_type);
    expect_failure(decode(&offset_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_ACTION_OUT_OF_RANGE);
}

static void test_typed_catches_filters_and_type_entries(void) {
    static const uint8_t null_type[4] = {0};
    static const uint8_t nonnull_type[4] = {1, 0, 0, 0};
    const uintptr_t region = (uintptr_t)UINT64_C(0x8000);
    TestCallSite site = {0, 1, 16, 1};

    static const uint8_t typed_action[] = {2, 0};
    TestBuffer typed =
        make_lsda(&site, 1, typed_action, sizeof typed_action, null_type);
    expect_failure(decode(&typed, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH);

    static const uint8_t filter_action[] = {0x7f, 0};
    TestBuffer filter =
        make_lsda(&site, 1, filter_action, sizeof filter_action, null_type);
    expect_failure(decode(&filter, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_FILTER);

    static const uint8_t cleanup_action[] = {0, 0};
    TestBuffer encoded_cleanup =
        make_lsda(&site, 1, cleanup_action, sizeof cleanup_action, null_type);
    expect_failure(decode(&encoded_cleanup, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_ACTION_KIND);

    static const uint8_t catch_action[] = {1, 0};
    TestBuffer nonnull =
        make_lsda(&site, 1, catch_action, sizeof catch_action, nonnull_type);
    expect_failure(decode(&nonnull, region, region + 1),
                   SCOOP_EH_DECODE_UNSUPPORTED,
                   SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH);
}

static void test_ranges_landing_pads_and_table_order(void) {
    static const uint8_t null_type[4] = {0};
    static const uint8_t catch_action[] = {1, 0};
    const uintptr_t region = (uintptr_t)UINT64_C(0x9000);

    TestCallSite null_landing = {0, 1, 0, 1};
    TestBuffer null_landing_lsda = make_lsda(&null_landing, 1, catch_action,
                                             sizeof catch_action, null_type);
    expect_failure(decode(&null_landing_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_NULL_LANDING_PAD);

    TestCallSite empty = {0, 0, 0, 0};
    TestBuffer empty_lsda = make_lsda(&empty, 1, NULL, 0, null_type);
    expect_failure(decode(&empty_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_EMPTY_CALL_SITE_RANGE);

    static const TestCallSite overlapping[] = {
        {0, 4, 16, 0},
        {3, 2, 24, 0},
    };
    TestBuffer overlap_lsda = make_lsda(overlapping, 2, NULL, 0, null_type);
    expect_failure(decode(&overlap_lsda, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_OVERLAPPING_CALL_SITE_RANGES);

    TestCallSite valid_site = {0, 1, 16, 0};
    TestBuffer bad_order = make_lsda(&valid_site, 1, NULL, 0, null_type);
    bad_order.bytes[2] = 1;
    expect_failure(decode(&bad_order, region, region + 1),
                   SCOOP_EH_DECODE_MALFORMED, SCOOP_EH_REASON_TABLE_ORDER);

    TestBuffer valid = make_lsda(&valid_site, 1, NULL, 0, null_type);
    expect_failure(decode(&valid, region, 0), SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_POINTER_OVERFLOW);
    expect_failure(decode(&valid, region, region), SCOOP_EH_DECODE_MALFORMED,
                   SCOOP_EH_REASON_IP_BEFORE_REGION);
}

int main(void) {
    test_closed_results();
    test_cleanup_without_type_table();
    test_multiple_ranges_and_ip_boundaries();
    test_multibyte_leb128();
    test_truncation_and_bounded_range();
    test_integer_and_pointer_overflow();
    test_unsupported_encodings();
    test_action_bounds_cycles_and_chains();
    test_typed_catches_filters_and_type_entries();
    test_ranges_landing_pads_and_table_order();
    puts("EH decoder tests passed");
    return 0;
}
