#include "lsda_reader.h"

typedef struct ScoopEhCallSite {
    uint64_t start;
    uint64_t end;
    uint64_t landing_pad_offset;
    uint64_t action;
    uintptr_t landing_pad;
} ScoopEhCallSite;

typedef struct ScoopEhActionRecord {
    int64_t filter;
    bool has_next;
    size_t next;
} ScoopEhActionRecord;

static bool scoop_eh_parse_call_site(ScoopEhByteRange lsda,
                                     size_t call_site_end, size_t *offset,
                                     uintptr_t region_start,
                                     ScoopEhCallSite *site,
                                     ScoopEhDecodeResult *error) {
    ScoopEhCursor cursor = {
        .range = {.bytes = lsda.bytes, .length = call_site_end},
        .offset = *offset,
    };
    uint64_t length = 0;
    ScoopEhReadStatus status = scoop_eh_read_uleb128(&cursor, &site->start);
    if (status == SCOOP_EH_READ_OK) {
        status = scoop_eh_read_uleb128(&cursor, &length);
    }
    if (status == SCOOP_EH_READ_OK) {
        status = scoop_eh_read_uleb128(&cursor, &site->landing_pad_offset);
    }
    if (status == SCOOP_EH_READ_OK) {
        status = scoop_eh_read_uleb128(&cursor, &site->action);
    }
    if (status != SCOOP_EH_READ_OK) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  scoop_eh_read_reason(status), cursor.offset);
        return false;
    }
    if (length == 0) {
        *error =
            scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                             SCOOP_EH_REASON_EMPTY_CALL_SITE_RANGE, *offset);
        return false;
    }
    if (length > UINT64_MAX - site->start) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_INTEGER_OVERFLOW, *offset);
        return false;
    }
    site->end = site->start + length;
    uintptr_t ignored = 0;
    if (!scoop_eh_checked_address_add(region_start, site->start, &ignored) ||
        !scoop_eh_checked_address_add(region_start, site->end, &ignored)) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_POINTER_OVERFLOW, *offset);
        return false;
    }
    site->landing_pad = 0;
    if (site->landing_pad_offset != 0 &&
        !scoop_eh_checked_address_add(region_start, site->landing_pad_offset,
                                      &site->landing_pad)) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_POINTER_OVERFLOW, *offset);
        return false;
    }
    *offset = cursor.offset;
    return true;
}

static bool scoop_eh_ranges_overlap(const ScoopEhCallSite *left,
                                    const ScoopEhCallSite *right) {
    return left->start < right->end && right->start < left->end;
}

static bool scoop_eh_validate_previous_ranges(
    ScoopEhByteRange lsda, size_t call_site_start, size_t call_site_end,
    size_t current_offset, uintptr_t region_start,
    const ScoopEhCallSite *current, ScoopEhDecodeResult *error) {
    size_t offset = call_site_start;
    while (offset < current_offset) {
        ScoopEhCallSite previous = {0};
        if (!scoop_eh_parse_call_site(lsda, call_site_end, &offset,
                                      region_start, &previous, error)) {
            return false;
        }
        if (offset > current_offset) {
            *error =
                scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                 SCOOP_EH_REASON_TABLE_ORDER, current_offset);
            return false;
        }
        if (scoop_eh_ranges_overlap(&previous, current)) {
            *error = scoop_eh_failure(
                SCOOP_EH_DECODE_MALFORMED,
                SCOOP_EH_REASON_OVERLAPPING_CALL_SITE_RANGES, current_offset);
            return false;
        }
    }
    if (offset != current_offset) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_TABLE_ORDER, current_offset);
        return false;
    }
    return true;
}

static bool scoop_eh_action_next_offset(size_t field_offset, int64_t delta,
                                        size_t action_start, size_t action_end,
                                        size_t *next) {
    if (delta > 0) {
        uint64_t distance = (uint64_t)delta;
        if (distance > (uint64_t)(action_end - field_offset)) {
            return false;
        }
        *next = field_offset + (size_t)distance;
    } else {
        uint64_t distance =
            delta == INT64_MIN ? (UINT64_C(1) << 63) : (uint64_t)(-delta);
        if (distance > (uint64_t)(field_offset - action_start)) {
            return false;
        }
        *next = field_offset - (size_t)distance;
    }
    return *next >= action_start && *next < action_end;
}

