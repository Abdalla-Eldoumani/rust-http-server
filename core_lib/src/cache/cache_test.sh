#!/bin/bash

echo "=== COMPREHENSIVE CACHE TEST ==="
echo ""

echo "Starting server..."
RUST_LOG=debug cargo run --bin server &
SERVER_PID=$!
sleep 6

echo ""
echo "1. Testing /api/items endpoint"
echo "------------------------------"
echo "First request (should be MISS):"
curl -v http://localhost:3000/api/items 2>&1 | grep -E "(< |> )" | head -20
echo "Second request (should be HIT):"
curl -v http://localhost:3000/api/items 2>&1 | grep -E "(< |> )" | head -20

echo ""
echo "2. Testing /health endpoint"
echo "---------------------------"
echo "First request (should be MISS):"
curl -v http://localhost:3000/health 2>&1 | grep -E "(< |> )" | head -20
echo "Second request (should be HIT):"
curl -v http://localhost:3000/health 2>&1 | grep -E "(< |> )" | head -20

echo ""
echo "3. Testing query parameters"
echo "----------------------------"
echo "Request with page=1 (should be MISS):"
curl -v "http://localhost:3000/api/items?page=1" 2>&1 | grep -E "(< |> )" | head -20
echo "Same request again (should be HIT):"
curl -v "http://localhost:3000/api/items?page=1" 2>&1 | grep -E "(< |> )" | head -20
echo "Request with page=2 (should be MISS):"
curl -v "http://localhost:3000/api/items?page=2" 2>&1 | grep -E "(< |> )" | head -20

echo ""
echo "4. Testing POST requests (should not be cached)"
echo "-----------------------------------------------"
echo "POST request (should be MISS):"
curl -s -X POST -H "Content-Type: application/json" -d '{"name":"test"}' -I http://localhost:3000/api/items 2>&1 | grep -E "(< |> )" | head -20
echo "Same POST again (should still be MISS):"
curl -s -X POST -H "Content-Type: application/json" -d '{"name":"test"}' -I http://localhost:3000/api/items 2>&1 | grep -E "(< |> )" | head -20

echo ""
echo "Stopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo ""
echo "=== CACHE TEST COMPLETED ==="