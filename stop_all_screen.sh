#!/bin/bash

# Tirade Trading Bot Suite - Screen-based Stop Script
# This script stops all services running in screen sessions

set -e

echo "🛑 Stopping Tirade Trading Bot Suite Screen Sessions..."
echo "======================================================="
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

# Function to check if a screen session exists
screen_exists() {
    local session_name=$1
    screen -list | grep -q "$session_name"
}

# Function to stop a screen session
stop_screen_session() {
    local session_name=$1
    local service_name=$2
    
    if screen_exists "$session_name"; then
        print_status "Stopping $service_name (session: $session_name)..."
        screen -S "$session_name" -X quit
        sleep 1
        
        # Verify session was stopped
        if ! screen_exists "$session_name"; then
            print_success "$service_name stopped successfully"
        else
            print_warning "$service_name may still be running"
        fi
    else
        print_warning "$service_name session ($session_name) not found"
    fi
}

# List of services to stop
declare -A services=(
    ["tirade-db"]="Database Service"
    ["tirade-price"]="Price Feed"
    ["tirade-trading"]="Trading Logic"
    ["tirade-dashboard"]="Dashboard"
)

# Stop all services
for session_name in "${!services[@]}"; do
    service_name="${services[$session_name]}"
    stop_screen_session "$session_name" "$service_name"
done

# Check if any tirade screen sessions are still running
remaining_sessions=$(screen -list | grep "tirade-" || true)

if [ -n "$remaining_sessions" ]; then
    print_warning "Some tirade sessions may still be running:"
    echo "$remaining_sessions"
    echo ""
    print_status "To force kill all remaining sessions:"
    echo "screen -list | grep 'tirade-' | awk '{print \$1}' | xargs -I {} screen -S {} -X quit"
else
    print_success "All tirade screen sessions have been stopped"
fi

# Clean up any remaining processes that might be using the ports
print_status "Checking for any remaining processes on tirade ports..."

# Check port 8080 (database)
if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null 2>&1; then
    print_warning "Port 8080 (database) is still in use"
    lsof -Pi :8080 -sTCP:LISTEN
fi

# Check port 8081 (price feed)
if lsof -Pi :8081 -sTCP:LISTEN -t >/dev/null 2>&1; then
    print_warning "Port 8081 (price feed) is still in use"
    lsof -Pi :8081 -sTCP:LISTEN
fi

# Check port 3000 (dashboard)
if lsof -Pi :3000 -sTCP:LISTEN -t >/dev/null 2>&1; then
    print_warning "Port 3000 (dashboard) is still in use"
    lsof -Pi :3000 -sTCP:LISTEN
fi

echo ""
echo "🎉 Tirade Trading Bot Suite has been stopped!"
echo "============================================="
echo ""
print_success "All screen sessions have been terminated"
echo ""
echo "💡 To restart all services: ./start_all_screen.sh"
echo "💡 To view remaining screen sessions: screen -list" 