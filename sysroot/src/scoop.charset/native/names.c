#include "charset.h"

static const char *const extra_names[] = {
    "UTF8", "UTF16", "UTF16LE", "UTF16BE", "UTF32", "UTF32LE", "UTF32BE"};
static const char *const extra_targets[] = {"UTF-8",    "UTF-16", "UTF-16LE",
                                            "UTF-16BE", "UTF-32", "UTF-32LE",
                                            "UTF-32BE"};

char *scoop_charset_copy(const char *data, size_t size) {
  char *copy = malloc(size + 1);
  if (copy) {
    if (size)
      memcpy(copy, data, size);
    copy[size] = 0;
  }
  return copy;
}

static int portable(const char *name) {
  return *name && strcmp(name, "CHAR") && strcmp(name, "WCHAR_T") &&
         !strstr(name, "INTERNAL") && !strstr(name, "SWAPPED");
}

scoop_charset_text scoop_charset_name(const char *data, int64_t size) {
  char *name = scoop_charset_copy(data, (size_t)size);
  if (!name)
    return (scoop_charset_text){NULL, 0, CHARSET_MEMORY, ENOMEM};
  for (int64_t i = 0; i < size; i++) {
    unsigned char c = (unsigned char)name[i];
    if (c <= 32 || c >= 127 || c == '/') {
      free(name);
      return (scoop_charset_text){NULL, 0, CHARSET_UNKNOWN, 0};
    }
    if (c >= 'a' && c <= 'z')
      name[i] = (char)(c - 'a' + 'A');
  }
  if (!portable(name)) {
    free(name);
    return (scoop_charset_text){NULL, 0, CHARSET_UNKNOWN, 0};
  }
  const char *lookup = name;
  for (size_t i = 0; i < sizeof(extra_names) / sizeof(*extra_names); i++) {
    if (!strcmp(name, extra_names[i]))
      lookup = extra_targets[i];
  }
  const char *canonical = iconv_canonicalize(lookup);
  if (!portable(canonical)) {
    free(name);
    return (scoop_charset_text){NULL, 0, CHARSET_UNKNOWN, 0};
  }
  iconv_t probe = iconv_open("UTF-32LE", canonical);
  if (probe == (iconv_t)-1) {
    int error = errno;
    free(name);
    return (scoop_charset_text){
        NULL, 0, error == ENOMEM ? CHARSET_MEMORY : CHARSET_UNKNOWN, error};
  }
  iconv_close(probe);
  size_t length = strlen(canonical);
  char *output = scoop_charset_copy(canonical, length);
  free(name);
  return (scoop_charset_text){output, output ? (int64_t)length : 0,
                              output ? 0 : CHARSET_MEMORY, output ? 0 : ENOMEM};
}

static int append_names(unsigned int count, const char *const *names,
                        void *data) {
  scoop_charset_text *result = data;
  for (unsigned int i = 0; i < count; i++) {
    if (!portable(names[i]) || !portable(iconv_canonicalize(names[i])))
      continue;
    size_t length = strlen(names[i]);
    char *next = realloc(result->data, (size_t)result->size + length + 1);
    if (!next) {
      result->code = CHARSET_MEMORY;
      result->backend = ENOMEM;
      return 1;
    }
    result->data = next;
    memcpy(next + result->size, names[i], length);
    result->size += (int64_t)length;
    next[result->size++] = '\n';
  }
  return 0;
}

scoop_charset_text scoop_charset_names(void) {
  scoop_charset_text result = {0};
  iconvlist(append_names, &result);
  if (!result.code)
    append_names(sizeof(extra_names) / sizeof(*extra_names), extra_names,
                 &result);
  return result;
}

void scoop_charset_text_free(char *data) { free(data); }
int32_t scoop_charset_version(void) { return _LIBICONV_VERSION; }
