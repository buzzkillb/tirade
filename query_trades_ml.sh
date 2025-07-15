#!/bin/bash

# ML Trade Data Query Script (Bash Version)
# 
# This script allows you to manually check and verify that machine learning trade data
# is being stored correctly in the database. It provides various query options and
# detailed analysis of the ML trade history.
#
# Usage:
#     ./query_trades_ml.sh --pair SOLUSDC --limit 20
#     ./query_trades_ml.sh --pair SOLUSDC --stats
#     ./query_trades_ml.sh --pair SOLUSDC --verify
#     ./query_trades_ml.sh --pair SOLUSDC --export csv

# Configuration
DEFAULT_DATABASE_URL="http://localhost:8080"
DEFAULT_PAIR="SOL/USDC"
DEFAULT_LIMIT=20

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Emojis
CHECK_MARK="✅"
CROSS_MARK="❌"
WARNING="⚠️"
MONEY="💰"
LOSS="💸"
ROBOT="🤖"
CHART="📊"
SEARCH="🔍"
FILE="📁"
CLOCK="🕐"

# Function to print colored output
print_info() {
    echo -e "${BLUE}$1${NC}"
}

print_success() {
    echo -e "${GREEN}$1${NC}"
}

print_warning() {
    echo -e "${YELLOW}$1${NC}"
}

print_error() {
    echo -e "${RED}$1${NC}"
}

print_header() {
    echo -e "${PURPLE}$1${NC}"
}

# Function to check if jq is installed
check_dependencies() {
    if ! command -v jq &> /dev/null; then
        print_error "❌ jq is required but not installed. Please install jq first."
        print_info "Install with: sudo apt-get install jq (Ubuntu/Debian) or brew install jq (macOS)"
        exit 1
    fi
    
    if ! command -v curl &> /dev/null; then
        print_error "❌ curl is required but not installed."
        exit 1
    fi
}

# Function to make API request
make_request() {
    local url="$1"
    local response
    
    response=$(curl -s -w "%{http_code}" "$url" 2>/dev/null)
    local http_code="${response: -3}"
    local body="${response%???}"
    
    if [ "$http_code" -eq 200 ]; then
        echo "$body"
    else
        print_error "❌ HTTP $http_code error for $url"
        return 1
    fi
}

# Function to get ML trades
get_ml_trades() {
    local pair="$1"
    local limit="$2"
    local url="${DATABASE_URL}/ml/trades/${pair}?limit=${limit}"
    
    local response
    response=$(make_request "$url")
    
    if [ $? -eq 0 ]; then
        local success=$(echo "$response" | jq -r '.success // false')
        if [ "$success" = "true" ]; then
            echo "$response" | jq -r '.data // []'
        else
            local message=$(echo "$response" | jq -r '.message // "Unknown error"')
            print_error "❌ Error: $message"
            return 1
        fi
    else
        return 1
    fi
}

# Function to get ML stats
get_ml_stats() {
    local pair="$1"
    local url="${DATABASE_URL}/ml/stats/${pair}"
    
    local response
    response=$(make_request "$url")
    
    if [ $? -eq 0 ]; then
        local success=$(echo "$response" | jq -r '.success // false')
        if [ "$success" = "true" ]; then
            echo "$response" | jq -r '.data // null'
        else
            local message=$(echo "$response" | jq -r '.message // "Unknown error"')
            print_error "❌ Error: $message"
            return 1
        fi
    else
        return 1
    fi
}

# Function to get ML status
get_ml_status() {
    local url="${DATABASE_URL}/ml/status"
    
    local response
    response=$(make_request "$url")
    
    if [ $? -eq 0 ]; then
        local success=$(echo "$response" | jq -r '.success // false')
        if [ "$success" = "true" ]; then
            echo "$response" | jq -r '.data // null'
        else
            local message=$(echo "$response" | jq -r '.message // "Unknown error"')
            print_error "❌ Error: $message"
            return 1
        fi
    else
        return 1
    fi
}

