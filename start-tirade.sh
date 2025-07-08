#!/bin/bash

# Tirade Startup Script
# Run this from the main /tirade folder

echo "🚀 Starting Tirade Trading System..."
echo "📁 Working Directory: $(pwd)"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the main tirade folder"
    echo "   Current directory: $(pwd)"
    echo "   Expected: /path/to/tirade/"
    exit 1
fi

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo "❌ Error: .env file not found in $(pwd)"
    echo "   Please copy your .env file to this directory"
    exit 1
fi

echo "✅ Environment check passed"
echo ""

# Initialize database if needed
echo "🗄️  Checking database initialization..."
if [ ! -f "data/trading_bot.db" ]; then
    echo "   Database not found, initializing..."
    ./init-database.sh
    echo "   ✅ Database initialized"
else
    echo "   ✅ Database already exists"
fi
echo ""

# Function to start a service
start_service() {
    local service_name=$1
    local binary_name=$2
    local port=$3
    
    echo "🔄 Starting $service_name..."
    echo "   Binary: $binary_name"
    echo "   Port: $port"
    
    # Start the service in background
    DATABASE_URL="http://localhost:8080" cargo run --bin $binary_name &
    local pid=$!
    
    echo "   ✅ $service_name started (PID: $pid)"
    echo ""
    
    # Wait a bit for service to initialize
    sleep 3
    
    return $pid
}

# Start services in order
echo "📊 Starting Database Service..."
start_service "Database Service" "database-service" "8080"
DB_PID=$!

echo "📈 Starting Price Feed..."
start_service "Price Feed" "price-feed" "8081"
PRICE_PID=$!

echo "🧠 Starting Trading Logic..."
start_service "Trading Logic" "trading-logic" "N/A"
TRADING_PID=$!

echo "🌐 Starting Dashboard..."
echo "   Note: Dashboard will bind to localhost (127.0.0.1) for security"
# Start dashboard with localhost binding
DATABASE_URL="http://localhost:8080" RUST_LOG=info cargo run --bin dashboard -- --host 127.0.0.1 &
DASHBOARD_PID=$!

echo ""
echo "🎉 All services started successfully!"
echo ""
echo "📊 Service Status:"
echo "   Database Service: PID $DB_PID (Port 8080)"
echo "   Price Feed: PID $PRICE_PID (Port 8081)"
echo "   Trading Logic: PID $TRADING_PID"
echo "   Dashboard: PID $DASHBOARD_PID (Port 3000)"
echo ""
echo "🌐 Dashboard URL: http://127.0.0.1:3000 (localhost only)"
echo "📊 Database API: http://localhost:8080"
echo ""
echo "💡 To stop all services:"
echo "   pkill -f 'cargo run'"
echo ""
echo "💡 To view logs:"
echo "   tail -f /var/log/syslog | grep -E '(trading|dashboard|price)'"
echo ""

# Wait for user to stop
echo "⏳ Services are running... Press Ctrl+C to stop all services"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "🛑 Stopping all services..."
    pkill -f 'cargo run'
    echo "✅ All services stopped"
    exit 0
}

# Set up signal handlers
trap cleanup SIGINT SIGTERM

# Keep script running
while true; do
    sleep 1
done 