static bool scoop_eh_parse_action_record(ScoopEhByteRange lsda,
                                         size_t action_start, size_t action_end,
                                         size_t offset,
                                         ScoopEhActionRecord *record,
                                         ScoopEhDecodeResult *error) {
    if (offset < action_start || offset >= action_end) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_ACTION_OUT_OF_RANGE, offset);
        return false;
    }
    ScoopEhCursor cursor = {
        .range = {.bytes = lsda.bytes, .length = action_end},
        .offset = offset,
    };
    ScoopEhReadStatus status = scoop_eh_read_sleb128(&cursor, &record->filter);
    if (status != SCOOP_EH_READ_OK) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  scoop_eh_read_reason(status), cursor.offset);
        return false;
    }
    size_t next_field_offset = cursor.offset;
    int64_t next_delta = 0;
    status = scoop_eh_read_sleb128(&cursor, &next_delta);
    if (status != SCOOP_EH_READ_OK) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  scoop_eh_read_reason(status), cursor.offset);
        return false;
    }
    record->has_next = next_delta != 0;
    record->next = 0;
    if (record->has_next &&
        !scoop_eh_action_next_offset(next_field_offset, next_delta,
                                     action_start, action_end, &record->next)) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_ACTION_OUT_OF_RANGE,
                                  next_field_offset);
        return false;
    }
    return true;
}

static bool scoop_eh_action_chain_has_cycle(ScoopEhByteRange lsda,
                                            size_t action_start,
                                            size_t action_end, size_t first,
                                            bool *has_cycle,
                                            ScoopEhDecodeResult *error) {
    size_t slow = first;
    size_t fast = first;
    *has_cycle = false;
    for (;;) {
        ScoopEhActionRecord slow_record = {0};
        if (!scoop_eh_parse_action_record(lsda, action_start, action_end, slow,
                                          &slow_record, error)) {
            return false;
        }
        if (!slow_record.has_next) {
            return true;
        }
        slow = slow_record.next;

        ScoopEhActionRecord fast_record = {0};
        if (!scoop_eh_parse_action_record(lsda, action_start, action_end, fast,
                                          &fast_record, error)) {
            return false;
        }
        if (!fast_record.has_next) {
            return true;
        }
        fast = fast_record.next;
        if (!scoop_eh_parse_action_record(lsda, action_start, action_end, fast,
                                          &fast_record, error)) {
            return false;
        }
        if (!fast_record.has_next) {
            return true;
        }
        fast = fast_record.next;
        if (slow == fast) {
            *has_cycle = true;
            return true;
        }
    }
}

static bool scoop_eh_validate_catch_action(ScoopEhByteRange lsda,
                                           size_t action_start,
                                           size_t type_table_base,
                                           uint64_t action,
                                           ScoopEhDecodeResult *error) {
    const size_t encoded_type_size = 4;
    if (type_table_base < encoded_type_size) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_TYPE_TABLE_OUT_OF_RANGE,
                                  type_table_base);
        return false;
    }
    size_t type_entry = type_table_base - encoded_type_size;
    if (type_entry < action_start) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_TABLE_ORDER, type_entry);
        return false;
    }
    size_t action_index = 0;
    if (!scoop_eh_size_from_u64(action - 1, &action_index) ||
        action_index >= type_entry - action_start) {
        *error =
            scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                             SCOOP_EH_REASON_ACTION_OUT_OF_RANGE, action_start);
        return false;
    }
    size_t first = action_start + action_index;
    bool has_cycle = false;
    if (!scoop_eh_action_chain_has_cycle(lsda, action_start, type_entry, first,
                                         &has_cycle, error)) {
        return false;
    }
    if (has_cycle) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                  SCOOP_EH_REASON_ACTION_CYCLE, first);
        return false;
    }

    ScoopEhActionRecord record = {0};
    if (!scoop_eh_parse_action_record(lsda, action_start, type_entry, first,
                                      &record, error)) {
        return false;
    }
    if (record.filter < 0) {
        *error = scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                                  SCOOP_EH_REASON_UNSUPPORTED_FILTER, first);
        return false;
    }
    if (record.filter == 0) {
        *error =
            scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                             SCOOP_EH_REASON_UNSUPPORTED_ACTION_KIND, first);
        return false;
    }
    if (record.filter != 1) {
        *error =
            scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                             SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH, first);
        return false;
    }
    for (size_t index = type_entry; index < type_table_base; index++) {
        if (lsda.bytes[index] != 0) {
            *error = scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                                      SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH,
                                      type_entry);
            return false;
        }
    }
    if (record.has_next) {
        *error =
            scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                             SCOOP_EH_REASON_UNSUPPORTED_ACTION_CHAIN, first);
        return false;
    }
    return true;
}

