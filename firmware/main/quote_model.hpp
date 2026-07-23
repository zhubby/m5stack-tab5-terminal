#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

enum class Market {
    Cn,
    Hk,
};

enum class QuoteVisualState {
    Normal,
    Stale,
    Offline,
    MarketClosed,
    Suspended,
};

struct Quote {
    std::string symbol;
    std::string name;
    Market market = Market::Cn;
    double last = 0.0;
    double change = 0.0;
    double change_pct = 0.0;
    double open = 0.0;
    double high = 0.0;
    double low = 0.0;
    double prev_close = 0.0;
    uint64_t volume = 0;
    double turnover = 0.0;
    std::string trade_status = "normal";
    QuoteVisualState status = QuoteVisualState::Normal;
    std::string quote_ts;
    bool stale = false;
};

struct IntradayPoint {
    std::string ts;
    double price = 0.0;
    double avg_price = 0.0;
    uint64_t volume = 0;
    double turnover = 0.0;
};

struct QuoteDetail {
    uint64_t request_id = 0;
    std::string symbol;
    Quote quote;
    std::vector<IntradayPoint> intraday;
    std::string server_ts;
    bool cached = false;
};

struct DetailError {
    uint64_t request_id = 0;
    std::string symbol;
    std::string message;
    std::string server_ts;
};

struct ParsedStreamMessage {
    std::vector<Quote> snapshot;
    std::optional<Quote> quote;
    std::optional<QuoteDetail> detail;
    std::optional<DetailError> detail_error;
    std::optional<std::string> status;
    std::optional<std::string> error;
};

std::vector<Quote> default_quotes();
QuoteDetail mock_detail_for_quote(const Quote &quote);
ParsedStreamMessage parse_stream_message(const char *data, int len);
QuoteVisualState visual_state_for(const Quote &quote);
const char *market_text(Market market);
