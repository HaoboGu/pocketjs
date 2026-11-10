#include "pocket_quickjs_clock.h"
/* Link-only fixture. Hardware programs supply their monotonic and Unix clocks. */
int pocket_qjs_clock_gettime(clockid_t clock, struct timespec *value) {
    (void)clock; value->tv_sec = 0; value->tv_nsec = 0; return 0;
}
int pocket_qjs_gettimeofday(struct timeval *value, void *zone) {
    (void)zone; value->tv_sec = 0; value->tv_usec = 0; return 0;
}
