#ifndef ZNET_H
#define ZNET_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum ZNet_Button
#ifdef __cplusplus
  : uint64_t
#endif // __cplusplus
 {
    ZNet_Button_A = (1 << 0),
    ZNet_Button_B = (1 << 1),
    ZNet_Button_X = (1 << 2),
    ZNet_Button_Y = (1 << 3),
    ZNet_Button_Up = (1 << 4),
    ZNet_Button_Down = (1 << 5),
    ZNet_Button_Left = (1 << 6),
    ZNet_Button_Right = (1 << 7),
    ZNet_Button_Start = (1 << 8),
    ZNet_Button_Select = (1 << 9),
    ZNet_Button_L1 = (1 << 10),
    ZNet_Button_R1 = (1 << 11),
    ZNet_Button_L2 = (1 << 12),
    ZNet_Button_R2 = (1 << 13),
    ZNet_Button_L3 = (1 << 14),
    ZNet_Button_R3 = (1 << 15),
    ZNet_Button_L4 = (1 << 16),
    ZNet_Button_R4 = (1 << 17),
    ZNet_Button_LStick = (1 << 18),
    ZNet_Button_RStick = (1 << 19),
    ZNet_Button_Home = (1 << 20),
    ZNet_Button_Capture = (1 << 21),
};
#ifndef __cplusplus
typedef uint64_t ZNet_Button;
#endif // __cplusplus

typedef struct ZNet_Controller {
    uint64_t buttons;
    uint8_t left_stick_x;
    uint8_t left_stick_y;
    uint8_t right_stick_x;
    uint8_t right_stick_y;
    uint8_t l1_analog;
    uint8_t r1_analog;
    uint8_t l2_analog;
    uint8_t r2_analog;
} ZNet_Controller;

/**
 * Gyro values are degrees per second
 * Acceleration is in G (1G = 9.8m/s^2)
 */
typedef struct ZNet_Motion {
    /**
     * Negative = Pitch forward
     */
    float gyro_pitch;
    /**
     * Negative = Clockwise
     */
    float gyro_roll;
    /**
     * Negative = Clockwise
     */
    float gyro_yaw;
    /**
     * -1.0 = Controller is placed left grip down
     * 1.0  = Controller is placed right grip down
     */
    float accel_x;
    /**
     * -1.0 = Controller is placed face up
     * 1.0  = Controller is placed face down
     */
    float accel_y;
    /**
     * -1.0 = Controller is placed triggers down
     * 1.0  = Controller is placed grips down
     */
    float accel_z;
} ZNet_Motion;

typedef struct ZNet_Device {
    struct ZNet_Controller controller;
    struct ZNet_Motion motion;
} ZNet_Device;

typedef struct ZNet_Packet {
    uint8_t name[16];
    uint8_t num_devices;
    struct ZNet_Device devices[4];
} ZNet_Packet;

#endif /* ZNET_H */
