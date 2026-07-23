#include "ui.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>

namespace {

constexpr lv_color_t kBg = lv_color_hex(0x101418);
constexpr lv_color_t kPanel = lv_color_hex(0x171d23);
constexpr lv_color_t kLine = lv_color_hex(0x28313a);
constexpr lv_color_t kText = lv_color_hex(0xf4f7fb);
constexpr lv_color_t kMuted = lv_color_hex(0x92a0ad);
constexpr lv_color_t kUp = lv_color_hex(0xff4d5a);
constexpr lv_color_t kDown = lv_color_hex(0x19b56b);
constexpr lv_color_t kWarn = lv_color_hex(0xffc857);
constexpr lv_color_t kAvg = lv_color_hex(0xf2b84b);

void set_label(lv_obj_t *label, const char *text) {
    if (label != nullptr) {
        lv_label_set_text(label, text);
    }
}

void format_price(char *buffer, size_t len, double value) {
    std::snprintf(buffer, len, "%.2f", value);
}

void format_change(char *buffer, size_t len, const Quote &quote) {
    std::snprintf(buffer, len, "%+.2f  %+.2f%%", quote.change, quote.change_pct);
}

void format_turnover(char *buffer, size_t len, double turnover) {
    if (turnover >= 100000000.0) {
        std::snprintf(buffer, len, "%.1f亿", turnover / 100000000.0);
    } else if (turnover >= 10000.0) {
        std::snprintf(buffer, len, "%.1f万", turnover / 10000.0);
    } else {
        std::snprintf(buffer, len, "%.0f", turnover);
    }
}

void format_volume(char *buffer, size_t len, uint64_t volume) {
    if (volume >= 100000000ULL) {
        std::snprintf(buffer, len, "%.1f亿", static_cast<double>(volume) / 100000000.0);
    } else if (volume >= 10000ULL) {
        std::snprintf(buffer, len, "%.1f万", static_cast<double>(volume) / 10000.0);
    } else {
        std::snprintf(buffer, len, "%llu", static_cast<unsigned long long>(volume));
    }
}

lv_color_t quote_color(const Quote &quote) {
    if (quote.stale) {
        return kMuted;
    }
    if (quote.change > 0) {
        return kUp;
    }
    if (quote.change < 0) {
        return kDown;
    }
    return kText;
}

const char *state_text(const Quote &quote) {
    switch (visual_state_for(quote)) {
        case QuoteVisualState::Stale:
            return "STALE";
        case QuoteVisualState::MarketClosed:
            return "休市";
        case QuoteVisualState::Suspended:
            return "停牌";
        case QuoteVisualState::Offline:
            return "离线";
        case QuoteVisualState::Normal:
        default:
            return market_text(quote.market);
    }
}

lv_obj_t *make_label(lv_obj_t *parent, int width, lv_text_align_t align) {
    lv_obj_t *label = lv_label_create(parent);
    lv_obj_set_width(label, width);
    lv_label_set_long_mode(label, LV_LABEL_LONG_DOT);
    lv_obj_set_style_text_color(label, kText, 0);
    lv_obj_set_style_text_align(label, align, 0);
    lv_obj_set_style_text_font(label, LV_FONT_DEFAULT, 0);
    return label;
}

lv_obj_t *make_panel(lv_obj_t *parent) {
    lv_obj_t *panel = lv_obj_create(parent);
    lv_obj_set_style_bg_color(panel, kPanel, 0);
    lv_obj_set_style_radius(panel, 8, 0);
    lv_obj_set_style_border_color(panel, kLine, 0);
    lv_obj_set_style_border_width(panel, 1, 0);
    return panel;
}

int normalize_chart_value(double value, double min_value, double max_value) {
    const double span = max_value - min_value;
    if (span <= 0.000001) {
        return 500;
    }
    const double scaled = ((value - min_value) / span) * 1000.0;
    return static_cast<int>(std::clamp(scaled, 0.0, 1000.0));
}

}  // namespace

void StockDashboard::create(lv_display_t *display) {
    screen_ = lv_display_get_screen_active(display);
    show_list_screen();
    set_connection_status("mock mode");
}

void StockDashboard::set_detail_request_callback(DetailRequestCallback callback, void *ctx) {
    detail_request_callback_ = callback;
    detail_request_ctx_ = ctx;
}

