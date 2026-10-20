#include "url.h"

void scoop_url_free(scoop_url *value) {
  if (!value)
    return;
  uriFreeUriMembersA(&value->uri);
  free(value->input);
  free(value->path);
  free(value->serialized);
  free(value);
}

scoop_url_result scoop_url_error(scoop_url *value, int code, int64_t offset) {
  scoop_url_free(value);
  return (scoop_url_result){NULL, code, offset};
}

scoop_url_result scoop_url_empty(void) {
  scoop_url *value = calloc(1, sizeof(*value));
  return (scoop_url_result){value, value ? 0 : URI_ERROR_MALLOC, -1};
}

scoop_url_result scoop_url_parse(const char *data, int64_t size) {
  scoop_url_result result = scoop_url_empty();
  if (result.code)
    return result;
  scoop_url *value = result.value;
  value->input = malloc((size_t)size + 1);
  if (!value->input)
    return scoop_url_error(value, URI_ERROR_MALLOC, -1);
  memcpy(value->input, data, (size_t)size);
  value->input[size] = 0;
  const char *error = NULL;
  int code = uriParseSingleUriExA(&value->uri, value->input,
                                  value->input + size, &error);
  if (code) {
    int64_t offset = error ? (int64_t)(error - value->input) : -1;
    return scoop_url_error(value, code, offset);
  }
  return result;
}

scoop_url_result scoop_url_resolve(const char *base, int64_t base_size,
                                   const char *reference,
                                   int64_t reference_size) {
  scoop_url_result left = scoop_url_parse(base, base_size);
  if (left.code)
    return left;
  scoop_url_result right = scoop_url_parse(reference, reference_size);
  if (right.code) {
    scoop_url_free(left.value);
    return right;
  }
  scoop_url_result result = scoop_url_empty();
  if (!result.code) {
    int code =
        uriAddBaseUriA(&result.value->uri, &right.value->uri, &left.value->uri);
    if (!code)
      code = uriMakeOwnerA(&result.value->uri);
    if (code)
      result = scoop_url_error(result.value, code, -1);
  }
  scoop_url_free(right.value);
  scoop_url_free(left.value);
  return result;
}

const char *scoop_url_version(void) { return uriBaseRuntimeVersionA(); }