ScoopEhDecodeResult scoop_eh_decode_lsda(ScoopEhByteRange lsda,
                                         uintptr_t region_start,
                                         uintptr_t instruction_pointer) {
    if (lsda.bytes == NULL) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_NULL_RANGE, 0);
    }
    ScoopEhCursor cursor = {.range = lsda, .offset = 0};
    uint8_t encoding = 0;
    ScoopEhReadStatus status = scoop_eh_read_u8(&cursor, &encoding);
    if (status != SCOOP_EH_READ_OK) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                scoop_eh_read_reason(status), cursor.offset);
    }
    if (encoding != SCOOP_EH_DW_PE_OMIT) {
        return scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                                SCOOP_EH_REASON_UNSUPPORTED_LPSTART_ENCODING,
                                0);
    }
    status = scoop_eh_read_u8(&cursor, &encoding);
    if (status != SCOOP_EH_READ_OK) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                scoop_eh_read_reason(status), cursor.offset);
    }
    bool has_type_table = encoding != SCOOP_EH_DW_PE_OMIT;
    if (has_type_table && encoding != SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT) {
        return scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                                SCOOP_EH_REASON_UNSUPPORTED_TTYPE_ENCODING, 1);
    }

    uint64_t type_table_delta = 0;
    status = has_type_table ? scoop_eh_read_uleb128(&cursor, &type_table_delta)
                            : SCOOP_EH_READ_OK;
    if (status != SCOOP_EH_READ_OK) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                scoop_eh_read_reason(status), cursor.offset);
    }
    size_t type_table_delta_size = 0;
    size_t type_table_base = lsda.length;
    if (has_type_table &&
        (!scoop_eh_size_from_u64(type_table_delta, &type_table_delta_size) ||
         !scoop_eh_checked_size_add(cursor.offset, type_table_delta_size,
                                    &type_table_base) ||
         type_table_base > lsda.length)) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_TYPE_TABLE_OUT_OF_RANGE,
                                cursor.offset);
    }

    size_t call_site_encoding_offset = cursor.offset;
    status = scoop_eh_read_u8(&cursor, &encoding);
    if (status != SCOOP_EH_READ_OK) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                scoop_eh_read_reason(status), cursor.offset);
    }
    if (encoding != SCOOP_EH_DW_PE_ULEB128) {
        return scoop_eh_failure(SCOOP_EH_DECODE_UNSUPPORTED,
                                SCOOP_EH_REASON_UNSUPPORTED_CALL_SITE_ENCODING,
                                call_site_encoding_offset);
    }
    uint64_t call_site_length = 0;
    status = scoop_eh_read_uleb128(&cursor, &call_site_length);
    if (status != SCOOP_EH_READ_OK) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                scoop_eh_read_reason(status), cursor.offset);
    }
    size_t call_site_length_size = 0;
    size_t call_site_end = 0;
    if (!scoop_eh_size_from_u64(call_site_length, &call_site_length_size) ||
        !scoop_eh_checked_size_add(cursor.offset, call_site_length_size,
                                   &call_site_end) ||
        call_site_end > lsda.length) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_TRUNCATED, cursor.offset);
    }
    if (type_table_base < call_site_end) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_TABLE_ORDER, type_table_base);
    }
    if (instruction_pointer == 0) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_POINTER_OVERFLOW, 0);
    }
    uintptr_t call_address = instruction_pointer - 1;
    if (call_address < region_start) {
        return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                SCOOP_EH_REASON_IP_BEFORE_REGION, 0);
    }
    uint64_t call_offset = (uint64_t)(call_address - region_start);

    size_t call_site_start = cursor.offset;
    size_t offset = call_site_start;
    ScoopEhDecodeResult selected =
        scoop_eh_result(SCOOP_EH_DECODE_NO_ACTION, 0, 0);
    while (offset < call_site_end) {
        size_t entry_offset = offset;
        ScoopEhCallSite site = {0};
        ScoopEhDecodeResult error = {0};
        if (!scoop_eh_parse_call_site(lsda, call_site_end, &offset,
                                      region_start, &site, &error)) {
            return error;
        }
        if (!scoop_eh_validate_previous_ranges(lsda, call_site_start,
                                               call_site_end, entry_offset,
                                               region_start, &site, &error)) {
            return error;
        }
        if (site.action != 0) {
            if (!has_type_table) {
                return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                        SCOOP_EH_REASON_ACTION_OUT_OF_RANGE,
                                        entry_offset);
            }
            if (site.landing_pad_offset == 0) {
                return scoop_eh_failure(SCOOP_EH_DECODE_MALFORMED,
                                        SCOOP_EH_REASON_NULL_LANDING_PAD,
                                        entry_offset);
            }
            if (!scoop_eh_validate_catch_action(lsda, call_site_end,
                                                type_table_base, site.action,
                                                &error)) {
                return error;
            }
        }
        if (call_offset < site.start || call_offset >= site.end) {
            continue;
        }
        if (site.landing_pad_offset == 0) {
            selected = scoop_eh_result(SCOOP_EH_DECODE_NO_ACTION, 0, 0);
        } else if (site.action == 0) {
            selected =
                scoop_eh_result(SCOOP_EH_DECODE_CLEANUP, site.landing_pad, 0);
        } else {
            selected =
                scoop_eh_result(SCOOP_EH_DECODE_CATCH_ALL, site.landing_pad, 1);
        }
    }
    return selected;
}

