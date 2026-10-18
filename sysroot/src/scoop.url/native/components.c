#include "url.h"

static scoop_url_text text(UriTextRangeA range) {
  return (scoop_url_text){
      range.first, range.first ? (int64_t)(range.afterLast - range.first) : 0};
}

scoop_url_text scoop_url_part(const scoop_url *value, int32_t part) {
  switch (part) {
  case 0:
    return text(value->uri.scheme);
  case 1:
    return text(value->uri.userInfo);
  case 2:
    return text(value->uri.hostText);
  case 3:
    return text(value->uri.portText);
  case 4:
    return (scoop_url_text){value->path, (int64_t)strlen(value->path)};
  case 5:
    return text(value->uri.query);
  case 6:
    return text(value->uri.fragment);
  default:
    return (scoop_url_text){value->serialized,
                            (int64_t)strlen(value->serialized)};
  }
}

int32_t scoop_url_host_kind(const scoop_url *value) {
  if (value->uri.hostData.ip4)
    return 1;
  if (value->uri.hostData.ip6)
    return 2;
  return value->uri.hostData.ipFuture.first ? 3 : 0;
}

scoop_url_text scoop_url_address(const scoop_url *value) {
  if (value->uri.hostData.ip4)
    return (scoop_url_text){(const char *)value->uri.hostData.ip4->data, 4};
  if (value->uri.hostData.ip6)
    return (scoop_url_text){(const char *)value->uri.hostData.ip6->data, 16};
  return (scoop_url_text){NULL, 0};
}

int32_t scoop_url_set(scoop_url *value, int32_t part, const char *data,
                      int64_t size) {
  const char *end = data ? data + size : NULL;
  switch (part) {
  case 0:
    return uriSetSchemeA(&value->uri, data, end);
  case 1:
    return uriSetUserInfoA(&value->uri, data, end);
  case 2:
    return uriSetHostAutoA(&value->uri, data, end);
  case 3:
    return uriSetPortTextA(&value->uri, data, end);
  case 4:
    return uriSetPathA(&value->uri, size ? data : NULL, size ? end : NULL);
  case 5:
    return uriSetQueryA(&value->uri, data, end);
  case 6:
    return uriSetFragmentA(&value->uri, data, end);
  default:
    return URI_ERROR_RANGE_INVALID;
  }
}
