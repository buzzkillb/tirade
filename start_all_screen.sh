#!/bin/bash

# Tirade Trading Bot Suite - Screen-based Startup Script
# This script starts all services in separate screen sessions

set -e

echo "🚀 Starting Tirade Trading Bot Suite with Screen Sessions..."
echo "============================================================="
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

# Check if screen is installed
if ! command -v screen &> /dev/null; then
    print_error "Screen is not installed!"
    print_status "Please install screen: sudo apt-get install screen"
    exit 1
fi

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

# Create logs directory if it doesn't exist
mkdir -p logs

# Function to check if a screen session exists
screen_exists() {
    local session_name=$1
    screen -list | grep -q "$session_name"
}

# Function to kill a screen session if it exists
kill_screen_session() {
    local session_name=$1
    if screen_exists "$session_name"; then
        print_status "Killing existing screen session: $session_name"
        screen -S "$session_name" -X quit
        sleep 1
    fi
}

# Function to start a service in a screen session
start_service_in_screen() {
    local service_name=$1
    local session_name=$2
    local working_dir=$3
    local command=$4
    local port=$5
    
    print_status "Starting $service_name in screen session: $session_name"
    
    # Kill existing session if it exists
    kill_screen_session "$session_name"
    
    # Create new screen session
    if [ -n "$working_dir" ]; then
        cd "$working_dir"
    fi
    
    # Start the service in a new screen session
    screen -dmS "$session_name" bash -c "$command; exec bash"
    
    # Return to original directory
    if [ -n "$working_dir" ]; then
        cd ..
    fi
    
    sleep 2
    
    # Check if screen session was created successfully
    if screen_exists "$session_name"; then
        print_success "$service_name started in screen session: $session_name"
        return 0
    else
        print_error "Failed to start $service_name in screen session"
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
start_service_in_screen "Database Service" "tirade-db" "database-service" \
    "DATABASE_URL=\"sqlite:../data/trading_bot.db\" cargo run" "8080"

# Wait for database service
wait_for_service "Database Service" "http://localhost:8080/health"

# Start price feed
print_status "Starting price feed..."
start_service_in_screen "Price Feed" "tirade-price" "price-feed" \
    "cargo run" "8081"

# Start trading logic
print_status "Starting trading logic..."
start_service_in_screen "Trading Logic" "tirade-trading" "." \
    "cargo run --bin trading-logic" "N/A"

# Start dashboard
print_status "Starting dashboard..."
start_service_in_screen "Dashboard" "tirade-dashboard" "dashboard" \
    "DATABASE_URL=\"http://localhost:8080\" cargo run" "3000"

# Wait for dashboard
wait_for_service "Dashboard" "http://localhost:3000"

# Save session names to a file for easy management
cat > logs/screen_sessions.txt << EOF
Database Service: tirade-db
Price Feed: tirade-price
Trading Logic: tirade-trading
Dashboard: tirade-dashboard
EOF

echo ""
echo "🎉 Tirade Trading Bot Suite is now running in screen sessions!"
echo "============================================================="
echo ""
echo "📊 Dashboard: http://localhost:3000"
echo "🗄️  Database API: http://localhost:8080"
echo "📡 Price Feed: Running on port 8081"
echo "🧠 Trading Logic: Running and analyzing markets"
echo ""
echo "📺 Screen Sessions:"
echo "   - Database: tirade-db"
echo "   - Price Feed: tirade-price"
echo "   - Trading Logic: tirade-trading"
echo "   - Dashboard: tirade-dashboard"
echo ""
echo "🛠️  Screen Commands:"
echo "   - List sessions: screen -list"
echo "   - Attach to session: screen -r tirade-db"
echo "   - Detach from session: Ctrl+A, then D"
echo "   - Kill session: screen -S tirade-db -X quit"
echo ""
echo "🛑 To stop all services: ./stop_all_screen.sh"
echo "📋 To view logs: tail -f logs/trading_logic.log"
echo ""
print_success "All services started successfully in screen sessions!" 