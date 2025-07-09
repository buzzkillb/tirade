#!/bin/bash

# Tirade Trading Bot Suite - Screen Session Management Script
# This script provides utilities to manage screen sessions

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${PURPLE}$1${NC}"
}

# Function to check if a screen session exists
screen_exists() {
    local session_name=$1
    screen -list | grep -q "$session_name"
}

# Function to show usage
show_usage() {
    echo "🛠️  Tirade Screen Session Manager"
    echo "=================================="
    echo ""
    echo "Usage: $0 [COMMAND] [SERVICE]"
    echo ""
    echo "Commands:"
    echo "  list, ls          - List all tirade screen sessions"
    echo "  status, st        - Show status of all services"
    echo "  attach, a         - Attach to a service session"
    echo "  logs, l           - Show logs for a service"
    echo "  restart, r        - Restart a specific service"
    echo "  kill, k           - Kill a specific service session"
    echo "  monitor, m        - Monitor all services in real-time"
    echo "  help, h           - Show this help message"
    echo ""
    echo "Services:"
    echo "  db, database      - Database service"
    echo "  price, pf         - Price feed service"
    echo "  trading, tl       - Trading logic service"
    echo "  dashboard, dash   - Dashboard service"
    echo "  all               - All services (for restart/kill)"
    echo ""
    echo "Examples:"
    echo "  $0 list                    # List all sessions"
    echo "  $0 attach db               # Attach to database session"
    echo "  $0 logs trading            # Show trading logic logs"
    echo "  $0 restart all             # Restart all services"
    echo "  $0 monitor                 # Monitor all services"
    echo ""
}

# Function to list all sessions
list_sessions() {
    print_header "📺 Tirade Screen Sessions"
    echo ""
    
    local sessions_found=false
    
    # Check each service
    declare -A services=(
        ["tirade-db"]="Database Service"
        ["tirade-price"]="Price Feed"
        ["tirade-trading"]="Trading Logic"
        ["tirade-dashboard"]="Dashboard"
    )
    
    for session_name in "${!services[@]}"; do
        service_name="${services[$session_name]}"
        if screen_exists "$session_name"; then
            echo -e "${GREEN}✅${NC} $service_name (${CYAN}$session_name${NC})"
            sessions_found=true
        else
            echo -e "${RED}❌${NC} $service_name (${CYAN}$session_name${NC}) - Not running"
        fi
    done
    
    echo ""
    if [ "$sessions_found" = false ]; then
        print_warning "No tirade screen sessions are currently running"
        echo "💡 To start all services: ./start_all_screen.sh"
    fi
}

# Function to show service status
show_status() {
    print_header "📊 Tirade Service Status"
    echo ""
    
    # Check database service
    if screen_exists "tirade-db"; then
        if curl -s "http://localhost:8080/health" > /dev/null 2>&1; then
            echo -e "${GREEN}✅${NC} Database Service - Running (Port 8080)"
        else
            echo -e "${YELLOW}⚠️${NC} Database Service - Screen running but not responding"
        fi
    else
        echo -e "${RED}❌${NC} Database Service - Not running"
    fi
    
    # Check price feed service
    if screen_exists "tirade-price"; then
        if lsof -Pi :8081 -sTCP:LISTEN -t >/dev/null 2>&1; then
            echo -e "${GREEN}✅${NC} Price Feed - Running (Port 8081)"
        else
            echo -e "${YELLOW}⚠️${NC} Price Feed - Screen running but port not listening"
        fi
    else
        echo -e "${RED}❌${NC} Price Feed - Not running"
    fi
    
    # Check trading logic service
    if screen_exists "tirade-trading"; then
        echo -e "${GREEN}✅${NC} Trading Logic - Running"
    else
        echo -e "${RED}❌${NC} Trading Logic - Not running"
    fi
    
    # Check dashboard service
    if screen_exists "tirade-dashboard"; then
        if curl -s "http://localhost:3000" > /dev/null 2>&1; then
            echo -e "${GREEN}✅${NC} Dashboard - Running (Port 3000)"
        else
            echo -e "${YELLOW}⚠️${NC} Dashboard - Screen running but not responding"
        fi
    else
        echo -e "${RED}❌${NC} Dashboard - Not running"
    fi
    
    echo ""
    echo "🌐 Dashboard URL: http://localhost:3000"
    echo "🗄️  Database API: http://localhost:8080"
}

# Function to attach to a session
attach_session() {
    local service=$1
    
    case $service in
        "db"|"database")
            if screen_exists "tirade-db"; then
                print_status "Attaching to Database Service session..."
                screen -r tirade-db
            else
                print_error "Database Service session not found"
            fi
            ;;
        "price"|"pf")
            if screen_exists "tirade-price"; then
                print_status "Attaching to Price Feed session..."
                screen -r tirade-price
            else
                print_error "Price Feed session not found"
            fi
            ;;
        "trading"|"tl")
            if screen_exists "tirade-trading"; then
                print_status "Attaching to Trading Logic session..."
                screen -r tirade-trading
            else
                print_error "Trading Logic session not found"
            fi
            ;;
        "dashboard"|"dash")
            if screen_exists "tirade-dashboard"; then
                print_status "Attaching to Dashboard session..."
                screen -r tirade-dashboard
            else
                print_error "Dashboard session not found"
            fi
            ;;
        *)
            print_error "Unknown service: $service"
            echo "Available services: db, price, trading, dashboard"
            ;;
    esac
}

