idf_component_register(
    SRCS
        "app_main.cpp"
        "network.cpp"
        "quote_model.cpp"
        "ui.cpp"
    INCLUDE_DIRS
        "."
    REQUIRES
        esp_event
        esp_netif
        esp_wifi
        esp_websocket_client
        json
        m5stack_tab5
        nvs_flash
)

target_compile_options(${COMPONENT_LIB} PRIVATE -std=gnu++20)

