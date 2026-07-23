#include <algorithm>
#include <cmath>
#include <vector>

#include "bsp/esp-bsp.h"
#include "esp_check.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "quote_model.hpp"
#include "ui.hpp"
#include "network.hpp"

namespace {

constexpr const char *TAG = "tab5_stock";
StockDashboard g_dashboard;

bool request_detail(const char *symbol, void *) {
    if (stock_network_is_configured()) {
        return stock_network_request_detail(symbol);
    }
    g_dashboard.show_local_mock_detail(symbol);
    return true;
}

void mock_quote_task(void *) {
    std::vector<Quote> quotes = default_quotes();
    uint32_t step = 0;

    while (true) {
        vTaskDelay(pdMS_TO_TICKS(CONFIG_TAB5_STOCK_UI_REFRESH_MS));
        step++;

        for (size_t i = 0; i < quotes.size(); ++i) {
            double wave = std::sin(static_cast<double>(step + i * 3) / 8.0) * 0.35;
            double base = quotes[i].prev_close != 0.0 ? quotes[i].prev_close : quotes[i].last - quotes[i].change;
            quotes[i].last = base * (1.0 + wave / 100.0);
            quotes[i].change = quotes[i].last - base;
            quotes[i].change_pct = base == 0.0 ? 0.0 : (quotes[i].change / base) * 100.0;
            quotes[i].high = std::max(quotes[i].high, quotes[i].last);
            quotes[i].low = std::min(quotes[i].low, quotes[i].last);

            if (bsp_display_lock(0)) {
                g_dashboard.apply_quote(quotes[i]);
                bsp_display_unlock();
            }
        }
    }
}

void init_display() {
    bsp_display_cfg_t cfg = {
        .lvgl_port_cfg = ESP_LVGL_PORT_INIT_CONFIG(),
        .buffer_size = BSP_LCD_H_RES * 80,
        .double_buffer = true,
        .flags = {
            .buff_dma = false,
            .buff_spiram = true,
        },
    };
    cfg.lvgl_port_cfg.task_stack = 12 * 1024;
    cfg.lvgl_port_cfg.task_priority = 4;

    lv_display_t *display = bsp_display_start_with_config(&cfg);
    ESP_ERROR_CHECK(display == nullptr ? ESP_FAIL : ESP_OK);
    ESP_ERROR_CHECK(bsp_display_brightness_set(80));

    if (bsp_display_lock(0)) {
        g_dashboard.create(display);
        g_dashboard.set_detail_request_callback(request_detail, nullptr);
        g_dashboard.apply_snapshot(default_quotes());
        bsp_display_unlock();
    }
}

}  // namespace

extern "C" void app_main(void) {
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    init_display();
    if (stock_network_is_configured()) {
        ESP_LOGI(TAG, "starting WebSocket network mode");
        stock_network_start(&g_dashboard);
    } else {
        ESP_LOGI(TAG, "starting local mock mode");
        xTaskCreatePinnedToCore(mock_quote_task, "mock_quote", 4096, nullptr, 4, nullptr, 1);
    }
}
