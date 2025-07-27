#!/bin/bash

BASE_URL="http://localhost:3000"
JWT_TOKEN=""

echo "=== Testing Advanced Rust HTTP Server API ==="
echo

extract_token() {
    echo "$1" | grep -o '"token":"[^"]*' | cut -d'"' -f4
}

echo "=== BASIC ENDPOINTS ==="
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

echo "4. Testing metrics endpoint (GET /api/metrics):"
curl -s $BASE_URL/api/metrics
echo

echo "=== AUTHENTICATION TESTS ==="
echo

echo "5. Testing user registration (POST /auth/register):"
REGISTER_RESPONSE=$(curl -v -X POST $BASE_URL/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "testpassword123",
    "password_confirmation": "testpassword123"
  }')
echo $REGISTER_RESPONSE
JWT_TOKEN=$(extract_token "$REGISTER_RESPONSE")
echo "Extracted JWT Token: ${JWT_TOKEN:0:50}..."
echo

echo "6. Testing user login (POST /auth/login):"
LOGIN_RESPONSE=$(curl -s -X POST $BASE_URL/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "testpassword123"
  }')
echo $LOGIN_RESPONSE
JWT_TOKEN=$(extract_token "$LOGIN_RESPONSE")
echo "Updated JWT Token: ${JWT_TOKEN:0:50}..."
echo

echo "=== ITEMS API v1 TESTS ==="
echo

echo "7. Testing list items v1 (GET /api/v1/items):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items
echo

echo "8. Testing list items v1 with pagination (GET /api/v1/items?page=1&limit=5):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/v1/items?page=1&limit=5"
echo

echo "9. Testing create item v1 (POST /api/v1/items):"
CREATE_RESPONSE=$(curl -s -X POST $BASE_URL/api/v1/items \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Test Item v1",
    "description": "Created via API test v1"
  }')
echo $CREATE_RESPONSE
ITEM_ID=$(echo $CREATE_RESPONSE)
echo "Created item ID: $ITEM_ID"
echo

echo "10. Testing get item v1 (GET /api/v1/items/$ITEM_ID):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items/$ITEM_ID
echo

echo "11. Testing update item v1 (PUT /api/v1/items/$ITEM_ID):"
curl -s -X PUT $BASE_URL/api/v1/items/$ITEM_ID \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Updated Test Item v1",
    "description": "Updated via API test v1"
  }'
echo

echo "12. Testing patch item v1 (PATCH /api/v1/items/$ITEM_ID):"
curl -s -X PATCH $BASE_URL/api/v1/items/$ITEM_ID \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Patched description v1"
  }'
echo

echo "=== ITEMS API v2 TESTS ==="
echo

echo "13. Testing create item v2 with enhanced features (POST /api/v2/items):"
CREATE_V2_RESPONSE=$(curl -s -X POST $BASE_URL/api/v2/items \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Enhanced Test Item v2",
    "description": "Created via API test v2 with enhanced features",
    "tags": ["test", "api", "v2", "enhanced"],
    "metadata": {"priority": "high", "category": "testing", "version": "2.0"}
  }')
echo $CREATE_V2_RESPONSE
ITEM_V2_ID=$(echo $CREATE_V2_RESPONSE)
echo "Created v2 item ID: $ITEM_V2_ID"
echo

echo "14. Testing list items v2 with enhanced features (GET /api/v2/items):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/v2/items?include_files=true"
echo

echo "=== SEARCH API TESTS ==="
echo

echo "15. Testing basic search (GET /api/items/search):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?q=test"
echo

echo "16. Testing advanced search with filters (GET /api/items/search):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?tags=test,api&sort=created_at&order=desc"
echo

echo "17. Testing fuzzy search (GET /api/items/search):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?q=tset&fuzzy=true"
echo

echo "=== FILE MANAGEMENT TESTS ==="
echo

echo "18. Testing file upload (POST /api/files/upload):"
echo "Creating test file..."
echo "This is a test file for upload" > test_upload.txt
UPLOAD_RESPONSE=$(curl -s -X POST $BASE_URL/api/files/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@test_upload.txt" \
  -F "item_id=$ITEM_V2_ID")
echo $UPLOAD_RESPONSE
FILE_ID=$(echo $UPLOAD_RESPONSE)
echo "Uploaded file ID: $FILE_ID"
rm test_upload.txt
echo

echo "19. Testing file info (GET /api/files/$FILE_ID/info):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/files/$FILE_ID/info
echo

echo "20. Testing file list (GET /api/files):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/files?item_id=$ITEM_V2_ID"
echo

echo "=== BACKGROUND JOBS TESTS ==="
echo

echo "21. Testing export job creation (POST /api/jobs/export):"
EXPORT_JOB_RESPONSE=$(curl -s -X POST $BASE_URL/api/jobs/export \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "format": "json",
    "filters": {
      "tags": ["test"]
    }
  }')
echo $EXPORT_JOB_RESPONSE
JOB_ID=$(echo $EXPORT_JOB_RESPONSE | jq -r '.data.job_id')
echo "Created job ID: $JOB_ID"
echo

echo "22. Testing job status (GET /api/jobs/$JOB_ID/status):"
sleep 1
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/jobs/$JOB_ID/status
echo

echo "23. Testing job list (GET /api/jobs):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/jobs?limit=5"
echo

echo "=== CACHE MANAGEMENT TESTS ==="
echo

echo "24. Testing cache stats (GET /api/cache/stats):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/cache/stats
echo

echo "=== ERROR HANDLING TESTS ==="
echo

echo "25. Testing unauthorized access (GET /api/v1/items without token):"
curl -s $BASE_URL/api/v1/items
echo

echo "26. Testing invalid token (GET /api/v1/items with invalid token):"
curl -s -H "Authorization: Bearer invalid_token" $BASE_URL/api/v1/items
echo

echo "27. Testing non-existent item (GET /api/v1/items/99999):"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items/99999
echo

echo "28. Testing invalid file upload (POST /api/files/upload with no file):"
curl -s -X POST $BASE_URL/api/files/upload \
  -H "Authorization: Bearer $JWT_TOKEN"
echo

echo "=== CLEANUP TESTS ==="
echo

echo "29. Testing file deletion (DELETE /api/files/$FILE_ID):"
curl -s -X DELETE $BASE_URL/api/files/$FILE_ID \
  -H "Authorization: Bearer $JWT_TOKEN"
echo

echo "30. Testing item deletion v1 (DELETE /api/v1/items/$ITEM_ID):"
curl -s -X DELETE $BASE_URL/api/v1/items/$ITEM_ID \
  -H "Authorization: Bearer $JWT_TOKEN"
echo

echo "31. Testing item deletion v2 (DELETE /api/v2/items/$ITEM_V2_ID):"
curl -s -X DELETE $BASE_URL/api/v2/items/$ITEM_V2_ID \
  -H "Authorization: Bearer $JWT_TOKEN"
echo

echo "=== FINAL STATS ==="
echo

echo "32. Final stats after all operations:"
curl -s $BASE_URL/api/stats
echo

echo "33. Final metrics after all operations:"
curl -s $BASE_URL/api/metrics
echo

echo "=== TEST COMPLETE ==="
echo "Note: WebSocket testing requires a separate WebSocket client."
echo "Dashboard is available at: $BASE_URL/dashboard"