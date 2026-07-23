#include "network.hpp"

#include <cstdio>
#include <cstring>
#include <string>

#include "bsp/esp-bsp.h"
#include "esp_check.h"
#include "esp_event.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_websocket_client.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "quote_model.hpp"

namespace {

constexpr const char *TAG = "stock_network";
constexpr EventBits_t WIFI_CONNECTED_BIT = BIT0;
constexpr EventBits_t WIFI_FAILED_BIT = BIT1;

EventGroupHandle_t s_wifi_events = nullptr;
StockDashboard *s_dashboard = nullptr;
int s_retry_count = 0;
std::string s_backend_uri;
std::string s_ws_message_buffer;
esp_websocket_client_handle_t s_ws_client = nullptr;
bool s_ws_connected = false;
uint64_t s_next_detail_request_id = 1;

void with_lvgl_lock(void (*fn)(StockDashboard *dashboard, void *ctx), void *ctx) {
    if (s_dashboard == nullptr) {
        return;
    }
    if (bsp_display_lock(0)) {
        fn(s_dashboard, ctx);
        bsp_display_unlock();
    }
}

void update_status(StockDashboard *dashboard, void *ctx) {
    dashboard->set_connection_status(static_cast<const char *>(ctx));
}

void update_error(StockDashboard *dashboard, void *ctx) {
    dashboard->set_error(static_cast<const char *>(ctx));
}

void apply_message(StockDashboard *dashboard, void *ctx) {
    auto *message = static_cast<ParsedStreamMessage *>(ctx);
    if (!message->snapshot.empty()) {
        dashboard->apply_snapshot(message->snapshot);
    }
    if (message->quote.has_value()) {
        dashboard->apply_quote(*message->quote);
    }
    if (message->detail.has_value()) {
        dashboard->apply_detail(*message->detail);
    }
    if (message->detail_error.has_value()) {
        dashboard->apply_detail_error(message->detail_error->symbol.c_str(), message->detail_error->message.c_str());
    }
    if (message->status.has_value()) {
        dashboard->set_connection_status(message->status->c_str());
    }
    if (message->error.has_value()) {
        dashboard->set_error(message->error->c_str());
    }
}

void wifi_event_handler(void *, esp_event_base_t event_base, int32_t event_id, void *) {
    if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
        if (s_retry_count < 8) {
            s_retry_count++;
            esp_wifi_connect();
            with_lvgl_lock(update_status, const_cast<char *>("wifi retry"));
        } else {
            xEventGroupSetBits(s_wifi_events, WIFI_FAILED_BIT);
            with_lvgl_lock(update_error, const_cast<char *>("wifi failed"));
        }
    } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
        s_retry_count = 0;
        xEventGroupSetBits(s_wifi_events, WIFI_CONNECTED_BIT);
        with_lvgl_lock(update_status, const_cast<char *>("wifi connected"));
    }
}

void websocket_event_handler(void *, esp_event_base_t, int32_t event_id, void *event_data) {
    auto *data = static_cast<esp_websocket_event_data_t *>(event_data);
    switch (event_id) {
        case WEBSOCKET_EVENT_CONNECTED:
            s_ws_connected = true;
            with_lvgl_lock(update_status, const_cast<char *>("backend connected"));
            break;
        case WEBSOCKET_EVENT_DISCONNECTED:
            s_ws_connected = false;
            with_lvgl_lock(update_error, const_cast<char *>("backend offline"));
            break;
        case WEBSOCKET_EVENT_DATA: {
            if (data->op_code == 0x1 && data->data_ptr != nullptr && data->data_len > 0) {
                if (data->payload_offset == 0) {
                    s_ws_message_buffer.clear();
                    s_ws_message_buffer.reserve(data->payload_len > 0 ? data->payload_len : data->data_len);
                }
                s_ws_message_buffer.append(data->data_ptr, data->data_len);

                const bool complete = data->payload_len == 0 ||
                                      static_cast<int>(s_ws_message_buffer.size()) >= data->payload_len;
                if (complete) {
                    ParsedStreamMessage message =
                        parse_stream_message(s_ws_message_buffer.data(), s_ws_message_buffer.size());
                    with_lvgl_lock(apply_message, &message);
                    s_ws_message_buffer.clear();
                }
            }
            break;
        }
        case WEBSOCKET_EVENT_ERROR:
            s_ws_connected = false;
            with_lvgl_lock(update_error, const_cast<char *>("websocket error"));
            break;
        default:
            break;
    }
}

