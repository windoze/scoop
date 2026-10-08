#ifndef SCOOP_RT_STARTUP_INTERNAL_H
#define SCOOP_RT_STARTUP_INTERNAL_H

#include "../startup.h"

uint32_t scoop_startup_call_gateway(uint32_t (*gateway)(void));
uint32_t scoop_startup_call_root(ScoopRootEntryGatewayFnV1 gateway, int32_t argc,
                                 const char *const *argv, int32_t *out_exit_code);
_Noreturn void scoop_startup_fatal(const char *message);
_Noreturn void scoop_startup_report_failure(const ScoopStaticStorageDescriptorV1 *failure,
                                            const ScoopInitializationUnitDescriptorV1 *unit);

#endif
