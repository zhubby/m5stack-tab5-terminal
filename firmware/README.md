# Tab5 firmware

ESP-IDF firmware for the M5Stack Tab5 LVGL stock dashboard.

## Build

Install ESP-IDF with ESP32-P4 support, then run:

```bash
idf.py set-target esp32p4
idf.py build
idf.py -p /dev/ttyACM0 flash monitor
```

The project uses managed components:

- `espressif/m5stack_tab5`
- `espressif/esp_lvgl_port`
- `espressif/esp_websocket_client`

## Runtime modes

If `CONFIG_TAB5_STOCK_WIFI_SSID` is empty, the device runs local mock quotes so the display, touch, scroll, color, and refresh behavior can be validated without a backend.

When Wi-Fi and backend URI are configured in `idf.py menuconfig`, the firmware connects to:

```text
ws://<backend-host>:8080/v1/quotes/stream
```

All LVGL calls are protected with `bsp_display_lock()` and `bsp_display_unlock()` as required by the Tab5 BSP.

## ESP32-P4 Wi-Fi note

ESP32-P4 does not include native Wi-Fi. Network mode requires the Tab5 wireless module path supported by your ESP-IDF/BSP setup, typically ESP32-P4 Wi-Fi expansion / hosted remote Wi-Fi. Leave Wi-Fi SSID empty for the default mock UI mode until that board-level path is enabled and verified on hardware.
