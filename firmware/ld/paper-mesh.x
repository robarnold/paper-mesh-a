/* The include order matches esp-hal's ESP32-S3 linkall.x.
 * Explicit chip script selection avoids dependence on include lookup order.
 */
INCLUDE "memory.x"
INCLUDE "alias.x"
INCLUDE "esp32s3-readonly.x"
INCLUDE "hal-defaults.x"