# Function to format trade for display
format_trade() {
    local trade="$1"
    local index="$2"
    
    local id=$(echo "$trade" | jq -r '.id')
    local pair=$(echo "$trade" | jq -r '.pair')
    local entry_price=$(echo "$trade" | jq -r '.entry_price')
    local exit_price=$(echo "$trade" | jq -r '.exit_price')
    local pnl=$(echo "$trade" | jq -r '.pnl')
    local duration_seconds=$(echo "$trade" | jq -r '.duration_seconds')
    local entry_time=$(echo "$trade" | jq -r '.entry_time')
    local exit_time=$(echo "$trade" | jq -r '.exit_time')
    local success=$(echo "$trade" | jq -r '.success')
    local market_regime=$(echo "$trade" | jq -r '.market_regime')
    local trend_strength=$(echo "$trade" | jq -r '.trend_strength')
    local volatility=$(echo "$trade" | jq -r '.volatility')
    local created_at=$(echo "$trade" | jq -r '.created_at')
    
    # Calculate PnL percentage
    local pnl_percent=$(echo "$pnl * 100" | bc -l 2>/dev/null || echo "0")
    
    # Calculate duration in minutes
    local duration_minutes=$(echo "scale=1; $duration_seconds / 60" | bc -l 2>/dev/null || echo "0")
    
    # Format timestamps
    local entry_time_formatted=$(date -d "$entry_time" +"%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "$entry_time")
    local created_at_formatted=$(date -d "$created_at" +"%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "$created_at")
    
    # Determine status emoji
    local status_emoji="$CHECK_MARK"
    if [ "$success" = "false" ]; then
        status_emoji="$CROSS_MARK"
    fi
    
    # Determine PnL emoji
    local pnl_emoji="$MONEY"
    if (( $(echo "$pnl < 0" | bc -l) )); then
        pnl_emoji="$LOSS"
    elif (( $(echo "$pnl == 0" | bc -l) )); then
        pnl_emoji="➡️"
    fi
    
    printf "%2d. %s %s %s | Entry: $%.4f | Exit: $%.4f | PnL: %+.2f%% | Duration: %.1fm | Regime: %s | Trend: %.3f | Vol: %.3f | Time: %s\n" \
        "$index" "$status_emoji" "$pnl_emoji" "$pair" "$entry_price" "$exit_price" "$pnl_percent" "$duration_minutes" "$market_regime" "$trend_strength" "$volatility" "$entry_time_formatted"
    
    if [ "$SHOW_DETAILS" = "true" ]; then
        echo "    ID: $id"
        echo "    Created: $created_at_formatted"
        echo
    fi
}

# Function to display trades
display_trades() {
    local trades="$1"
    local count=$(echo "$trades" | jq 'length')
    
    if [ "$count" -eq 0 ]; then
        print_warning "📭 No ML trades found for this pair."
        return
    fi
    
    print_header "\n$ROBOT ML Trade History ($count trades):"
    echo "================================================================================================================================"
    
    local index=1
    echo "$trades" | jq -c '.[]' | while read -r trade; do
        format_trade "$trade" "$index"
        ((index++))
    done
}

# Function to display stats
display_stats() {
    local stats="$1"
    
    if [ "$stats" = "null" ]; then
        print_warning "📭 No ML stats available for this pair."
        return
    fi
    
    local total_trades=$(echo "$stats" | jq -r '.total_trades // 0')
    local win_rate=$(echo "$stats" | jq -r '.win_rate // 0')
    local avg_pnl=$(echo "$stats" | jq -r '.avg_pnl // 0')
    local avg_win=$(echo "$stats" | jq -r '.avg_win // 0')
    local avg_loss=$(echo "$stats" | jq -r '.avg_loss // 0')
    
    print_header "\n$CHART ML Trade Statistics:"
    echo "=================================================="
    echo "Total Trades: $total_trades"
    printf "Win Rate: %.1f%%\n" "$(echo "$win_rate * 100" | bc -l)"
    printf "Average PnL: %+.2f%%\n" "$(echo "$avg_pnl * 100" | bc -l)"
    printf "Average Win: %+.2f%%\n" "$(echo "$avg_win * 100" | bc -l)"
    printf "Average Loss: %+.2f%%\n" "$(echo "$avg_loss * 100" | bc -l)"
}

# Function to display ML status
display_ml_status() {
    local status="$1"
    
    if [ "$status" = "null" ]; then
        print_warning "📭 No ML status available."
        return
    fi
    
    local enabled=$(echo "$status" | jq -r '.enabled // false')
    local min_confidence=$(echo "$status" | jq -r '.min_confidence // 0')
    local max_position_size=$(echo "$status" | jq -r '.max_position_size // 0')
    local total_trades=$(echo "$status" | jq -r '.total_trades // 0')
    local win_rate=$(echo "$status" | jq -r '.win_rate // 0')
    local avg_pnl=$(echo "$status" | jq -r '.avg_pnl // 0')
    
    local enabled_emoji="$CHECK_MARK"
    if [ "$enabled" = "false" ]; then
        enabled_emoji="$CROSS_MARK"
    fi
    
    print_header "\n$ROBOT ML System Status:"
    echo "=================================================="
    echo "Enabled: $enabled_emoji"
    printf "Min Confidence: %.1f%%\n" "$(echo "$min_confidence * 100" | bc -l)"
    printf "Max Position Size: %.1f%%\n" "$(echo "$max_position_size * 100" | bc -l)"
    echo "Total Trades: $total_trades"
    printf "Win Rate: %.1f%%\n" "$(echo "$win_rate * 100" | bc -l)"
    printf "Average PnL: %+.2f%%\n" "$(echo "$avg_pnl * 100" | bc -l)"
}

# Function to verify trade data
verify_trade_data() {
    local trades="$1"
    local count=$(echo "$trades" | jq 'length')
    
    if [ "$count" -eq 0 ]; then
        print_error "❌ No trades found for verification"
        return 1
    fi
    
    print_info "\n$SEARCH Verifying trade data integrity..."
    
    local errors=0
    local warnings=0
    local index=1
    
    echo "$trades" | jq -c '.[]' | while read -r trade; do
        local id=$(echo "$trade" | jq -r '.id')
        local pair=$(echo "$trade" | jq -r '.pair')
        local entry_price=$(echo "$trade" | jq -r '.entry_price')
        local exit_price=$(echo "$trade" | jq -r '.exit_price')
        local pnl=$(echo "$trade" | jq -r '.pnl')
        local duration_seconds=$(echo "$trade" | jq -r '.duration_seconds')
        local entry_time=$(echo "$trade" | jq -r '.entry_time')
        local exit_time=$(echo "$trade" | jq -r '.exit_time')
        local success=$(echo "$trade" | jq -r '.success')
        local market_regime=$(echo "$trade" | jq -r '.market_regime')
        local trend_strength=$(echo "$trade" | jq -r '.trend_strength')
        local volatility=$(echo "$trade" | jq -r '.volatility')
        local created_at=$(echo "$trade" | jq -r '.created_at')
        
        # Check required fields
        if [ "$id" = "null" ] || [ -z "$id" ]; then
            print_error "   • Trade $index: Missing required field 'id'"
            ((errors++))
        fi
        
        if [ "$entry_price" = "null" ] || [ "$entry_price" = "0" ]; then
            print_error "   • Trade $index: Invalid entry_price ($entry_price)"
            ((errors++))
        fi
        
        if [ "$exit_price" = "null" ] || [ "$exit_price" = "0" ]; then
            print_error "   • Trade $index: Invalid exit_price ($exit_price)"
            ((errors++))
        fi
        
        if [ "$duration_seconds" = "null" ] || (( $(echo "$duration_seconds < 0" | bc -l) )); then
            print_warning "   • Trade $index: Negative or invalid duration ($duration_seconds)s"
            ((warnings++))
        fi
        
        if [ "$trend_strength" = "null" ] || (( $(echo "$trend_strength < 0" | bc -l) )) || (( $(echo "$trend_strength > 1" | bc -l) )); then
            print_warning "   • Trade $index: Trend strength out of range ($trend_strength)"
            ((warnings++))
        fi
        
        if [ "$volatility" = "null" ] || (( $(echo "$volatility < 0" | bc -l) )); then
            print_warning "   • Trade $index: Negative volatility ($volatility)"
            ((warnings++))
        fi
        
        # Check market regime values
        case "$market_regime" in
            "Consolidating"|"Trending"|"Volatile")
                ;;
            *)
                print_warning "   • Trade $index: Unknown market regime ($market_regime)"
                ((warnings++))
                ;;
        esac
        
        # Check time consistency
        if [ "$entry_time" != "null" ] && [ "$exit_time" != "null" ]; then
            local entry_timestamp=$(date -d "$entry_time" +%s 2>/dev/null || echo "0")
            local exit_timestamp=$(date -d "$exit_time" +%s 2>/dev/null || echo "0")
            
            if [ "$exit_timestamp" -le "$entry_timestamp" ]; then
                print_error "   • Trade $index: Exit time before or equal to entry time"
                ((errors++))
            fi
        fi
        
        ((index++))
    done
    
    if [ "$errors" -eq 0 ]; then
        print_success "✅ All trade data is valid!"
    else
        print_error "❌ Data validation errors found:"
    fi
    
    if [ "$warnings" -gt 0 ]; then
        print_warning "\n$WARNING Warnings found:"
    fi
    
    print_header "\n📊 Verification Summary:"
    echo "   Total trades checked: $count"
    echo "   Errors: $errors"
    echo "   Warnings: $warnings"
}

