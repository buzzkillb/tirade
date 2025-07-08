#!/bin/bash

# Tirade Trading Bot Suite - Stop All Services Script

echo "🛑 Stopping Tirade Trading Bot Suite..."
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

# Function to stop a service by PID file
stop_service() {
    local service_name=$1
    local pid_file=$2
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p $pid > /dev/null 2>&1; then
            print_status "Stopping $service_name (PID: $pid)..."
            kill $pid
            sleep 2
            if ps -p $pid > /dev/null 2>&1; then
                print_warning "$service_name didn't stop gracefully, force killing..."
                kill -9 $pid
            fi
            print_success "$service_name stopped"
        else
            print_warning "$service_name (PID: $pid) is not running"
        fi
        rm -f "$pid_file"
    else
        print_warning "PID file for $service_name not found"
    fi
}

# Stop services in reverse order
print_status "Stopping services..."

# Stop dashboard
stop_service "Dashboard" "logs/dashboard.pid"

# Stop trading logic
stop_service "Trading Logic" "logs/trading_logic.pid"

# Stop price feed
stop_service "Price Feed" "logs/price_feed.pid"

# Stop database service
stop_service "Database Service" "logs/database.pid"

# Clean up PID file
rm -f logs/pids.txt

echo ""
print_success "All Tirade services stopped!"
echo ""
echo "📁 Log files are preserved in the logs/ directory"
echo "🚀 To restart all services, run: ./start_all.sh" 