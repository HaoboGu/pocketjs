#include "pocket_guest_quickjs.h"
#include <stdlib.h>

#if QJS_VERSION_MAJOR == 0 && QJS_VERSION_MINOR < 16
#error "The owned Rust guest requires QuickJS-NG 0.16.2 or newer"
#endif

struct quickjs_realm {
  JSRuntime *runtime;
  JSContext *context;
  pocket_guest_quickjs *binding;
  unsigned remaining;
};

static int interrupt(JSRuntime *runtime, void *opaque) {
  (void)runtime;
  struct quickjs_realm *realm = opaque;
  if (realm->remaining == 0)
    return 1;
  --realm->remaining;
  return 0;
}

extern void *pocket_c_malloc(size_t size);
extern void *pocket_c_calloc(size_t count, size_t size);
extern void *pocket_c_realloc(void *pointer, size_t size);
extern void pocket_c_free(void *pointer);
extern size_t pocket_c_usable_size(const void *pointer);
static void *guest_calloc(void *opaque, size_t count, size_t size) {
  (void)opaque;
  return pocket_c_calloc(count, size);
}
static void *guest_malloc(void *opaque, size_t size) {
  (void)opaque;
  return pocket_c_malloc(size);
}
static void guest_free(void *opaque, void *pointer) {
  (void)opaque;
  pocket_c_free(pointer);
}
static void *guest_realloc(void *opaque, void *pointer, size_t size) {
  (void)opaque;
  return pocket_c_realloc(pointer, size);
}
static const JSMallocFunctions allocator = {
    .js_calloc = guest_calloc,
    .js_malloc = guest_malloc,
    .js_free = guest_free,
    .js_realloc = guest_realloc,
    .js_malloc_usable_size = pocket_c_usable_size,
};

void pocket_quickjs_realm_destroy(void *opaque) {
  struct quickjs_realm *realm = opaque;
  if (realm->binding)
    pocket_guest_quickjs_destroy(realm->binding);
  if (realm->context)
    JS_FreeContext(realm->context);
  if (realm->runtime)
    JS_FreeRuntime(realm->runtime);
  pocket_c_free(realm);
}

void *pocket_quickjs_realm_create(size_t heap_limit, size_t stack_limit) {
  struct quickjs_realm *realm = pocket_c_calloc(1, sizeof *realm);
  if (!realm)
    return NULL;
  realm->runtime = JS_NewRuntime2(&allocator, NULL);
  if (!realm->runtime)
    goto failed;
  JS_SetMemoryLimit(realm->runtime, heap_limit);
  JS_SetMaxStackSize(realm->runtime, stack_limit);
  realm->context = JS_NewContext(realm->runtime);
  if (!realm->context)
    goto failed;
  realm->binding = pocket_guest_quickjs_attach(realm->context, POCKET_QJS_CANONICAL);
  if (!realm->binding)
    goto failed;
  JS_SetInterruptHandler(realm->runtime, interrupt, realm);
  return realm;
failed:
  pocket_quickjs_realm_destroy(realm);
  return NULL;
}

void *pocket_quickjs_realm_binding(void *opaque) {
  return ((struct quickjs_realm *)opaque)->binding;
}
void *pocket_quickjs_realm_context(void *opaque) {
  return ((struct quickjs_realm *)opaque)->context;
}
void pocket_quickjs_realm_budget(void *opaque, unsigned polls) {
  ((struct quickjs_realm *)opaque)->remaining = polls;
}
const char *pocket_quickjs_realm_error(void *opaque) {
  return pocket_guest_quickjs_error(((struct quickjs_realm *)opaque)->binding);
}
