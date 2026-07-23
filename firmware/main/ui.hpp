#pragma once

#include <string>
#include <vector>

#include "lvgl.h"
#include "quote_model.hpp"

class StockDashboard {
   public:
    using DetailRequestCallback = bool (*)(const char *symbol, void *ctx);

    void create(lv_display_t *display);
    void set_detail_request_callback(DetailRequestCallback callback, void *ctx);
    void set_connection_status(const char *status);
    void set_error(const char *message);
    void apply_snapshot(const std::vector<Quote> &quotes);
    void apply_quote(const Quote &quote);
    void apply_detail(const QuoteDetail &detail);
    void apply_detail_error(const char *symbol, const char *message);
    void show_local_mock_detail(const char *symbol);
    void mark_offline();

   private:
    enum class ViewMode {
        List,
        Detail,
    };

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
    lv_obj_t *detail_title_label_ = nullptr;
    lv_obj_t *detail_subtitle_label_ = nullptr;
    lv_obj_t *detail_price_label_ = nullptr;
    lv_obj_t *detail_change_label_ = nullptr;
    lv_obj_t *detail_status_label_ = nullptr;
    lv_obj_t *detail_message_label_ = nullptr;
    lv_obj_t *detail_chart_ = nullptr;
    lv_obj_t *detail_volume_chart_ = nullptr;
    lv_chart_series_t *detail_price_series_ = nullptr;
    lv_chart_series_t *detail_avg_series_ = nullptr;
    lv_chart_series_t *detail_prev_close_series_ = nullptr;
    lv_chart_series_t *detail_volume_series_ = nullptr;
    std::vector<Row> rows_;
    std::vector<Quote> quotes_;
    std::vector<lv_obj_t *> detail_metric_values_;
    std::string connection_status_ = "starting";
    std::string selected_symbol_;
    ViewMode view_mode_ = ViewMode::List;
    DetailRequestCallback detail_request_callback_ = nullptr;
    void *detail_request_ctx_ = nullptr;

    void prepare_screen();
    void show_list_screen();
    void open_detail(const std::string &symbol);
    void create_header();
    void create_market_strip();
    void create_list();
    void create_detail_screen(const Quote *quote, const char *message);
    void update_detail_quote(const Quote &quote);
    void update_detail_chart(const QuoteDetail &detail);
    void rebuild_rows(const std::vector<Quote> &quotes);
    void update_row(Row &row, const Quote &quote);
    Row *find_row(const std::string &symbol);
    Quote *find_quote(const std::string &symbol);
    const Quote *find_quote(const std::string &symbol) const;
    void set_detail_metric(size_t index, const char *value);
    static void on_row_clicked(lv_event_t *event);
    static void on_back_clicked(lv_event_t *event);
};