# Function to export trades
export_trades() {
    local trades="$1"
    local format_type="$2"
    local filename="$3"
    
    local count=$(echo "$trades" | jq 'length')
    
    if [ "$count" -eq 0 ]; then
        print_warning "📭 No trades to export."
        return
    fi
    
    if [ -z "$filename" ]; then
        local pair=$(echo "$trades" | jq -r '.[0].pair')
        local timestamp=$(date +"%Y%m%d_%H%M%S")
        filename="ml_trades_${pair}_${timestamp}"
    fi
    
    case "$format_type" in
        "csv")
            filename="${filename}.csv"
            echo "id,pair,entry_price,exit_price,pnl,duration_seconds,entry_time,exit_time,success,market_regime,trend_strength,volatility,created_at" > "$filename"
            echo "$trades" | jq -r '.[] | [.id, .pair, .entry_price, .exit_price, .pnl, .duration_seconds, .entry_time, .exit_time, .success, .market_regime, .trend_strength, .volatility, .created_at] | @csv' >> "$filename"
            print_success "📁 Exported $count trades to $filename"
            ;;
        "json")
            filename="${filename}.json"
            echo "$trades" > "$filename"
            print_success "📁 Exported $count trades to $filename"
            ;;
        *)
            print_error "❌ Unsupported export format: $format_type"
            ;;
    esac
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --pair PAIR           Trading pair (default: $DEFAULT_PAIR)"
    echo "  --limit N             Number of trades to fetch (default: $DEFAULT_LIMIT)"
    echo "  --database-url URL    Database service URL (default: $DEFAULT_DATABASE_URL)"
    echo "  --stats               Show ML trade statistics"
    echo "  --status              Show ML system status"
    echo "  --verify              Verify trade data integrity"
    echo "  --details             Show detailed trade information"
    echo "  --export FORMAT       Export trades to file (csv or json)"
    echo "  --output FILENAME     Output filename for export"
    echo "  --help                Show this help message"
    echo
    echo "Examples:"
    echo "  $0 --pair SOLUSDC --limit 20"
    echo "  $0 --pair SOLUSDC --stats"
    echo "  $0 --pair SOLUSDC --verify"
    echo "  $0 --pair SOLUSDC --export csv"
    echo "  $0 --pair SOLUSDC --export json --output my_trades.json"
}

