#include <stdint.h>

typedef uint8_t boolean;

#define VCON_COMPONENTS(mname)    \
    mname(controller, Controller) \
    mname(motion,     Motion)     \
    mname(analog,     Analogs)    \
    mname(button,     Buttons)    \
    mname(touch_pad,  TouchPad)

typedef struct {
    void     *items;
    uintptr_t length;
} Slice;

typedef struct {
#define VCON_DVIEW(cname, ctype) Slice cname ## s;
VCON_COMPONENTS(VCON_DVIEW)
#undef VCON_DVIEW
} DeviceView;

boolean vcon_update(
    uint8_t     updated,
    DeviceView *input_devices,
    uintptr_t   num_input_devices,
    DeviceView *output_device
);