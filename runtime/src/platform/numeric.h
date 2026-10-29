#ifndef SCOOP_PLATFORM_NUMERIC_H
#define SCOOP_PLATFORM_NUMERIC_H

/* Parse in the C numeric locale without changing the native caller's locale. */
float scoop_platform_parse_float(const char *text, char **end);
double scoop_platform_parse_double(const char *text, char **end);

#endif