# Main function
main() {
    # Check dependencies
    check_dependencies
    
    # Parse command line arguments
    PAIR="$DEFAULT_PAIR"
    LIMIT="$DEFAULT_LIMIT"
    DATABASE_URL="$DEFAULT_DATABASE_URL"
    SHOW_STATS=false
    SHOW_STATUS=false
    SHOW_VERIFY=false
    SHOW_DETAILS=false
    EXPORT_FORMAT=""
    OUTPUT_FILENAME=""
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --pair)
                PAIR="$2"
                shift 2
                ;;
            --limit)
                LIMIT="$2"
                shift 2
                ;;
            --database-url)
                DATABASE_URL="$2"
                shift 2
                ;;
            --stats)
                SHOW_STATS=true
                shift
                ;;
            --status)
                SHOW_STATUS=true
                shift
                ;;
            --verify)
                SHOW_VERIFY=true
                shift
                ;;
            --details)
                SHOW_DETAILS=true
                shift
                ;;
            --export)
                EXPORT_FORMAT="$2"
                shift 2
                ;;
            --output)
                OUTPUT_FILENAME="$2"
                shift 2
                ;;
            --help)
                show_usage
                exit 0
                ;;
            *)
                print_error "❌ Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
    
    # Remove trailing slash from database URL
    DATABASE_URL="${DATABASE_URL%/}"
    
    print_info "$SEARCH Querying ML trades for $PAIR..."
    print_info "🌐 Database URL: $DATABASE_URL"
    
    # Get ML trades
    local trades
    trades=$(get_ml_trades "$PAIR" "$LIMIT")
    
    if [ $? -ne 0 ] || [ -z "$trades" ]; then
        print_error "❌ No ML trades found for $PAIR"
        exit 1
    fi
    
    # Display trades
    display_trades "$trades"
    
    # Show statistics if requested
    if [ "$SHOW_STATS" = "true" ]; then
        local stats
        stats=$(get_ml_stats "$PAIR")
        if [ $? -eq 0 ]; then
            display_stats "$stats"
        fi
    fi
    
    # Show ML status if requested
    if [ "$SHOW_STATUS" = "true" ]; then
        local status
        status=$(get_ml_status)
        if [ $? -eq 0 ]; then
            display_ml_status "$status"
        fi
    fi
    
    # Verify data integrity if requested
    if [ "$SHOW_VERIFY" = "true" ]; then
        verify_trade_data "$trades"
    fi
    
    # Export if requested
    if [ -n "$EXPORT_FORMAT" ]; then
        export_trades "$trades" "$EXPORT_FORMAT" "$OUTPUT_FILENAME"
    fi
    
    print_success "\n✅ Query completed successfully!"
}

# Run main function
main "$@" 