const char *scoop_eh_decode_reason_name(ScoopEhDecodeReason reason) {
    switch (reason) {
    case SCOOP_EH_REASON_NONE:
        return "none";
    case SCOOP_EH_REASON_NULL_RANGE:
        return "null byte range";
    case SCOOP_EH_REASON_TRUNCATED:
        return "truncated LSDA";
    case SCOOP_EH_REASON_INTEGER_OVERFLOW:
        return "integer overflow";
    case SCOOP_EH_REASON_POINTER_OVERFLOW:
        return "pointer overflow";
    case SCOOP_EH_REASON_IP_BEFORE_REGION:
        return "instruction pointer precedes function region";
    case SCOOP_EH_REASON_EMPTY_CALL_SITE_RANGE:
        return "empty call-site range";
    case SCOOP_EH_REASON_OVERLAPPING_CALL_SITE_RANGES:
        return "overlapping call-site ranges";
    case SCOOP_EH_REASON_TYPE_TABLE_OUT_OF_RANGE:
        return "type table is out of range";
    case SCOOP_EH_REASON_TABLE_ORDER:
        return "LSDA tables are out of order";
    case SCOOP_EH_REASON_NULL_LANDING_PAD:
        return "action has no landing pad";
    case SCOOP_EH_REASON_ACTION_OUT_OF_RANGE:
        return "action is out of range";
    case SCOOP_EH_REASON_ACTION_CYCLE:
        return "action chain contains a cycle";
    case SCOOP_EH_REASON_UNSUPPORTED_LPSTART_ENCODING:
        return "unsupported LPStart encoding";
    case SCOOP_EH_REASON_UNSUPPORTED_TTYPE_ENCODING:
        return "unsupported type-table encoding";
    case SCOOP_EH_REASON_UNSUPPORTED_CALL_SITE_ENCODING:
        return "unsupported call-site encoding";
    case SCOOP_EH_REASON_UNSUPPORTED_ACTION_CHAIN:
        return "unsupported action chain";
    case SCOOP_EH_REASON_UNSUPPORTED_ACTION_KIND:
        return "unsupported action kind";
    case SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH:
        return "unsupported typed catch";
    case SCOOP_EH_REASON_UNSUPPORTED_FILTER:
        return "unsupported negative filter";
    }
    return "unknown decode reason";
}
