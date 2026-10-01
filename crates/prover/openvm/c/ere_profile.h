/* Profile hooks of the generated code that call the ere host, see
 * https://github.com/han0110/openvm/commit/f73d411c192153cc5f4dc0f4794944605ec6dc9e */

#ifndef ERE_PROFILE_H
#define ERE_PROFILE_H

#include <stdint.h>

struct MeteringState;

extern __attribute__((preserve_most)) void ere_profile(
    struct MeteringState* metering, void* trace_heights, uint32_t event,
    uint64_t pc, uint64_t value);

/* A jump passes the trace heights, so that the block stores them before the
 * host reads them. */
#undef RV_PROFILE_JUMP
#undef RV_PROFILE_SP
#undef RV_PROFILE_EXIT
#undef RV_PROFILE_CHECK
#define RV_PROFILE_JUMP(pc, target) \
  ere_profile(&state->mode_state, trace_heights, 0, pc, target)
#define RV_PROFILE_SP(pc, value) \
  ere_profile(&state->mode_state, nullptr, 1, pc, value)
#define RV_PROFILE_EXIT() ere_profile(&state->mode_state, nullptr, 2, 0, 0)
#define RV_PROFILE_CHECK(end) \
  ere_profile(&state->mode_state, nullptr, 3 + (end), 0, 0)

#endif /* ERE_PROFILE_H */
