#ifndef POCKET_QUICKJS_CLOCK_H
#define POCKET_QUICKJS_CLOCK_H
#include <sys/types.h>
#include <time.h>
#include <sys/time.h>
/* The program supplies monotonic and Unix time for its engine build. */
#ifndef CLOCK_MONOTONIC
#define CLOCK_MONOTONIC 1
#endif
int pocket_qjs_clock_gettime(clockid_t clock, struct timespec *value);
int pocket_qjs_gettimeofday(struct timeval *value, void *zone);
#endif
