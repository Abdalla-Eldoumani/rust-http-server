#!/bin/bash

BASE_URL="http://localhost:3000"
JWT_TOKEN=""

echo "=== Testing Advanced Server Features ==="
echo

extract_token() {
    echo "$1" | grep -o '"token":"[^"]*' | cut -d'"' -f4
}

echo "Setting up authentication..."
LOGIN_RESPONSE=$(curl -s -X POST $BASE_URL/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "testpassword123"
  }' 2>/dev/null)

if [ $? -eq 0 ] && [ -n "$LOGIN_RESPONSE" ]; then
    JWT_TOKEN=$(extract_token "$LOGIN_RESPONSE")
    echo "Authentication successful. Token: ${JWT_TOKEN:0:20}..."
else
    echo "Authentication failed or user doesn't exist. Creating test user..."
    REGISTER_RESPONSE=$(curl -s -X POST $BASE_URL/auth/register \
      -H "Content-Type: application/json" \
      -d '{
        "username": "testuser",
        "email": "test@example.com",
        "password": "testpassword123"
      }')
    JWT_TOKEN=$(extract_token "$REGISTER_RESPONSE")
    echo "User created. Token: ${JWT_TOKEN:0:20}..."
fi
echo

echo "=== WEBSOCKET TESTING ==="
echo

echo "1. WebSocket Connection Test:"
echo "WebSocket endpoint: ws://localhost:3000/ws?token=$JWT_TOKEN"
echo "Use a WebSocket client to connect and test real-time features"
echo "Example messages to send:"
echo '  {"type": "subscribe", "events": ["item_created", "metrics_update"]}'
echo

echo "=== RATE LIMITING TESTS ==="
echo

echo "2. Testing Rate Limiting (making rapid requests):"
echo "Making 15 rapid requests to test rate limiting..."
for i in {1..15}; do
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items)
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    REMAINING=$(curl -s -D - -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items 2>/dev/null | grep -i "x-ratelimit-remaining" | cut -d' ' -f2 | tr -d '\r')
    echo "Request $i: HTTP $HTTP_CODE - Remaining: $REMAINING"
    sleep 0.1
done
echo

echo "=== EXPORT FUNCTIONALITY TESTS ==="
echo

echo "3. Testing Export via Background Jobs:"

echo "3a. Creating test items for export..."
for i in {1..3}; do
    curl -s -X POST $BASE_URL/api/v2/items \
      -H "Authorization: Bearer $JWT_TOKEN" \
      -H "Content-Type: application/json" \
      -d "{
        \"name\": \"Export Test Item $i\",
        \"description\": \"Item created for export testing\",
        \"tags\": [\"export\", \"test\", \"item$i\"],
        \"metadata\": {\"test_batch\": \"export_test\", \"item_number\": $i}
      }" > /dev/null
done
echo "Created 3 test items for export"

echo "3b. Testing JSON export job:"
JSON_EXPORT_RESPONSE=$(curl -s -X POST $BASE_URL/api/jobs/export \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "format": "json",
    "filters": {
      "tags": ["export"]
    }
  }')
echo $JSON_EXPORT_RESPONSE | jq '.'
JSON_JOB_ID=$(echo $JSON_EXPORT_RESPONSE | jq -r '.data.job_id')

echo "3c. Testing CSV export job:"
CSV_EXPORT_RESPONSE=$(curl -s -X POST $BASE_URL/api/jobs/export \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "format": "csv",
    "filters": {
      "tags": ["test"]
    }
  }')
echo $CSV_EXPORT_RESPONSE | jq '.'
CSV_JOB_ID=$(echo $CSV_EXPORT_RESPONSE | jq -r '.data.job_id')

echo "3d. Checking export job statuses:"
sleep 2
echo "JSON Export Job Status:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/jobs/$JSON_JOB_ID/status | jq '.'
echo "CSV Export Job Status:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/jobs/$CSV_JOB_ID/status | jq '.'
echo

echo "=== SEARCH AND FILTERING TESTS ==="
echo

echo "4. Advanced Search Tests:"

echo "4a. Full-text search:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?q=export" | jq '.data.items[] | {id, name, relevance_score}'
echo

echo "4b. Tag-based filtering:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?tags=test,export" | jq '.data.items[] | {id, name, tags}'
echo

echo "4c. Date range filtering:"
TODAY=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
YESTERDAY=$(date -u -d "1 day ago" +"%Y-%m-%dT%H:%M:%SZ")
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?created_after=$YESTERDAY&created_before=$TODAY" | jq '.data.search_info'
echo

echo "4d. Fuzzy search test:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?q=exprt&fuzzy=true" | jq '.data.items[] | {id, name}'
echo

echo "=== FILE MANAGEMENT TESTS ==="
echo

echo "5. File Management Feature Tests:"

echo "5a. Creating test files for upload:"
echo "This is a test document for file management testing." > test_doc.txt
echo "name,value\ntest1,100\ntest2,200" > test_data.csv
echo '{"test": "data", "numbers": [1,2,3]}' > test_data.json