void StockDashboard::prepare_screen() {
    lv_obj_clean(screen_);
    lv_obj_set_style_bg_color(screen_, kBg, 0);
    lv_obj_set_style_pad_all(screen_, 18, 0);
    lv_obj_set_flex_flow(screen_, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(screen_, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_STRETCH, LV_FLEX_ALIGN_START);

    time_label_ = nullptr;
    status_label_ = nullptr;
    list_ = nullptr;
    detail_title_label_ = nullptr;
    detail_subtitle_label_ = nullptr;
    detail_price_label_ = nullptr;
    detail_change_label_ = nullptr;
    detail_status_label_ = nullptr;
    detail_message_label_ = nullptr;
    detail_chart_ = nullptr;
    detail_volume_chart_ = nullptr;
    detail_price_series_ = nullptr;
    detail_avg_series_ = nullptr;
    detail_prev_close_series_ = nullptr;
    detail_volume_series_ = nullptr;
    detail_metric_values_.clear();
}

void StockDashboard::show_list_screen() {
    view_mode_ = ViewMode::List;
    prepare_screen();
    create_header();
    create_market_strip();
    create_list();
    rebuild_rows(quotes_);
    set_connection_status(connection_status_.c_str());
}

void StockDashboard::create_header() {
    lv_obj_t *header = lv_obj_create(screen_);
    lv_obj_set_height(header, 72);
    lv_obj_set_style_bg_color(header, kBg, 0);
    lv_obj_set_style_border_width(header, 0, 0);
    lv_obj_set_style_pad_all(header, 0, 0);
    lv_obj_set_flex_flow(header, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(header, LV_FLEX_ALIGN_SPACE_BETWEEN, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);

    lv_obj_t *title = lv_label_create(header);
    lv_label_set_text(title, "Tab5 股票监控");
    lv_obj_set_style_text_color(title, kText, 0);
    lv_obj_set_style_text_font(title, LV_FONT_DEFAULT, 0);

    status_label_ = lv_label_create(header);
    lv_label_set_text(status_label_, connection_status_.c_str());
    lv_obj_set_style_text_color(status_label_, kMuted, 0);
    lv_obj_set_style_text_font(status_label_, LV_FONT_DEFAULT, 0);
}

void StockDashboard::create_market_strip() {
    lv_obj_t *strip = make_panel(screen_);
    lv_obj_set_height(strip, 78);
    lv_obj_set_style_pad_all(strip, 14, 0);
    lv_obj_set_flex_flow(strip, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(strip, LV_FLEX_ALIGN_SPACE_BETWEEN, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);

    lv_obj_t *market = lv_label_create(strip);
    lv_label_set_text(market, "A股 / 港股    近实时 3-15s    点击卡片查看详情");
    lv_obj_set_style_text_color(market, kMuted, 0);
    lv_obj_set_style_text_font(market, LV_FONT_DEFAULT, 0);

    time_label_ = lv_label_create(strip);
    lv_label_set_text(time_label_, "等待行情");
    lv_obj_set_style_text_color(time_label_, kWarn, 0);
    lv_obj_set_style_text_font(time_label_, LV_FONT_DEFAULT, 0);
}

void StockDashboard::create_list() {
    list_ = lv_obj_create(screen_);
    lv_obj_set_flex_grow(list_, 1);
    lv_obj_set_style_bg_color(list_, kBg, 0);
    lv_obj_set_style_border_width(list_, 0, 0);
    lv_obj_set_style_pad_all(list_, 0, 0);
    lv_obj_set_style_pad_row(list_, 8, 0);
    lv_obj_set_scroll_dir(list_, LV_DIR_VER);
    lv_obj_set_flex_flow(list_, LV_FLEX_FLOW_COLUMN);
}

void StockDashboard::set_connection_status(const char *status) {
    connection_status_ = status == nullptr ? "" : status;
    if (status_label_ != nullptr) {
        lv_label_set_text(status_label_, connection_status_.c_str());
        lv_obj_set_style_text_color(status_label_, kMuted, 0);
    }
    if (detail_status_label_ != nullptr) {
        lv_label_set_text(detail_status_label_, connection_status_.c_str());
        lv_obj_set_style_text_color(detail_status_label_, kMuted, 0);
    }
}

void StockDashboard::set_error(const char *message) {
    connection_status_ = message == nullptr ? "error" : message;
    if (status_label_ != nullptr) {
        lv_label_set_text(status_label_, connection_status_.c_str());
        lv_obj_set_style_text_color(status_label_, kWarn, 0);
    }
    if (detail_status_label_ != nullptr) {
        lv_label_set_text(detail_status_label_, connection_status_.c_str());
        lv_obj_set_style_text_color(detail_status_label_, kWarn, 0);
    }
}

void StockDashboard::apply_snapshot(const std::vector<Quote> &quotes) {
    quotes_ = quotes;
    if (view_mode_ == ViewMode::List) {
        rebuild_rows(quotes_);
        if (time_label_ != nullptr && !quotes.empty()) {
            lv_label_set_text(time_label_, quotes.front().quote_ts.empty() ? "mock quote" : quotes.front().quote_ts.c_str());
        }
    }
}

void StockDashboard::apply_quote(const Quote &quote) {
    auto quote_iter = std::find_if(quotes_.begin(), quotes_.end(), [&](const Quote &existing) {
        return existing.symbol == quote.symbol;
    });
    if (quote_iter == quotes_.end()) {
        quotes_.push_back(quote);
    } else {
        *quote_iter = quote;
    }

    if (view_mode_ == ViewMode::Detail && quote.symbol == selected_symbol_) {
        update_detail_quote(quote);
        return;
    }

    if (view_mode_ != ViewMode::List) {
        return;
    }

    Row *row = find_row(quote.symbol);
    if (row == nullptr) {
        rebuild_rows(quotes_);
        return;
    }

    update_row(*row, quote);
    if (time_label_ != nullptr && !quote.quote_ts.empty()) {
        lv_label_set_text(time_label_, quote.quote_ts.c_str());
    }
}

void StockDashboard::apply_detail(const QuoteDetail &detail) {
    if (detail.symbol != selected_symbol_) {
        return;
    }
    apply_quote(detail.quote);
    update_detail_chart(detail);
    char message[96];
    std::snprintf(message, sizeof(message), "%s  %u点", detail.cached ? "缓存分时" : "最新分时", static_cast<unsigned>(detail.intraday.size()));
    set_label(detail_message_label_, message);
    if (detail_message_label_ != nullptr) {
        lv_obj_set_style_text_color(detail_message_label_, kMuted, 0);
    }
}

void StockDashboard::apply_detail_error(const char *symbol, const char *message) {
    if (symbol != nullptr && !selected_symbol_.empty() && selected_symbol_ != symbol) {
        return;
    }
    set_label(detail_message_label_, message == nullptr ? "详情加载失败" : message);
    if (detail_message_label_ != nullptr) {
        lv_obj_set_style_text_color(detail_message_label_, kWarn, 0);
    }
}

void StockDashboard::show_local_mock_detail(const char *symbol) {
    if (symbol == nullptr) {
        return;
    }
    const Quote *quote = find_quote(symbol);
    if (quote == nullptr) {
        apply_detail_error(symbol, "mock quote missing");
        return;
    }
    apply_detail(mock_detail_for_quote(*quote));
}

void StockDashboard::mark_offline() {
    set_error("offline");
    for (Row &row : rows_) {
        if (row.status_label != nullptr) {
            set_label(row.status_label, "离线");
            lv_obj_set_style_text_color(row.status_label, kWarn, 0);
        }
    }
    if (view_mode_ == ViewMode::Detail) {
        apply_detail_error(selected_symbol_.c_str(), "offline");
    }
}

void StockDashboard::open_detail(const std::string &symbol) {
    selected_symbol_ = symbol;
    const Quote *quote = find_quote(symbol);
    create_detail_screen(quote, "加载分时...");

    if (detail_request_callback_ == nullptr || !detail_request_callback_(symbol.c_str(), detail_request_ctx_)) {
        apply_detail_error(symbol.c_str(), "详情请求失败");
    }
}

void StockDashboard::create_detail_screen(const Quote *quote, const char *message) {
    view_mode_ = ViewMode::Detail;
    prepare_screen();

    lv_obj_t *top = lv_obj_create(screen_);
    lv_obj_set_height(top, 78);
    lv_obj_set_style_bg_color(top, kBg, 0);
    lv_obj_set_style_border_width(top, 0, 0);
    lv_obj_set_style_pad_all(top, 0, 0);
    lv_obj_set_flex_flow(top, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(top, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);

    lv_obj_t *back = lv_btn_create(top);
    lv_obj_set_size(back, 92, 48);
    lv_obj_set_style_bg_color(back, kPanel, 0);
    lv_obj_set_style_border_color(back, kLine, 0);
    lv_obj_set_style_border_width(back, 1, 0);
    lv_obj_add_event_cb(back, &StockDashboard::on_back_clicked, LV_EVENT_CLICKED, this);
    lv_obj_t *back_label = lv_label_create(back);
    lv_label_set_text(back_label, "返回");
    lv_obj_center(back_label);

    lv_obj_t *title_box = lv_obj_create(top);
    lv_obj_set_flex_grow(title_box, 1);
    lv_obj_set_height(title_box, LV_PCT(100));
    lv_obj_set_style_bg_opa(title_box, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(title_box, 0, 0);
    lv_obj_set_style_pad_left(title_box, 16, 0);
    lv_obj_set_flex_flow(title_box, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(title_box, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_START);

    detail_title_label_ = lv_label_create(title_box);
    lv_obj_set_style_text_color(detail_title_label_, kText, 0);
    detail_subtitle_label_ = lv_label_create(title_box);
    lv_obj_set_style_text_color(detail_subtitle_label_, kMuted, 0);

    detail_status_label_ = lv_label_create(top);
    lv_label_set_text(detail_status_label_, connection_status_.c_str());
    lv_obj_set_width(detail_status_label_, 200);
    lv_label_set_long_mode(detail_status_label_, LV_LABEL_LONG_DOT);
    lv_obj_set_style_text_align(detail_status_label_, LV_TEXT_ALIGN_RIGHT, 0);
    lv_obj_set_style_text_color(detail_status_label_, kMuted, 0);

    lv_obj_t *body = lv_obj_create(screen_);
    lv_obj_set_flex_grow(body, 1);
    lv_obj_set_style_bg_color(body, kBg, 0);
    lv_obj_set_style_border_width(body, 0, 0);
    lv_obj_set_style_pad_all(body, 0, 0);
    lv_obj_set_style_pad_column(body, 14, 0);
    lv_obj_set_flex_flow(body, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(body, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_STRETCH, LV_FLEX_ALIGN_START);

    lv_obj_t *chart_panel = make_panel(body);
    lv_obj_set_width(chart_panel, 790);
    lv_obj_set_height(chart_panel, LV_PCT(100));
    lv_obj_set_style_pad_all(chart_panel, 16, 0);
    lv_obj_set_flex_flow(chart_panel, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(chart_panel, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_STRETCH, LV_FLEX_ALIGN_START);

    lv_obj_t *quote_line = lv_obj_create(chart_panel);
    lv_obj_set_height(82);
    lv_obj_set_style_bg_opa(quote_line, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(quote_line, 0, 0);
    lv_obj_set_style_pad_all(quote_line, 0, 0);
    lv_obj_set_flex_flow(quote_line, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(quote_line, LV_FLEX_ALIGN_SPACE_BETWEEN, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);

    detail_price_label_ = lv_label_create(quote_line);
    lv_obj_set_width(detail_price_label_, 220);
    lv_obj_set_style_text_color(detail_price_label_, kText, 0);
    detail_change_label_ = lv_label_create(quote_line);
    lv_obj_set_width(detail_change_label_, 240);
    lv_obj_set_style_text_align(detail_change_label_, LV_TEXT_ALIGN_RIGHT, 0);
    lv_obj_set_style_text_color(detail_change_label_, kText, 0);
    detail_message_label_ = lv_label_create(quote_line);
    lv_obj_set_width(detail_message_label_, 250);
    lv_label_set_long_mode(detail_message_label_, LV_LABEL_LONG_DOT);
    lv_obj_set_style_text_align(detail_message_label_, LV_TEXT_ALIGN_RIGHT, 0);
    lv_obj_set_style_text_color(detail_message_label_, kMuted, 0);
    lv_label_set_text(detail_message_label_, message);

    detail_chart_ = lv_chart_create(chart_panel);
    lv_obj_set_height(detail_chart_, 330);
    lv_obj_set_width(detail_chart_, LV_PCT(100));
    lv_chart_set_type(detail_chart_, LV_CHART_TYPE_LINE);
    lv_chart_set_range(detail_chart_, LV_CHART_AXIS_PRIMARY_Y, 0, 1000);
    lv_obj_set_style_bg_color(detail_chart_, lv_color_hex(0x111820), 0);
    lv_obj_set_style_border_color(detail_chart_, kLine, 0);
    lv_obj_set_style_line_width(detail_chart_, 2, LV_PART_ITEMS);

    detail_volume_chart_ = lv_chart_create(chart_panel);
    lv_obj_set_height(detail_volume_chart_, 120);
    lv_obj_set_width(detail_volume_chart_, LV_PCT(100));
    lv_chart_set_type(detail_volume_chart_, LV_CHART_TYPE_BAR);
    lv_chart_set_range(detail_volume_chart_, LV_CHART_AXIS_PRIMARY_Y, 0, 1000);
    lv_obj_set_style_bg_color(detail_volume_chart_, lv_color_hex(0x111820), 0);
    lv_obj_set_style_border_color(detail_volume_chart_, kLine, 0);

    lv_obj_t *metric_panel = make_panel(body);
    lv_obj_set_flex_grow(metric_panel, 1);
    lv_obj_set_height(metric_panel, LV_PCT(100));
    lv_obj_set_style_pad_all(metric_panel, 14, 0);
    lv_obj_set_style_pad_row(metric_panel, 10, 0);
    lv_obj_set_style_pad_column(metric_panel, 10, 0);
    lv_obj_set_flex_flow(metric_panel, LV_FLEX_FLOW_ROW_WRAP);

    const char *labels[] = {"今开", "最高", "最低", "昨收", "成交量", "成交额", "市场", "状态", "时间"};
    for (const char *label_text : labels) {
        lv_obj_t *cell = lv_obj_create(metric_panel);
        lv_obj_set_size(cell, 156, 72);
        lv_obj_set_style_bg_color(cell, lv_color_hex(0x121920), 0);
        lv_obj_set_style_border_color(cell, kLine, 0);
        lv_obj_set_style_border_width(cell, 1, 0);
        lv_obj_set_style_radius(cell, 6, 0);
        lv_obj_set_style_pad_all(cell, 8, 0);
        lv_obj_set_flex_flow(cell, LV_FLEX_FLOW_COLUMN);

        lv_obj_t *caption = lv_label_create(cell);
        lv_label_set_text(caption, label_text);
        lv_obj_set_style_text_color(caption, kMuted, 0);
        lv_obj_t *value = lv_label_create(cell);
        lv_obj_set_width(value, LV_PCT(100));
        lv_label_set_long_mode(value, LV_LABEL_LONG_DOT);
        lv_obj_set_style_text_color(value, kText, 0);
        detail_metric_values_.push_back(value);
    }

    if (quote != nullptr) {
        update_detail_quote(*quote);
    } else {
        lv_label_set_text_fmt(detail_title_label_, "%s", selected_symbol_.c_str());
        lv_label_set_text(detail_subtitle_label_, "等待行情");
        set_label(detail_price_label_, "--");
        set_label(detail_change_label_, "--");
    }
}

void StockDashboard::update_detail_quote(const Quote &quote) {
    if (detail_title_label_ == nullptr) {
        return;
    }

    char price[32];
    char change[48];
    char open[32];
    char high[32];
    char low[32];
    char prev_close[32];
    char volume[32];
    char turnover[32];

    format_price(price, sizeof(price), quote.last);
    format_change(change, sizeof(change), quote);
    format_price(open, sizeof(open), quote.open);
    format_price(high, sizeof(high), quote.high);
    format_price(low, sizeof(low), quote.low);
    format_price(prev_close, sizeof(prev_close), quote.prev_close);
    format_volume(volume, sizeof(volume), quote.volume);
    format_turnover(turnover, sizeof(turnover), quote.turnover);

    lv_label_set_text_fmt(detail_title_label_, "%s  %s", quote.name.c_str(), quote.symbol.c_str());
    lv_label_set_text_fmt(detail_subtitle_label_, "%s  %s", market_text(quote.market), state_text(quote));
    set_label(detail_price_label_, price);
    set_label(detail_change_label_, change);

    const lv_color_t color = quote_color(quote);
    lv_obj_set_style_text_color(detail_price_label_, color, 0);
    lv_obj_set_style_text_color(detail_change_label_, color, 0);

    set_detail_metric(0, open);
    set_detail_metric(1, high);
    set_detail_metric(2, low);
    set_detail_metric(3, prev_close);
    set_detail_metric(4, volume);
    set_detail_metric(5, turnover);
    set_detail_metric(6, market_text(quote.market));
    set_detail_metric(7, state_text(quote));
    set_detail_metric(8, quote.quote_ts.empty() ? "--" : quote.quote_ts.c_str());
}

void StockDashboard::update_detail_chart(const QuoteDetail &detail) {
    if (detail_chart_ == nullptr || detail_volume_chart_ == nullptr || detail.intraday.empty()) {
        return;
    }

    auto price_range = std::minmax_element(
        detail.intraday.begin(),
        detail.intraday.end(),
        [](const IntradayPoint &left, const IntradayPoint &right) { return left.price < right.price; });
    double min_price = price_range.first->price;
    double max_price = price_range.second->price;
    if (detail.quote.prev_close > 0.0) {
        min_price = std::min(min_price, detail.quote.prev_close);
        max_price = std::max(max_price, detail.quote.prev_close);
    }
    if (std::abs(max_price - min_price) < 0.000001) {
        min_price -= 1.0;
        max_price += 1.0;
    }

    uint64_t max_volume = 1;
    for (const IntradayPoint &point : detail.intraday) {
        max_volume = std::max(max_volume, point.volume);
    }

    const uint16_t point_count = static_cast<uint16_t>(std::min<size_t>(detail.intraday.size(), 240));
    lv_chart_set_point_count(detail_chart_, point_count);
    lv_chart_set_point_count(detail_volume_chart_, point_count);
    lv_chart_set_range(detail_chart_, LV_CHART_AXIS_PRIMARY_Y, 0, 1000);
    lv_chart_set_range(detail_volume_chart_, LV_CHART_AXIS_PRIMARY_Y, 0, 1000);

    detail_price_series_ = lv_chart_add_series(detail_chart_, quote_color(detail.quote), LV_CHART_AXIS_PRIMARY_Y);
    detail_avg_series_ = lv_chart_add_series(detail_chart_, kAvg, LV_CHART_AXIS_PRIMARY_Y);
    detail_prev_close_series_ = lv_chart_add_series(detail_chart_, kMuted, LV_CHART_AXIS_PRIMARY_Y);
    detail_volume_series_ = lv_chart_add_series(detail_volume_chart_, kMuted, LV_CHART_AXIS_PRIMARY_Y);

    const size_t start = detail.intraday.size() - point_count;
    const int prev_close = normalize_chart_value(detail.quote.prev_close, min_price, max_price);
    for (size_t index = start; index < detail.intraday.size(); ++index) {
        const IntradayPoint &point = detail.intraday[index];
        lv_chart_set_next_value(detail_chart_, detail_price_series_, normalize_chart_value(point.price, min_price, max_price));
        lv_chart_set_next_value(detail_chart_, detail_avg_series_, normalize_chart_value(point.avg_price, min_price, max_price));
        lv_chart_set_next_value(detail_chart_, detail_prev_close_series_, prev_close);
        const int scaled_volume = static_cast<int>((static_cast<double>(point.volume) / static_cast<double>(max_volume)) * 1000.0);
        lv_chart_set_next_value(detail_volume_chart_, detail_volume_series_, scaled_volume);
    }
    lv_chart_refresh(detail_chart_);
    lv_chart_refresh(detail_volume_chart_);
}

void StockDashboard::rebuild_rows(const std::vector<Quote> &quotes) {
    rows_.clear();
    if (list_ == nullptr) {
        return;
    }
    lv_obj_clean(list_);
    rows_.reserve(quotes.size());

    for (const Quote &quote : quotes) {
        rows_.push_back(Row{});
        Row &row = rows_.back();
        row.symbol = quote.symbol;
        row.container = lv_obj_create(list_);
        lv_obj_set_height(row.container, 70);
        lv_obj_set_width(row.container, LV_PCT(100));
        lv_obj_set_style_bg_color(row.container, kPanel, 0);
        lv_obj_set_style_radius(row.container, 6, 0);
        lv_obj_set_style_border_color(row.container, kLine, 0);
        lv_obj_set_style_border_width(row.container, 1, 0);
        lv_obj_set_style_pad_all(row.container, 12, 0);
        lv_obj_set_flex_flow(row.container, LV_FLEX_FLOW_ROW);
        lv_obj_set_flex_align(row.container, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);
        lv_obj_add_flag(row.container, LV_OBJ_FLAG_CLICKABLE);
        lv_obj_add_event_cb(row.container, &StockDashboard::on_row_clicked, LV_EVENT_CLICKED, this);

        row.symbol_label = make_label(row.container, 120, LV_TEXT_ALIGN_LEFT);
        row.name_label = make_label(row.container, 160, LV_TEXT_ALIGN_LEFT);
        row.last_label = make_label(row.container, 100, LV_TEXT_ALIGN_RIGHT);
        row.change_label = make_label(row.container, 150, LV_TEXT_ALIGN_RIGHT);
        row.turnover_label = make_label(row.container, 125, LV_TEXT_ALIGN_RIGHT);
        row.status_label = make_label(row.container, 70, LV_TEXT_ALIGN_RIGHT);

        update_row(row, quote);
    }
}

void StockDashboard::update_row(Row &row, const Quote &quote) {
    char price[32];
    char change[48];
    char turnover[32];

    format_price(price, sizeof(price), quote.last);
    format_change(change, sizeof(change), quote);
    format_turnover(turnover, sizeof(turnover), quote.turnover);

    set_label(row.symbol_label, quote.symbol.c_str());
    set_label(row.name_label, quote.name.c_str());
    set_label(row.last_label, price);
    set_label(row.change_label, change);
    set_label(row.turnover_label, turnover);
    set_label(row.status_label, state_text(quote));

    const lv_color_t color = quote_color(quote);
    lv_obj_set_style_text_color(row.last_label, color, 0);
    lv_obj_set_style_text_color(row.change_label, color, 0);
    lv_obj_set_style_text_color(row.status_label, quote.stale ? kWarn : kMuted, 0);
}

StockDashboard::Row *StockDashboard::find_row(const std::string &symbol) {
    auto iter = std::find_if(rows_.begin(), rows_.end(), [&](const Row &row) { return row.symbol == symbol; });
    return iter == rows_.end() ? nullptr : &(*iter);
}

Quote *StockDashboard::find_quote(const std::string &symbol) {
    auto iter = std::find_if(quotes_.begin(), quotes_.end(), [&](const Quote &quote) { return quote.symbol == symbol; });
    return iter == quotes_.end() ? nullptr : &(*iter);
}

const Quote *StockDashboard::find_quote(const std::string &symbol) const {
    auto iter = std::find_if(quotes_.begin(), quotes_.end(), [&](const Quote &quote) { return quote.symbol == symbol; });
    return iter == quotes_.end() ? nullptr : &(*iter);
}

void StockDashboard::set_detail_metric(size_t index, const char *value) {
    if (index < detail_metric_values_.size()) {
        set_label(detail_metric_values_[index], value);
    }
}

void StockDashboard::on_row_clicked(lv_event_t *event) {
    auto *dashboard = static_cast<StockDashboard *>(lv_event_get_user_data(event));
    lv_obj_t *target = static_cast<lv_obj_t *>(lv_event_get_current_target(event));
    if (dashboard == nullptr || target == nullptr || lv_obj_get_child_cnt(target) == 0) {
        return;
    }
    lv_obj_t *symbol_label = lv_obj_get_child(target, 0);
    dashboard->open_detail(lv_label_get_text(symbol_label));
}

void StockDashboard::on_back_clicked(lv_event_t *event) {
    auto *dashboard = static_cast<StockDashboard *>(lv_event_get_user_data(event));
    if (dashboard != nullptr) {
        dashboard->selected_symbol_.clear();
        dashboard->show_list_screen();
    }
}
