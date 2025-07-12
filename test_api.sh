#!/bin/bash

BASE_URL="http://localhost:3000"

echo "=== Testing Rust HTTP Server API ==="
echo

echo "1. Testing root endpoint (GET /):"
curl -s $BASE_URL/
echo

echo "2. Testing health endpoint (GET /health):"
curl -s $BASE_URL/health
echo

echo "3. Testing stats endpoint (GET /api/stats):"
curl -s $BASE_URL/api/stats
echo

echo "4. Testing list items (GET /api/items):"
curl -s $BASE_URL/api/items
echo

echo "5. Testing list items with pagination (GET /api/items?limit=1&offset=1):"
curl -s "$BASE_URL/api/items?limit=1&offset=1"
echo

echo "6. Testing get item (GET /api/items/1):"
curl -s $BASE_URL/api/items/1
echo

echo "7. Testing create item (POST /api/items):"
curl -s -X POST $BASE_URL/api/items \
  -H "Content-Type: application/json" \
  -d '{
    "name": "New Test Item",
    "description": "Created via API test",
    "tags": ["test", "api", "new"],
    "metadata": {"priority": "high", "category": "testing"}
  }'
echo

echo "8. Testing update item (PUT /api/items/1):"
curl -s -X PUT $BASE_URL/api/items/1 \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Updated Item Name",
    "description": "This item has been updated",
    "tags": ["updated", "modified"],
    "metadata": {"status": "updated", "version": 2}
  }'
echo

echo "9. Testing patch item (PATCH /api/items/2):"
curl -s -X PATCH $BASE_URL/api/items/2 \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Partially Updated Item",
    "tags": ["patched"]
  }'
echo

echo "10. Testing form submission (POST /api/form):"
curl -s -X POST $BASE_URL/api/form \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "name=John%20Doe&email=john@example.com&message=Hello%20from%20form"
echo

echo "11. Testing HEAD request (HEAD /api/head):"
curl -I -s $BASE_URL/api/head
echo

echo "12. Testing OPTIONS request (OPTIONS /api/options):"
curl -s -X OPTIONS $BASE_URL/api/options
echo

echo "13. Testing delete item (DELETE /api/items/3):"
curl -s -X DELETE $BASE_URL/api/items/3
echo

echo "14. Testing error handling - invalid ID (GET /api/items/0):"
curl -s $BASE_URL/api/items/0
echo

echo "15. Testing error handling - non-existent item (GET /api/items/999):"
curl -s $BASE_URL/api/items/999
echo

echo "16. Final stats after all operations:"
curl -s $BASE_URL/api/stats