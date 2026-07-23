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
    uint64_t volume = 0;
    double turnover = 0.0;
    std::string trade_status = "normal";
    QuoteVisualState status = QuoteVisualState::Normal;
    std::string quote_ts;
    bool stale = false;
};

struct ParsedStreamMessage {
    std::vector<Quote> snapshot;
    std::optional<Quote> quote;
    std::optional<std::string> status;
    std::optional<std::string> error;
};

std::vector<Quote> default_quotes();
ParsedStreamMessage parse_stream_message(const char *data, int len);
QuoteVisualState visual_state_for(const Quote &quote);
const char *market_text(Market market);
