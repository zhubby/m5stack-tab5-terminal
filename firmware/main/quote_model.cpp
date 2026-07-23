#include "quote_model.hpp"

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

Quote parse_quote_object(const cJSON *object) {
    Quote quote;
    quote.symbol = json_string(object, "symbol");
    quote.name = json_string(object, "name", quote.symbol.c_str());
    quote.market = parse_market(cJSON_GetObjectItemCaseSensitive(object, "market"));
    quote.last = json_double(object, "last");
    quote.change = json_double(object, "change");
    quote.change_pct = json_double(object, "change_pct");
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

    const cJSON *stale = cJSON_GetObjectItemCaseSensitive(object, "stale");
    quote.stale = cJSON_IsBool(stale) && cJSON_IsTrue(stale);
    return quote;
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
         .volume = 51200000,
         .turnover = 4011000000.0,
         .trade_status = "normal",
         .quote_ts = "--",
         .stale = false},
    };
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
