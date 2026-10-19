extern int m54_optional_hook(void) __attribute__((weak));
extern const int m54_optional_data __attribute__((weak));

int m54_optional_value(void) {
  return (m54_optional_hook ? m54_optional_hook() : 7) +
         (&m54_optional_data ? m54_optional_data : 10);
}
