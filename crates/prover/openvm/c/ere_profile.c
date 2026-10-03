/* Forwards the profile hooks to the callback that the ere host registers. */

#include <stdatomic.h>

#include "ere_profile.h"

typedef void (*EreProfileFn)(struct MeteringState* metering, void* trace_heights,
                             uint32_t event, uint64_t pc, uint64_t value);

void register_ere_profile(EreProfileFn fn);

/* Every run registers the same callback, so one global serves all threads. */
static _Atomic(EreProfileFn) g_ere_profile_fn;

void register_ere_profile(EreProfileFn fn) {
  atomic_store_explicit(&g_ere_profile_fn, fn, memory_order_relaxed);
}

/* An inlined hook would lose `preserve_most` in the calling block. */
__attribute__((preserve_most, noinline)) void ere_profile(
    struct MeteringState* metering, void* trace_heights, uint32_t event,
    uint64_t pc, uint64_t value) {
  atomic_load_explicit(&g_ere_profile_fn, memory_order_relaxed)(
      metering, trace_heights, event, pc, value);
}
