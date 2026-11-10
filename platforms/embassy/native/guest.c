#include "pocket_guest_quickjs.h"
#include <stdlib.h>

#if QJS_VERSION_MAJOR == 0 && QJS_VERSION_MINOR < 16
#error "pocket-embassy requires QuickJS-NG 0.16.2 or newer"
#endif

struct embassy_guest {
    JSRuntime *runtime;
    JSContext *context;
    pocket_guest_quickjs *binding;
    unsigned remaining;
};

static int interrupt(JSRuntime *runtime, void *opaque) {
    (void)runtime;
    struct embassy_guest *guest = opaque;
    if (guest->remaining == 0) return 1;
    guest->remaining--;
    return 0;
}
extern size_t pocket_c_usable_size(const void *pointer);
static void *guest_calloc(void *opaque, size_t count, size_t size) {
    (void)opaque; return calloc(count, size);
}
static void *guest_malloc(void *opaque, size_t size) {
    (void)opaque; return malloc(size);
}
static void guest_free(void *opaque, void *pointer) {
    (void)opaque; free(pointer);
}
static void *guest_realloc(void *opaque, void *pointer, size_t size) {
    (void)opaque; return realloc(pointer, size);
}
static const JSMallocFunctions allocator = {
    guest_calloc, guest_malloc, guest_free, guest_realloc, pocket_c_usable_size
};
void *pocket_embassy_guest_create(size_t heap_limit, size_t stack_limit) {
    struct embassy_guest *guest = calloc(1, sizeof *guest);
    if (!guest) return NULL;
    guest->runtime = JS_NewRuntime2(&allocator, NULL);
    if (!guest->runtime) { free(guest); return NULL; }
    JS_SetMemoryLimit(guest->runtime, heap_limit);
    JS_SetMaxStackSize(guest->runtime, stack_limit);
    guest->context = JS_NewContext(guest->runtime);
    if (!guest->context) { JS_FreeRuntime(guest->runtime); free(guest); return NULL; }
    guest->binding = pocket_guest_quickjs_attach(guest->context, POCKET_QJS_CANONICAL);
    if (!guest->binding) {
        JS_FreeContext(guest->context); JS_FreeRuntime(guest->runtime); free(guest); return NULL;
    }
    JS_SetInterruptHandler(guest->runtime, interrupt, guest);
    return guest;
}
void *pocket_embassy_guest_binding(void *opaque) {
    return ((struct embassy_guest *)opaque)->binding;
}
void *pocket_embassy_guest_context(void *opaque) {
    return ((struct embassy_guest *)opaque)->context;
}
void pocket_embassy_guest_budget(void *opaque, unsigned polls) {
    ((struct embassy_guest *)opaque)->remaining = polls;
}
const char *pocket_embassy_guest_error(void *opaque) {
    return pocket_guest_quickjs_error(((struct embassy_guest *)opaque)->binding);
}
void pocket_embassy_guest_destroy(void *opaque) {
    struct embassy_guest *guest = opaque;
    pocket_guest_quickjs_destroy(guest->binding);
    JS_FreeContext(guest->context);
    JS_FreeRuntime(guest->runtime);
    free(guest);
}

#ifdef POCKET_EMBASSY_BARE_METAL
#include <time.h>
#include <sys/time.h>
#include <errno.h>
extern uint64_t pocket_embassy_uptime_us(void);
int clock_gettime(clockid_t clock, struct timespec *value) {
    if (clock != CLOCK_MONOTONIC) { errno = EINVAL; return -1; }
    uint64_t micros = pocket_embassy_uptime_us();
    value->tv_sec = micros / 1000000;
    value->tv_nsec = (micros % 1000000) * 1000;
    return 0;
}
int gettimeofday(struct timeval *value, void *zone) {
    (void)zone;
    uint64_t micros = pocket_embassy_uptime_us();
    value->tv_sec = micros / 1000000;
    value->tv_usec = micros % 1000000;
    return 0;
}
#endif
