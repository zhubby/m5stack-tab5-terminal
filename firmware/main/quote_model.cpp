#include "quote_model.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>

#include "cJSON.h"

namespace {

Market parse_market(const cJSON *node) {
    if (!cJSON_IsString(node) || node->valuestring == nullptr) {
        return Market::Cn;
    }
    return std::strcmp(node->valuestring, "hk") == 0 ? Market::Hk : Market::Cn;
}

std::string json_string(const cJSON *object, const char *name, const char *fallback = "") {
    const cJSON *node = cJSON_GetObjectItemCaseSensitive(object, name);
    if (!cJSON_IsString(node) || node->valuestring == nullptr) {
        return fallback;
    }
    return node->valuestring;
}

double json_double(const cJSON *object, const char *name) {
    const cJSON *node = cJSON_GetObjectItemCaseSensitive(object, name);
    return cJSON_IsNumber(node) ? node->valuedouble : 0.0;
}

uint64_t json_u64(const cJSON *object, const char *name) {
    const cJSON *node = cJSON_GetObjectItemCaseSensitive(object, name);
    return cJSON_IsNumber(node) && node->valuedouble > 0 ? static_cast<uint64_t>(node->valuedouble)
                                                        : 0;
}

bool json_bool(const cJSON *object, const char *name) {
    const cJSON *node = cJSON_GetObjectItemCaseSensitive(object, name);
    return cJSON_IsBool(node) && cJSON_IsTrue(node);
}

Quote parse_quote_object(const cJSON *object) {
    Quote quote;
    quote.symbol = json_string(object, "symbol");
    quote.name = json_string(object, "name", quote.symbol.c_str());
    quote.market = parse_market(cJSON_GetObjectItemCaseSensitive(object, "market"));
    quote.last = json_double(object, "last");
    quote.change = json_double(object, "change");
    quote.change_pct = json_double(object, "change_pct");
    quote.open = json_double(object, "open");
    quote.high = json_double(object, "high");
    quote.low = json_double(object, "low");
    quote.prev_close = json_double(object, "prev_close");
    if (quote.prev_close == 0.0 && quote.last != 0.0) {
        quote.prev_close = quote.last - quote.change;
    }
    if (quote.open == 0.0) {
        quote.open = quote.last;
    }
    if (quote.high == 0.0) {
        quote.high = quote.last > quote.open ? quote.last : quote.open;
    }
    if (quote.low == 0.0) {
        quote.low = quote.last < quote.open ? quote.last : quote.open;
    }
    quote.volume = json_u64(object, "volume");
    quote.turnover = json_double(object, "turnover");
    quote.trade_status = json_string(object, "trade_status", "normal");
    const std::string status = json_string(object, "status", "normal");
    if (status == "stale") {
        quote.status = QuoteVisualState::Stale;
    } else if (status == "offline") {
        quote.status = QuoteVisualState::Offline;
    } else if (status == "market_closed") {
        quote.status = QuoteVisualState::MarketClosed;
    } else if (status == "suspended") {
        quote.status = QuoteVisualState::Suspended;
    } else {
        quote.status = QuoteVisualState::Normal;
    }
    quote.quote_ts = json_string(object, "quote_ts");
    quote.stale = json_bool(object, "stale");
    return quote;
}

IntradayPoint parse_intraday_point(const cJSON *object) {
    IntradayPoint point;
    point.ts = json_string(object, "ts");
    point.price = json_double(object, "price");
    point.avg_price = json_double(object, "avg_price");
    point.volume = json_u64(object, "volume");
    point.turnover = json_double(object, "turnover");
    return point;
}

QuoteDetail parse_detail_object(const cJSON *root) {
    QuoteDetail detail;
    detail.request_id = json_u64(root, "request_id");
    detail.symbol = json_string(root, "symbol");
    detail.server_ts = json_string(root, "server_ts");
    detail.cached = json_bool(root, "cached");

    const cJSON *quote = cJSON_GetObjectItemCaseSensitive(root, "quote");
    if (cJSON_IsObject(quote)) {
        detail.quote = parse_quote_object(quote);
    }

    const cJSON *points = cJSON_GetObjectItemCaseSensitive(root, "intraday");
    const cJSON *entry = nullptr;
    cJSON_ArrayForEach(entry, points) {
        if (cJSON_IsObject(entry)) {
            detail.intraday.push_back(parse_intraday_point(entry));
        }
    }
    return detail;
}

}  // namespace