esp_err_t wifi_start() {
    s_wifi_events = xEventGroupCreate();
    ESP_RETURN_ON_FALSE(s_wifi_events != nullptr, ESP_ERR_NO_MEM, TAG, "failed to create event group");

    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(WIFI_EVENT, ESP_EVENT_ANY_ID, wifi_event_handler, nullptr, nullptr));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(IP_EVENT, IP_EVENT_STA_GOT_IP, wifi_event_handler, nullptr, nullptr));

    wifi_config_t wifi_config = {};
    std::strncpy(reinterpret_cast<char *>(wifi_config.sta.ssid), CONFIG_TAB5_STOCK_WIFI_SSID, sizeof(wifi_config.sta.ssid));
    std::strncpy(reinterpret_cast<char *>(wifi_config.sta.password), CONFIG_TAB5_STOCK_WIFI_PASSWORD, sizeof(wifi_config.sta.password));
    wifi_config.sta.threshold.authmode =
        std::strlen(CONFIG_TAB5_STOCK_WIFI_PASSWORD) == 0 ? WIFI_AUTH_OPEN : WIFI_AUTH_WPA2_PSK;

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    EventBits_t bits = xEventGroupWaitBits(
        s_wifi_events,
        WIFI_CONNECTED_BIT | WIFI_FAILED_BIT,
        pdFALSE,
        pdFALSE,
        pdMS_TO_TICKS(20000));

    return (bits & WIFI_CONNECTED_BIT) ? ESP_OK : ESP_FAIL;
}

}  // namespace

bool stock_network_is_configured() {
    return std::strlen(CONFIG_TAB5_STOCK_WIFI_SSID) > 0 && std::strlen(CONFIG_TAB5_STOCK_BACKEND_URI) > 0;
}

void stock_network_start(StockDashboard *dashboard) {
    s_dashboard = dashboard;
    if (!stock_network_is_configured()) {
        ESP_LOGW(TAG, "Wi-Fi/backend not configured; staying in mock mode");
        return;
    }

    if (wifi_start() != ESP_OK) {
        ESP_LOGE(TAG, "Wi-Fi connection failed");
        return;
    }

    s_backend_uri = CONFIG_TAB5_STOCK_BACKEND_URI;
    if (std::strlen(CONFIG_TAB5_STOCK_DEVICE_TOKEN) > 0) {
        s_backend_uri += s_backend_uri.find('?') == std::string::npos ? "?token=" : "&token=";
        s_backend_uri += CONFIG_TAB5_STOCK_DEVICE_TOKEN;
    }

    esp_websocket_client_config_t websocket_cfg = {};
    websocket_cfg.uri = s_backend_uri.c_str();
    websocket_cfg.reconnect_timeout_ms = 5000;
    websocket_cfg.network_timeout_ms = 5000;

    s_ws_client = esp_websocket_client_init(&websocket_cfg);
    if (s_ws_client == nullptr) {
        with_lvgl_lock(update_error, const_cast<char *>("websocket init failed"));
        return;
    }

    ESP_ERROR_CHECK(esp_websocket_register_events(s_ws_client, WEBSOCKET_EVENT_ANY, websocket_event_handler, nullptr));
    ESP_ERROR_CHECK(esp_websocket_client_start(s_ws_client));
}

bool stock_network_request_detail(const char *symbol) {
    if (symbol == nullptr || symbol[0] == '\0' || s_ws_client == nullptr || !s_ws_connected) {
        return false;
    }

    char payload[160];
    const uint64_t request_id = s_next_detail_request_id++;
    const int len = std::snprintf(
        payload,
        sizeof(payload),
        "{\"type\":\"detail_request\",\"request_id\":%llu,\"symbol\":\"%s\"}",
        static_cast<unsigned long long>(request_id),
        symbol);
    if (len <= 0 || len >= static_cast<int>(sizeof(payload))) {
        return false;
    }

    return esp_websocket_client_send_text(s_ws_client, payload, len, pdMS_TO_TICKS(100)) >= 0;
}
