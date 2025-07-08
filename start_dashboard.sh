#!/bin/bash

echo "🚀 Starting Tirade Dashboard..."
echo "📊 Dashboard will be available at: http://localhost:3000"
echo ""

# Set environment variables
export DATABASE_URL="http://localhost:8080"

# Start the dashboard
cd dashboard
cargo run 