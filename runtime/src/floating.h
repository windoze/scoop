#ifndef SCOOP_RT_FLOATING_H
#define SCOOP_RT_FLOATING_H

#include <stdbool.h>

/* Called once when the current OS thread first attaches to the runtime. */
bool scoop_float_init_environment(void);

#endif /* SCOOP_RT_FLOATING_H */