std::vector<Quote> default_quotes() {
    return {
        {.symbol = "000001.SH",
         .name = "上证指数",
         .market = Market::Cn,
         .last = 3104.52,
         .change = 12.31,
         .change_pct = 0.40,
         .open = 3092.20,
         .high = 3116.80,
         .low = 3088.10,
         .prev_close = 3092.21,
         .volume = 238820000,
         .turnover = 331200000000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
        {.symbol = "399001.SZ",
         .name = "深证成指",
         .market = Market::Cn,
         .last = 9824.18,
         .change = -21.77,
         .change_pct = -0.22,
         .open = 9851.32,
         .high = 9886.90,
         .low = 9798.40,
         .prev_close = 9845.95,
         .volume = 315440000,
         .turnover = 412900000000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
        {.symbol = "600519.SH",
         .name = "贵州茅台",
         .market = Market::Cn,
         .last = 1682.65,
         .change = 9.20,
         .change_pct = 0.55,
         .open = 1675.20,
         .high = 1688.90,
         .low = 1669.30,
         .prev_close = 1673.45,
         .volume = 2600000,
         .turnover = 4374000000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
        {.symbol = "00700.HK",
         .name = "腾讯控股",
         .market = Market::Hk,
         .last = 381.20,
         .change = -1.80,
         .change_pct = -0.47,
         .open = 383.80,
         .high = 385.40,
         .low = 379.20,
         .prev_close = 383.00,
         .volume = 16800000,
         .turnover = 6404200000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
        {.symbol = "09988.HK",
         .name = "阿里巴巴-W",
         .market = Market::Hk,
         .last = 78.35,
         .change = 0.55,
         .change_pct = 0.71,
         .open = 77.90,
         .high = 78.80,
         .low = 77.35,
         .prev_close = 77.80,
         .volume = 51200000,
         .turnover = 4011000000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
    };
}

QuoteDetail mock_detail_for_quote(const Quote &quote) {
    QuoteDetail detail;
    detail.request_id = 0;
    detail.symbol = quote.symbol;
    detail.quote = quote;
    detail.server_ts = "mock";
    detail.cached = false;

    const double base = quote.prev_close != 0.0 ? quote.prev_close : quote.last;
    double cumulative = 0.0;
    detail.intraday.reserve(120);
    for (int index = 0; index < 120; ++index) {
        const double progress = static_cast<double>(index) / 119.0;
        const double wave = std::sin(static_cast<double>(index) / 8.0) * base * 0.0018;
        const double price = base + (quote.last - base) * progress + wave;
        cumulative += price;

        char ts[16];
        std::snprintf(ts, sizeof(ts), "T-%03dm", 119 - index);
        IntradayPoint point;
        point.ts = ts;
        point.price = price;
        point.avg_price = cumulative / static_cast<double>(index + 1);
        point.volume = 2000 + static_cast<uint64_t>((index * 137 + quote.symbol.size() * 211) % 14000);
        point.turnover = point.price * static_cast<double>(point.volume);
        detail.intraday.push_back(point);
    }
    return detail;
}

ParsedStreamMessage parse_stream_message(const char *data, int len) {
    ParsedStreamMessage result;
    cJSON *root = cJSON_ParseWithLength(data, len);
    if (root == nullptr) {
        result.error = "invalid JSON";
        return result;
    }

    const std::string type = json_string(root, "type");
    if (type == "snapshot") {
        const cJSON *quotes = cJSON_GetObjectItemCaseSensitive(root, "quotes");
        const cJSON *entry = nullptr;
        cJSON_ArrayForEach(entry, quotes) {
            if (cJSON_IsObject(entry)) {
                result.snapshot.push_back(parse_quote_object(entry));
            }
        }
    } else if (type == "quote") {
        const cJSON *quote = cJSON_GetObjectItemCaseSensitive(root, "quote");
        if (cJSON_IsObject(quote)) {
            result.quote = parse_quote_object(quote);
        }
    } else if (type == "detail") {
        result.detail = parse_detail_object(root);
    } else if (type == "detail_error") {
        DetailError detail_error;
        detail_error.request_id = json_u64(root, "request_id");
        detail_error.symbol = json_string(root, "symbol");
        detail_error.message = json_string(root, "message", "detail error");
        detail_error.server_ts = json_string(root, "server_ts");
        result.detail_error = detail_error;
    } else if (type == "status") {
        result.status = json_string(root, "status");
    } else if (type == "error") {
        result.error = json_string(root, "message", "backend error");
    }

    cJSON_Delete(root);
    return result;
}

QuoteVisualState visual_state_for(const Quote &quote) {
    if (quote.stale) {
        return QuoteVisualState::Stale;
    }
    if (quote.status != QuoteVisualState::Normal) {
        return quote.status;
    }
    if (quote.trade_status == "suspended") {
        return QuoteVisualState::Suspended;
    }
    if (quote.trade_status == "market_closed") {
        return QuoteVisualState::MarketClosed;
    }
    return QuoteVisualState::Normal;
}

const char *market_text(Market market) {
    return market == Market::Hk ? "HK" : "CN";
}
