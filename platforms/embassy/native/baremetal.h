#ifndef POCKET_EMBASSY_BAREMETAL_H
#define POCKET_EMBASSY_BAREMETAL_H
#include <sys/types.h>
#include <time.h>
#include <sys/time.h>
/* Private clock calls do not use the board's POSIX clock namespace. */
#ifndef CLOCK_MONOTONIC
#define CLOCK_MONOTONIC 1
#endif
int pocket_embassy_clock_gettime(clockid_t clock, struct timespec *value);
int pocket_embassy_gettimeofday(struct timeval *value, void *zone);
#endif