echo "5b. Testing multiple file uploads:"
DOC_UPLOAD=$(curl -s -X POST $BASE_URL/api/files/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@test_doc.txt")
echo "Document upload:" 
echo $DOC_UPLOAD | jq '.'
DOC_FILE_ID=$(echo $DOC_UPLOAD | jq -r '.data.id')

CSV_UPLOAD=$(curl -s -X POST $BASE_URL/api/files/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@test_data.csv")
echo "CSV upload:"
echo $CSV_UPLOAD | jq '.'
CSV_FILE_ID=$(echo $CSV_UPLOAD | jq -r '.data.id')

echo "5c. Testing file listing with filters:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/files?content_type=text/plain" | jq '.data.files[] | {id, filename, content_type, size}'
echo

echo "5d. Testing file download:"
echo "Downloading file $DOC_FILE_ID:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/files/$DOC_FILE_ID -o downloaded_file.txt
echo "Downloaded content:"
cat downloaded_file.txt
echo

echo "5e. Cleanup test files:"
rm -f test_doc.txt test_data.csv test_data.json downloaded_file.txt
curl -s -X DELETE $BASE_URL/api/files/$DOC_FILE_ID -H "Authorization: Bearer $JWT_TOKEN" > /dev/null
curl -s -X DELETE $BASE_URL/api/files/$CSV_FILE_ID -H "Authorization: Bearer $JWT_TOKEN" > /dev/null
echo "Test files cleaned up"
echo

echo "=== CACHE PERFORMANCE TESTS ==="
echo

echo "6. Cache Performance Tests:"

echo "6a. Initial cache stats:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/cache/stats | jq '.'

echo "6b. Making repeated requests to test caching:"
for i in {1..5}; do
    echo "Request $i:"
    START_TIME=$(date +%s%N)
    curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items > /dev/null
    END_TIME=$(date +%s%N)
    DURATION=$(( (END_TIME - START_TIME) / 1000000 ))
    echo "Response time: ${DURATION}ms"
done

echo "6c. Cache stats after repeated requests:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/cache/stats | jq '.'
echo

echo "=== BACKGROUND JOB MONITORING ==="
echo

echo "7. Background Job System Tests:"

echo "7a. Creating bulk import job:"
IMPORT_DATA='[
  {"name": "Bulk Item 1", "description": "Imported item 1", "tags": ["bulk", "import"]},
  {"name": "Bulk Item 2", "description": "Imported item 2", "tags": ["bulk", "import"]},
  {"name": "Bulk Item 3", "description": "Imported item 3", "tags": ["bulk", "import"]}
]'
IMPORT_JOB_RESPONSE=$(curl -s -X POST $BASE_URL/api/jobs/import \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"source\": \"json\",
    \"data\": \"$(echo $IMPORT_DATA | base64 -w 0)\",
    \"options\": {
      \"skip_duplicates\": true,
      \"validate_data\": true
    }
  }")
echo $IMPORT_JOB_RESPONSE | jq '.'
IMPORT_JOB_ID=$(echo $IMPORT_JOB_RESPONSE | jq -r '.data.job_id')

echo "7b. Monitoring job progress:"
for i in {1..5}; do
    echo "Check $i:"
    JOB_STATUS=$(curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/jobs/$IMPORT_JOB_ID/status)
    echo $JOB_STATUS | jq '{status: .data.status, progress: .data.progress}'
    STATUS=$(echo $JOB_STATUS | jq -r '.data.status')
    if [ "$STATUS" = "completed" ] || [ "$STATUS" = "failed" ]; then
        break
    fi
    sleep 1
done

echo "7c. Job history:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/jobs?limit=5" | jq '.data.jobs[] | {id, job_type, status, created_at}'
echo

echo "=== SYSTEM MONITORING ==="
echo

echo "8. System Health and Monitoring:"

echo "8a. Detailed health check:"
curl -s $BASE_URL/health | jq '.'

echo "8b. Performance metrics:"
curl -s $BASE_URL/api/metrics | jq '.data | {requests, response_times, cache, websocket}'

echo "8c. Server statistics:"
curl -s $BASE_URL/api/stats | jq '.'
echo

echo "=== CONFIGURATION TESTING ==="
echo

echo "9. Configuration and Environment Tests:"
echo "Current server configuration can be tested by checking:"
echo "- Server responds on configured host:port"
echo "- Database connectivity (shown in health check)"
echo "- File upload directory accessibility"
echo "- Rate limiting behavior (tested above)"
echo "- CORS headers in responses"
echo

echo "=== SECURITY TESTING ==="
echo

echo "10. Basic Security Tests:"

echo "10a. Testing without authentication:"
curl -s $BASE_URL/api/v1/items

echo "10b. Testing with invalid token:"
curl -s -H "Authorization: Bearer invalid_token_here" $BASE_URL/api/v1/items

echo "10c. Testing file upload size limits (if configured):"
echo "Large file upload test would require creating a file larger than configured limit"

echo "10d. Testing SQL injection prevention:"
curl -s -H "Authorization: Bearer $JWT_TOKEN" "$BASE_URL/api/items/search?q='; DROP TABLE items; --"
echo

echo "=== PERFORMANCE SUMMARY ==="
echo

echo "11. Final Performance Summary:"
echo "Making 10 concurrent requests to measure performance..."

for i in {1..10}; do
    (
        START=$(date +%s%N)
        curl -s -H "Authorization: Bearer $JWT_TOKEN" $BASE_URL/api/v1/items > /dev/null
        END=$(date +%s%N)
        DURATION=$(( (END - START) / 1000000 ))
        echo "Concurrent request $i: ${DURATION}ms"
    ) &
done
wait

echo
echo "Final system metrics:"
curl -s $BASE_URL/api/metrics
echo

echo "=== FEATURE TESTING COMPLETE ==="
echo
echo "Additional manual testing recommendations:"
echo "1. WebSocket real-time communication: ws://localhost:3000/ws?token=$JWT_TOKEN"
echo "2. Dashboard interface: http://localhost:3000/dashboard"
echo "3. Load testing with tools like wrk or ab"
echo "4. Database persistence testing (restart server and verify data)"
echo "5. Configuration file changes and environment variable overrides"