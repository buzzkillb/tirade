#!/bin/bash

# Tirade Trading Bot Suite - Complete Startup Script
# This script starts all services for the Tirade trading bot system

set -e

echo "🚀 Starting Tirade Trading Bot Suite..."
echo "========================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

# Check if .env file exists
if [ ! -f ".env" ]; then
    print_error ".env file not found!"
    print_status "Please copy env.example to .env and configure your settings"
    exit 1
fi

# Load environment variables
print_status "Loading environment variables..."
source .env

# Check if database directory exists
if [ ! -d "data" ]; then
    print_status "Creating data directory..."
    mkdir -p data
fi

# Function to check if a port is in use
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null ; then
        return 0
    else
        return 1
    fi
}

# Function to wait for a service to be ready
wait_for_service() {
    local service_name=$1
    local url=$2
    local max_attempts=30
    local attempt=1
    
    print_status "Waiting for $service_name to be ready..."
    
    while [ $attempt -le $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            print_success "$service_name is ready!"
            return 0
        fi
        
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    print_error "$service_name failed to start within expected time"
    return 1
}

# Start database service
print_status "Starting database service..."
if check_port 8080; then
    print_warning "Port 8080 is already in use. Database service may already be running."
else
    cd database-service
    DATABASE_URL="sqlite:../data/trading_bot.db" cargo run > ../logs/database.log 2>&1 &
    DATABASE_PID=$!
    cd ..
    echo $DATABASE_PID > logs/database.pid
    print_success "Database service started (PID: $DATABASE_PID)"
fi

# Wait for database service
wait_for_service "Database Service" "http://localhost:8080/health"

# Start price feed
print_status "Starting price feed..."
if check_port 8081; then
    print_warning "Port 8081 is already in use. Price feed may already be running."
else
    cd price-feed
    cargo run > ../logs/price_feed.log 2>&1 &
    PRICE_FEED_PID=$!
    cd ..
    echo $PRICE_FEED_PID > logs/price_feed.pid
    print_success "Price feed started (PID: $PRICE_FEED_PID)"
fi

# Start trading logic
print_status "Starting trading logic..."
cd trading-logic
cargo run > ../logs/trading_logic.log 2>&1 &
TRADING_LOGIC_PID=$!
cd ..
echo $TRADING_LOGIC_PID > logs/trading_logic.pid
print_success "Trading logic started (PID: $TRADING_LOGIC_PID)"

# Start dashboard
print_status "Starting dashboard..."
if check_port 3000; then
    print_warning "Port 3000 is already in use. Dashboard may already be running."
else
    cd dashboard
    DATABASE_URL="http://localhost:8080" cargo run > ../logs/dashboard.log 2>&1 &
    DASHBOARD_PID=$!
    cd ..
    echo $DASHBOARD_PID > logs/dashboard.pid
    print_success "Dashboard started (PID: $DASHBOARD_PID)"
fi

# Wait for dashboard
wait_for_service "Dashboard" "http://localhost:3000"

# Create logs directory if it doesn't exist
mkdir -p logs

# Save PIDs to a file for easy cleanup
cat > logs/pids.txt << EOF
Database Service: $DATABASE_PID
Price Feed: $PRICE_FEED_PID
Trading Logic: $TRADING_LOGIC_PID
Dashboard: $DASHBOARD_PID
EOF

echo ""
echo "🎉 Tirade Trading Bot Suite is now running!"
echo "========================================"
echo ""
echo "📊 Dashboard: http://localhost:3000"
echo "🗄️  Database API: http://localhost:8080"
echo "📡 Price Feed: Running on port 8081"
echo "🧠 Trading Logic: Running and analyzing markets"
echo ""
echo "📁 Logs are available in the logs/ directory:"
echo "   - Database: logs/database.log"
echo "   - Price Feed: logs/price_feed.log"
echo "   - Trading Logic: logs/trading_logic.log"
echo "   - Dashboard: logs/dashboard.log"
echo ""
echo "🛑 To stop all services, run: ./stop_all.sh"
echo "📋 To view logs: tail -f logs/trading_logic.log"
echo ""
print_success "All services started successfully!" 