# Function to show logs
show_logs() {
    local service=$1
    
    case $service in
        "db"|"database")
            if [ -f "logs/database.log" ]; then
                print_status "Showing Database Service logs..."
                tail -f logs/database.log
            else
                print_error "Database log file not found"
            fi
            ;;
        "price"|"pf")
            if [ -f "logs/price_feed.log" ]; then
                print_status "Showing Price Feed logs..."
                tail -f logs/price_feed.log
            else
                print_error "Price Feed log file not found"
            fi
            ;;
        "trading"|"tl")
            if [ -f "logs/trading_logic.log" ]; then
                print_status "Showing Trading Logic logs..."
                tail -f logs/trading_logic.log
            else
                print_error "Trading Logic log file not found"
            fi
            ;;
        "dashboard"|"dash")
            if [ -f "logs/dashboard.log" ]; then
                print_status "Showing Dashboard logs..."
                tail -f logs/dashboard.log
            else
                print_error "Dashboard log file not found"
            fi
            ;;
        *)
            print_error "Unknown service: $service"
            echo "Available services: db, price, trading, dashboard"
            ;;
    esac
}

# Function to restart a service
restart_service() {
    local service=$1
    
    case $service in
        "db"|"database")
            print_status "Restarting Database Service..."
            screen -S tirade-db -X quit 2>/dev/null || true
            sleep 2
            cd database-service
            screen -dmS tirade-db bash -c "DATABASE_URL=\"sqlite:../data/trading_bot.db\" cargo run; exec bash"
            cd ..
            print_success "Database Service restarted"
            ;;
        "price"|"pf")
            print_status "Restarting Price Feed..."
            screen -S tirade-price -X quit 2>/dev/null || true
            sleep 2
            cd price-feed
            screen -dmS tirade-price bash -c "cargo run; exec bash"
            cd ..
            print_success "Price Feed restarted"
            ;;
        "trading"|"tl")
            print_status "Restarting Trading Logic..."
            screen -S tirade-trading -X quit 2>/dev/null || true
            sleep 2
            screen -dmS tirade-trading bash -c "cargo run --bin trading-logic; exec bash"
            print_success "Trading Logic restarted"
            ;;
        "dashboard"|"dash")
            print_status "Restarting Dashboard..."
            screen -S tirade-dashboard -X quit 2>/dev/null || true
            sleep 2
            cd dashboard
            screen -dmS tirade-dashboard bash -c "DATABASE_URL=\"http://localhost:8080\" cargo run; exec bash"
            cd ..
            print_success "Dashboard restarted"
            ;;
        "all")
            print_status "Restarting all services..."
            ./stop_all_screen.sh
            sleep 3
            ./start_all_screen.sh
            ;;
        *)
            print_error "Unknown service: $service"
            echo "Available services: db, price, trading, dashboard, all"
            ;;
    esac
}

# Function to kill a service
kill_service() {
    local service=$1
    
    case $service in
        "db"|"database")
            print_status "Killing Database Service..."
            screen -S tirade-db -X quit 2>/dev/null || true
            print_success "Database Service killed"
            ;;
        "price"|"pf")
            print_status "Killing Price Feed..."
            screen -S tirade-price -X quit 2>/dev/null || true
            print_success "Price Feed killed"
            ;;
        "trading"|"tl")
            print_status "Killing Trading Logic..."
            screen -S tirade-trading -X quit 2>/dev/null || true
            print_success "Trading Logic killed"
            ;;
        "dashboard"|"dash")
            print_status "Killing Dashboard..."
            screen -S tirade-dashboard -X quit 2>/dev/null || true
            print_success "Dashboard killed"
            ;;
        "all")
            print_status "Killing all services..."
            ./stop_all_screen.sh
            ;;
        *)
            print_error "Unknown service: $service"
            echo "Available services: db, price, trading, dashboard, all"
            ;;
    esac
}

# Function to monitor all services
monitor_services() {
    print_header "📊 Real-time Service Monitor"
    echo "Press Ctrl+C to exit"
    echo ""
    
    while true; do
        clear
        show_status
        echo ""
        echo "🔄 Refreshing in 5 seconds..."
        sleep 5
    done
}

# Main script logic
case $1 in
    "list"|"ls")
        list_sessions
        ;;
    "status"|"st")
        show_status
        ;;
    "attach"|"a")
        if [ -z "$2" ]; then
            print_error "Please specify a service to attach to"
            echo "Usage: $0 attach [db|price|trading|dashboard]"
        else
            attach_session "$2"
        fi
        ;;
    "logs"|"l")
        if [ -z "$2" ]; then
            print_error "Please specify a service for logs"
            echo "Usage: $0 logs [db|price|trading|dashboard]"
        else
            show_logs "$2"
        fi
        ;;
    "restart"|"r")
        if [ -z "$2" ]; then
            print_error "Please specify a service to restart"
            echo "Usage: $0 restart [db|price|trading|dashboard|all]"
        else
            restart_service "$2"
        fi
        ;;
    "kill"|"k")
        if [ -z "$2" ]; then
            print_error "Please specify a service to kill"
            echo "Usage: $0 kill [db|price|trading|dashboard|all]"
        else
            kill_service "$2"
        fi
        ;;
    "monitor"|"m")
        monitor_services
        ;;
    "help"|"h"|"")
        show_usage
        ;;
    *)
        print_error "Unknown command: $1"
        echo ""
        show_usage
        exit 1
        ;;
esac 