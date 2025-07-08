#!/bin/bash

echo "🛑 Stopping Tirade Trading System..."
echo ""

# Function to stop a service
stop_service() {
    local service_name=$1
    local binary_name=$2
    
    echo "🔄 Stopping $service_name..."
    
    # Find and kill the process
    local pids=$(pgrep -f "$binary_name")
    if [ -n "$pids" ]; then
        echo "   Found PIDs: $pids"
        kill $pids
        echo "   ✅ Sent stop signal to $service_name"
    else
        echo "   ℹ️  $service_name not running"
    fi
    echo ""
}

# Stop services in reverse order
echo "🌐 Stopping Dashboard..."
stop_service "Dashboard" "dashboard"

echo "🧠 Stopping Trading Logic..."
stop_service "Trading Logic" "trading-logic"

echo "📈 Stopping Price Feed..."
stop_service "Price Feed" "price-feed"

echo "📊 Stopping Database Service..."
stop_service "Database Service" "database-service"

# Wait a moment for graceful shutdown
echo "⏳ Waiting for graceful shutdown..."
sleep 3

# Force kill any remaining processes
echo "🔍 Checking for remaining processes..."
remaining=$(pgrep -f "cargo run")
if [ -n "$remaining" ]; then
    echo "   ⚠️  Force killing remaining processes: $remaining"
    kill -9 $remaining
    echo "   ✅ Force killed remaining processes"
else
    echo "   ✅ All processes stopped gracefully"
fi

echo ""
echo "🎉 All Tirade services stopped!"
echo ""
echo "💡 To verify all processes are stopped:"
echo "   ps aux | grep cargo"
echo "" 