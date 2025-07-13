#!/bin/bash

BASE_URL="http://localhost:3000"

echo "=== Testing New Features ==="
echo

echo "1. Testing Metrics API:"
curl -s $BASE_URL/api/metrics
echo

echo "2. Testing Export - JSON format:"
curl -s "$BASE_URL/api/items/export?format=json" -o items.json
echo "Exported to items.json"
echo

echo "3. Testing Export - CSV format:"
curl -s "$BASE_URL/api/items/export?format=csv" -o items.csv
echo "Exported to items.csv"
cat items.csv
echo

echo "4. Testing Export - YAML format:"
curl -s "$BASE_URL/api/items/export?format=yaml" -o items.yaml
echo "Exported to items.yaml"
echo

echo "5. Testing Rate Limiting (making rapid requests):"
for i in {1..10}; do
    echo -n "Request $i: "
    curl -s -o /dev/null -w "%{http_code} - Remaining: " $BASE_URL/api/items
    curl -s -D - $BASE_URL/api/items | grep -i "x-ratelimit-remaining" | cut -d' ' -f2
done
echo

echo "6. Dashboard is available at: $BASE_URL/dashboard"