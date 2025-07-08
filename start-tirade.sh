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

# Ensure database file has proper permissions
if [ -f "data/trading_bot.db" ]; then
    chmod 644 data/trading_bot.db
    echo "   ✅ Database permissions set"
fi

# Ensure data directory exists and has proper permissions
if [ ! -d "data" ]; then
    mkdir -p data
    echo "   ✅ Data directory created"
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
    echo "   🔄 Starting $binary_name..."
    DATABASE_URL="http://localhost:8080" cargo run --bin $binary_name &
    local pid=$!
    
    # Check if the process started successfully
    sleep 2
    if ! kill -0 $pid 2>/dev/null; then
        echo "   ❌ Failed to start $service_name"
        return 1
    fi
    
    echo "   ✅ $service_name started (PID: $pid)"
    echo ""
    
    # Wait longer for service to initialize and be ready
    echo "   ⏳ Waiting for $service_name to be ready..."
    sleep 5
    
    # Check if service is responding (only for database service which has health endpoint)
    if [ "$port" = "8080" ]; then
        local max_attempts=10
        local attempt=1
        while [ $attempt -le $max_attempts ]; do
            echo "   🔍 Testing connection to http://localhost:$port/health..."
            if curl -s "http://localhost:$port/health" > /dev/null 2>&1; then
                echo "   ✅ $service_name is ready and responding"
                break
            else
                echo "   ⏳ Attempt $attempt/$max_attempts: $service_name not ready yet..."
                echo "   🔍 Debug: curl exit code: $?"
                sleep 2
                attempt=$((attempt + 1))
            fi
        done
        
        if [ $attempt -gt $max_attempts ]; then
            echo "   ⚠️  Warning: $service_name may not be fully ready, but continuing..."
            echo "   🔍 Debug: Service may need more time to start"
        fi
    else
        echo "   ⏳ Waiting for $service_name to initialize..."
        sleep 3
        echo "   ✅ $service_name started (no health check available)"
    fi
    
    echo ""
    echo $pid
}

# Start services in order
echo "📊 Starting Database Service..."
DB_PID=$(start_service "Database Service" "database-service" "8080")
if [ $? -ne 0 ]; then
    echo "❌ Failed to start Database Service"
    exit 1
fi

echo "📈 Starting Price Feed..."
echo "   ⏳ Ensuring database is ready before starting price feed..."
sleep 5
PRICE_PID=$(start_service "Price Feed" "price-feed" "8081")
if [ $? -ne 0 ]; then
    echo "❌ Failed to start Price Feed"
    echo "🛑 Stopping Database Service..."
    kill $DB_PID 2>/dev/null
    exit 1
fi

echo "🧠 Starting Trading Logic..."
TRADING_PID=$(start_service "Trading Logic" "trading-logic" "N/A")
if [ $? -ne 0 ]; then
    echo "❌ Failed to start Trading Logic"
    echo "🛑 Stopping other services..."
    kill $DB_PID $PRICE_PID 2>/dev/null
    exit 1
fi

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