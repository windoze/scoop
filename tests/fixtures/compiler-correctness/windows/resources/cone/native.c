#ifndef SCOOP_RESOURCE_C_ONLY
#error native C flags must be applied to this translation unit
#endif

int resource_fixture_marker(void);

int resource_fixture_marker(void) { return 27; }
