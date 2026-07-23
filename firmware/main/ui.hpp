#pragma once

#include <string>
#include <vector>

#include "lvgl.h"
#include "quote_model.hpp"

class StockDashboard {
   public:
    void create(lv_display_t *display);
    void set_connection_status(const char *status);
    void set_error(const char *message);
    void apply_snapshot(const std::vector<Quote> &quotes);
    void apply_quote(const Quote &quote);
    void mark_offline();

   private:
    struct Row {
        std::string symbol;
        lv_obj_t *container = nullptr;
        lv_obj_t *symbol_label = nullptr;
        lv_obj_t *name_label = nullptr;
        lv_obj_t *last_label = nullptr;
        lv_obj_t *change_label = nullptr;
        lv_obj_t *turnover_label = nullptr;
        lv_obj_t *status_label = nullptr;
    };

    lv_obj_t *screen_ = nullptr;
    lv_obj_t *time_label_ = nullptr;
    lv_obj_t *status_label_ = nullptr;
    lv_obj_t *list_ = nullptr;
    std::vector<Row> rows_;
    std::vector<Quote> quotes_;

    void create_header();
    void create_market_strip();
    void create_list();
    void rebuild_rows(const std::vector<Quote> &quotes);
    void update_row(Row &row, const Quote &quote);
    Row *find_row(const std::string &symbol);
};
