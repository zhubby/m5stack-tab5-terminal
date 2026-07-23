#include "ui.hpp"

#include <algorithm>
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

void set_label(lv_obj_t *label, const char *text) {
    lv_label_set_text(label, text);
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

}  // namespace

void StockDashboard::create(lv_display_t *display) {
    screen_ = lv_display_get_screen_active(display);
    lv_obj_set_style_bg_color(screen_, kBg, 0);
    lv_obj_set_style_pad_all(screen_, 18, 0);
    lv_obj_set_flex_flow(screen_, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(screen_, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_STRETCH, LV_FLEX_ALIGN_START);

    create_header();
    create_market_strip();
    create_list();
    set_connection_status("mock mode");
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
    lv_label_set_text(status_label_, "starting");
    lv_obj_set_style_text_color(status_label_, kMuted, 0);
    lv_obj_set_style_text_font(status_label_, LV_FONT_DEFAULT, 0);
}

void StockDashboard::create_market_strip() {
    lv_obj_t *strip = lv_obj_create(screen_);
    lv_obj_set_height(strip, 78);
    lv_obj_set_style_bg_color(strip, kPanel, 0);
    lv_obj_set_style_radius(strip, 8, 0);
    lv_obj_set_style_border_color(strip, kLine, 0);
    lv_obj_set_style_border_width(strip, 1, 0);
    lv_obj_set_style_pad_all(strip, 14, 0);
    lv_obj_set_flex_flow(strip, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(strip, LV_FLEX_ALIGN_SPACE_BETWEEN, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);

    lv_obj_t *market = lv_label_create(strip);
    lv_label_set_text(market, "A股 / 港股    近实时 3-15s");
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
    if (status_label_ != nullptr) {
        lv_label_set_text(status_label_, status);
        lv_obj_set_style_text_color(status_label_, kMuted, 0);
    }
}

void StockDashboard::set_error(const char *message) {
    set_connection_status(message);
    if (status_label_ != nullptr) {
        lv_obj_set_style_text_color(status_label_, kWarn, 0);
    }
}

void StockDashboard::apply_snapshot(const std::vector<Quote> &quotes) {
    rebuild_rows(quotes);
    if (time_label_ != nullptr && !quotes.empty()) {
        lv_label_set_text(time_label_, quotes.front().quote_ts.empty() ? "mock quote" : quotes.front().quote_ts.c_str());
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

void StockDashboard::mark_offline() {
    set_error("offline");
    for (Row &row : rows_) {
        if (row.status_label != nullptr) {
            set_label(row.status_label, "离线");
            lv_obj_set_style_text_color(row.status_label, kWarn, 0);
        }
    }
}

void StockDashboard::rebuild_rows(const std::vector<Quote> &quotes) {
    quotes_ = quotes;
    rows_.clear();
    lv_obj_clean(list_);
    rows_.reserve(quotes.size());

    for (const Quote &quote : quotes) {
        Row row;
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

        row.symbol_label = make_label(row.container, 105, LV_TEXT_ALIGN_LEFT);
        row.name_label = make_label(row.container, 135, LV_TEXT_ALIGN_LEFT);
        row.last_label = make_label(row.container, 90, LV_TEXT_ALIGN_RIGHT);
        row.change_label = make_label(row.container, 130, LV_TEXT_ALIGN_RIGHT);
        row.turnover_label = make_label(row.container, 105, LV_TEXT_ALIGN_RIGHT);
        row.status_label = make_label(row.container, 60, LV_TEXT_ALIGN_RIGHT);

        update_row(row, quote);
        rows_.push_back(row);
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
