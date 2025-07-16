#!/bin/bash

echo "🚀 Starting Terminal Dashboard..."
echo "═══════════════════════════════════════════════════════════════"

# Set environment variables
export DATABASE_SERVICE_URL="http://localhost:8080"

# Navigate to dashboard directory
cd dashboard

# Build and run the dashboard
echo "📦 Building dashboard..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "🌐 Starting dashboard on http://localhost:3000"
    echo "═══════════════════════════════════════════════════════════════"
    cargo run --release
else
    echo "❌ Build failed!"
    exit 1
fi