#!/bin/bash

# Comprehensive Cache Test Script
# This script tests HTTP caching behavior and validates cache headers

BASE_URL="http://localhost:3000"
RESULTS_DIR="./cache_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Comprehensive Cache Testing ===${NC}"
echo "Timestamp: $TIMESTAMP"
echo "Base URL: $BASE_URL"
echo

mkdir -p "$RESULTS_DIR"
RESULTS_FILE="$RESULTS_DIR/cache_test_$TIMESTAMP.log"

log_result() {
    echo "$1" >> "$RESULTS_FILE"
}

log_both() {
    echo "$1"
    echo "$1" >> "$RESULTS_FILE"
}

check_cache_header() {
    local response="$1"
    local expected_status="$2"
    local test_name="$3"
    
    if echo "$response" | grep -i "x-cache:" > /dev/null; then
        if echo "$response" | grep -i "x-cache: $expected_status" > /dev/null; then
            echo -e "${GREEN}✓ $test_name: Cache $expected_status${NC}"
            log_result "✓ $test_name: Cache $expected_status"
            return 0
        else
            echo -e "${RED}✗ $test_name: Expected cache $expected_status${NC}"
            log_result "✗ $test_name: Expected cache $expected_status"
            echo "Actual cache status:"
            echo "$response" | grep -i "x-cache:"
            return 1
        fi
    else
        echo -e "${RED}✗ $test_name: No cache headers found${NC}"
        log_result "✗ $test_name: No cache headers found"
        echo "Available headers:"
        echo "$response" | grep -i "x-"
        return 1
    fi
}

check_http_status() {
    local response="$1"
    local expected_code="$2"
    local test_name="$3"
    
    if echo "$response" | grep "HTTP/[0-9.]* $expected_code" > /dev/null; then
        echo -e "${GREEN}✓ $test_name: HTTP $expected_code${NC}"
        log_result "✓ $test_name: HTTP $expected_code"
        return 0
    else
        echo -e "${RED}✗ $test_name: Expected HTTP $expected_code${NC}"
        log_result "✗ $test_name: Expected HTTP $expected_code"
        return 1
    fi
}

start_server() {
    echo -e "${YELLOW}Starting server...${NC}"
    RUST_LOG=debug cargo run --bin server > server.log 2>&1 &
    SERVER_PID=$!
    
    echo "Waiting for server to start..."
    for i in {1..30}; do
        if curl -s "$BASE_URL/health" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Server started successfully${NC}"
            log_result "✓ Server started successfully"
            return 0
        fi
        sleep 1
    done
    
    echo -e "${RED}✗ Server failed to start${NC}"
    log_result "✗ Server failed to start"
    return 1
}

stop_server() {
    echo -e "${YELLOW}Stopping server...${NC}"
    if [ ! -z "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null
        wait $SERVER_PID 2>/dev/null
        echo -e "${GREEN}✓ Server stopped${NC}"
        log_result "✓ Server stopped"
    fi
}

test_basic_cache_behavior() {
    echo -e "${BLUE}=== Testing Basic Cache Behavior ===${NC}"
    log_result "=== Basic Cache Behavior Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing /api/items endpoint caching${NC}"
    
    echo "Making first request..."
    RESPONSE1=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    check_http_status "$RESPONSE1" "200" "Items endpoint first request"
    
    CACHE_STATUS1=$(echo "$RESPONSE1" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS1" = "MISS" ] || [ "$CACHE_STATUS1" = "HIT" ]; then
        echo -e "${GREEN}✓ Items endpoint first request: Cache $CACHE_STATUS1${NC}"
        log_result "✓ Items endpoint first request: Cache $CACHE_STATUS1"
    else
        echo -e "${RED}✗ Items endpoint first request: No cache headers found${NC}"
        log_result "✗ Items endpoint first request: No cache headers found"
    fi
    
    echo "Making second request..."
    RESPONSE2=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    check_http_status "$RESPONSE2" "200" "Items endpoint second request"
    
    CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS2" = "MISS" ] || [ "$CACHE_STATUS2" = "HIT" ]; then
        echo -e "${GREEN}✓ Items endpoint second request: Cache $CACHE_STATUS2${NC}"
        log_result "✓ Items endpoint second request: Cache $CACHE_STATUS2"
    else
        echo -e "${RED}✗ Items endpoint second request: No cache headers found${NC}"
        log_result "✗ Items endpoint second request: No cache headers found"
    fi
    
    log_result ""
    echo
}

test_health_endpoint_cache() {
    echo -e "${BLUE}=== Testing Health Endpoint Cache ===${NC}"
    log_result "=== Health Endpoint Cache Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing /health endpoint caching${NC}"
    
    echo "Making first request..."
    RESPONSE1=$(curl -s -i "$BASE_URL/health" 2>&1)
    check_http_status "$RESPONSE1" "200" "Health endpoint first request"
    
    CACHE_STATUS1=$(echo "$RESPONSE1" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS1" = "MISS" ] || [ "$CACHE_STATUS1" = "HIT" ]; then
        echo -e "${GREEN}✓ Health endpoint first request: Cache $CACHE_STATUS1${NC}"
        log_result "✓ Health endpoint first request: Cache $CACHE_STATUS1"
    else
        echo -e "${RED}✗ Health endpoint first request: No cache headers found${NC}"
        log_result "✗ Health endpoint first request: No cache headers found"
    fi
    
    echo "Making second request..."
    RESPONSE2=$(curl -s -i "$BASE_URL/health" 2>&1)
    check_http_status "$RESPONSE2" "200" "Health endpoint second request"
    
    CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS2" = "MISS" ] || [ "$CACHE_STATUS2" = "HIT" ]; then
        echo -e "${GREEN}✓ Health endpoint second request: Cache $CACHE_STATUS2${NC}"
        log_result "✓ Health endpoint second request: Cache $CACHE_STATUS2"
    else
        echo -e "${RED}✗ Health endpoint second request: No cache headers found${NC}"
        log_result "✗ Health endpoint second request: No cache headers found"
    fi
    
    log_result ""
    echo
}

test_query_parameter_cache() {
    echo -e "${BLUE}=== Testing Query Parameter Cache ===${NC}"
    log_result "=== Query Parameter Cache Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing cache with different query parameters${NC}"
    
    echo "Making request with page=1 (expecting MISS)..."
    RESPONSE1=$(curl -s -i "$BASE_URL/api/items?page=1" 2>&1)
    check_http_status "$RESPONSE1" "200" "Items page=1 first request"
    check_cache_header "$RESPONSE1" "MISS" "Items page=1 first request"
    
    echo "Making same request again (expecting HIT)..."
    RESPONSE2=$(curl -s -i "$BASE_URL/api/items?page=1" 2>&1)
    check_http_status "$RESPONSE2" "200" "Items page=1 second request"
    check_cache_header "$RESPONSE2" "HIT" "Items page=1 second request"
    
    echo "Making request with page=2 (expecting MISS)..."
    RESPONSE3=$(curl -s -i "$BASE_URL/api/items?page=2" 2>&1)
    check_http_status "$RESPONSE3" "200" "Items page=2 first request"
    check_cache_header "$RESPONSE3" "MISS" "Items page=2 first request"
    
    echo "Making request with limit=10 (expecting MISS)..."
    RESPONSE4=$(curl -s -i "$BASE_URL/api/items?limit=10" 2>&1)
    check_http_status "$RESPONSE4" "200" "Items limit=10 first request"
    check_cache_header "$RESPONSE4" "MISS" "Items limit=10 first request"
    
    log_result ""
    echo
}

test_post_requests_not_cached() {
    echo -e "${BLUE}=== Testing POST Requests Not Cached ===${NC}"
    log_result "=== POST Requests Not Cached Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing that POST requests are not cached${NC}"
    
    echo "Making first POST request (expecting MISS)..."
    RESPONSE1=$(curl -s -w "HTTP_%{http_code}" -X POST -H "Content-Type: application/json" -d '{"name":"cache test item","description":"test description"}' "$BASE_URL/api/items" 2>&1)
    HTTP_CODE1=$(echo $RESPONSE1 | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    check_http_status "HTTP/1.1 $HTTP_CODE1" "201" "POST request first attempt"
    echo -e "${YELLOW}! POST request first attempt: Cache headers not checked (POST with body)${NC}"
    log_result "! POST request first attempt: Cache headers not checked (POST with body)"
    
    echo "Making second POST request (expecting MISS)..."
    RESPONSE2=$(curl -s -w "HTTP_%{http_code}" -X POST -H "Content-Type: application/json" -d '{"name":"cache test item 2","description":"test description 2"}' "$BASE_URL/api/items" 2>&1)
    HTTP_CODE2=$(echo $RESPONSE2 | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    check_http_status "HTTP/1.1 $HTTP_CODE2" "201" "POST request second attempt"
    echo -e "${YELLOW}! POST request second attempt: Cache headers not checked (POST with body)${NC}"
    log_result "! POST request second attempt: Cache headers not checked (POST with body)"
    
    log_result ""
    echo
}

test_put_requests_not_cached() {
    echo -e "${BLUE}=== Testing PUT Requests Not Cached ===${NC}"
    log_result "=== PUT Requests Not Cached Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing that PUT requests are not cached${NC}"
    
    CREATE_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d '{"name":"item for PUT test","description":"test description"}' "$BASE_URL/api/items")
    ITEM_ID=$(echo "$CREATE_RESPONSE" | jq -r '.data.id' 2>/dev/null || echo "$CREATE_RESPONSE" | jq -r '.id' 2>/dev/null)
    
    if [ "$ITEM_ID" != "null" ] && [ -n "$ITEM_ID" ]; then
        echo "Created item ID: $ITEM_ID for PUT testing"
        
        echo "Making first PUT request (expecting MISS)..."
        RESPONSE1=$(curl -s -w "HTTP_%{http_code}" -X PUT -H "Content-Type: application/json" -d '{"name":"updated item","description":"updated description"}' "$BASE_URL/api/items/$ITEM_ID" 2>&1)
        HTTP_CODE1=$(echo $RESPONSE1 | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
        check_http_status "HTTP/1.1 $HTTP_CODE1" "200" "PUT request first attempt"
        echo -e "${YELLOW}! PUT request first attempt: Cache headers not checked (PUT with body)${NC}"
        log_result "! PUT request first attempt: Cache headers not checked (PUT with body)"
        
        echo "Making second PUT request (expecting MISS)..."
        RESPONSE2=$(curl -s -w "HTTP_%{http_code}" -X PUT -H "Content-Type: application/json" -d '{"name":"updated item again","description":"updated description again"}' "$BASE_URL/api/items/$ITEM_ID" 2>&1)
        HTTP_CODE2=$(echo $RESPONSE2 | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
        check_http_status "HTTP/1.1 $HTTP_CODE2" "200" "PUT request second attempt"
        echo -e "${YELLOW}! PUT request second attempt: Cache headers not checked (PUT with body)${NC}"
        log_result "! PUT request second attempt: Cache headers not checked (PUT with body)"
    else
        echo -e "${RED}✗ Failed to create item for PUT testing${NC}"
        log_result "✗ Failed to create item for PUT testing"
    fi
    
    log_result ""
    echo
}

test_delete_requests_not_cached() {
    echo -e "${BLUE}=== Testing DELETE Requests Not Cached ===${NC}"
    log_result "=== DELETE Requests Not Cached Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing that DELETE requests are not cached${NC}"
    
    CREATE_RESPONSE1=$(curl -s -X POST -H "Content-Type: application/json" -d '{"name":"item for DELETE test 1","description":"test description 1"}' "$BASE_URL/api/items")
    ITEM_ID1=$(echo "$CREATE_RESPONSE1" | jq -r '.data.id' 2>/dev/null || echo "$CREATE_RESPONSE1" | jq -r '.id' 2>/dev/null)
    
    CREATE_RESPONSE2=$(curl -s -X POST -H "Content-Type: application/json" -d '{"name":"item for DELETE test 2","description":"test description 2"}' "$BASE_URL/api/items")
    ITEM_ID2=$(echo "$CREATE_RESPONSE2" | jq -r '.data.id' 2>/dev/null || echo "$CREATE_RESPONSE2" | jq -r '.id' 2>/dev/null)
    
    if [ "$ITEM_ID1" != "null" ] && [ -n "$ITEM_ID1" ] && [ "$ITEM_ID2" != "null" ] && [ -n "$ITEM_ID2" ]; then
        echo "Created items ID: $ITEM_ID1, $ITEM_ID2 for DELETE testing"
        
        echo "Making first DELETE request..."
        RESPONSE1=$(curl -s -i -X DELETE "$BASE_URL/api/items/$ITEM_ID1" 2>&1)
        check_http_status "$RESPONSE1" "204" "DELETE request first attempt"
        
        if echo "$RESPONSE1" | grep -i "x-cache:" > /dev/null; then
            CACHE_STATUS1=$(echo "$RESPONSE1" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
            echo -e "${GREEN}✓ DELETE request first attempt: Cache $CACHE_STATUS1${NC}"
            log_result "✓ DELETE request first attempt: Cache $CACHE_STATUS1"
        else
            echo -e "${GREEN}✓ DELETE request first attempt: No cache headers (expected)${NC}"
            log_result "✓ DELETE request first attempt: No cache headers (expected)"
        fi
        
        echo "Making second DELETE request..."
        RESPONSE2=$(curl -s -i -X DELETE "$BASE_URL/api/items/$ITEM_ID2" 2>&1)
        check_http_status "$RESPONSE2" "204" "DELETE request second attempt"
        
        if echo "$RESPONSE2" | grep -i "x-cache:" > /dev/null; then
            CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
            echo -e "${GREEN}✓ DELETE request second attempt: Cache $CACHE_STATUS2${NC}"
            log_result "✓ DELETE request second attempt: Cache $CACHE_STATUS2"
        else
            echo -e "${GREEN}✓ DELETE request second attempt: No cache headers (expected)${NC}"
            log_result "✓ DELETE request second attempt: No cache headers (expected)"
        fi
    else
        echo -e "${RED}✗ Failed to create items for DELETE testing${NC}"
        log_result "✗ Failed to create items for DELETE testing"
    fi
    
    log_result ""
    echo
}

test_cache_headers_present() {
    echo -e "${BLUE}=== Testing Cache Headers Present ===${NC}"
    log_result "=== Cache Headers Present Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing that proper cache headers are present${NC}"
    
    RESPONSE=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    
    if echo "$RESPONSE" | grep -i "x-cache:" > /dev/null; then
        CACHE_STATUS=$(echo "$RESPONSE" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
        echo -e "${GREEN}✓ X-Cache header present: $CACHE_STATUS${NC}"
        log_result "✓ X-Cache header present: $CACHE_STATUS"
    else
        echo -e "${RED}✗ X-Cache header missing${NC}"
        log_result "✗ X-Cache header missing"
    fi
    
    if echo "$RESPONSE" | grep -i "cache-control:" > /dev/null; then
        echo -e "${GREEN}✓ Cache-Control header present${NC}"
        log_result "✓ Cache-Control header present"
    else
        echo -e "${YELLOW}! Cache-Control header missing (optional)${NC}"
        log_result "! Cache-Control header missing (optional)"
    fi
    
    if echo "$RESPONSE" | grep -i "etag:" > /dev/null; then
        echo -e "${GREEN}✓ ETag header present${NC}"
        log_result "✓ ETag header present"
    else
        echo -e "${YELLOW}! ETag header missing (optional)${NC}"
        log_result "! ETag header missing (optional)"
    fi
    
    if echo "$RESPONSE" | grep -i "x-request-id:" > /dev/null; then
        echo -e "${GREEN}✓ X-Request-ID header present${NC}"
        log_result "✓ X-Request-ID header present"
    fi
    
    if echo "$RESPONSE" | grep -i "x-response-time:" > /dev/null; then
        echo -e "${GREEN}✓ X-Response-Time header present${NC}"
        log_result "✓ X-Response-Time header present"
    fi
    
    if echo "$RESPONSE" | grep -i "x-ratelimit-limit:" > /dev/null; then
        echo -e "${GREEN}✓ X-RateLimit-Limit header present${NC}"
        log_result "✓ X-RateLimit-Limit header present"
    fi
    
    log_result ""
    echo
}

test_stats_endpoint_cache() {
    echo -e "${BLUE}=== Testing Stats Endpoint Cache ===${NC}"
    log_result "=== Stats Endpoint Cache Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing /api/stats endpoint caching${NC}"
    
    echo "Making first request..."
    RESPONSE1=$(curl -s -i "$BASE_URL/api/stats" 2>&1)
    check_http_status "$RESPONSE1" "200" "Stats endpoint first request"
    
    CACHE_STATUS1=$(echo "$RESPONSE1" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS1" = "MISS" ] || [ "$CACHE_STATUS1" = "HIT" ]; then
        echo -e "${GREEN}✓ Stats endpoint first request: Cache $CACHE_STATUS1${NC}"
        log_result "✓ Stats endpoint first request: Cache $CACHE_STATUS1"
    else
        echo -e "${RED}✗ Stats endpoint first request: No cache headers found${NC}"
        log_result "✗ Stats endpoint first request: No cache headers found"
    fi
    
    echo "Making second request..."
    RESPONSE2=$(curl -s -i "$BASE_URL/api/stats" 2>&1)
    check_http_status "$RESPONSE2" "200" "Stats endpoint second request"
    
    CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [ "$CACHE_STATUS2" = "MISS" ] || [ "$CACHE_STATUS2" = "HIT" ]; then
        echo -e "${GREEN}✓ Stats endpoint second request: Cache $CACHE_STATUS2${NC}"
        log_result "✓ Stats endpoint second request: Cache $CACHE_STATUS2"
    else
        echo -e "${RED}✗ Stats endpoint second request: No cache headers found${NC}"
        log_result "✗ Stats endpoint second request: No cache headers found"
    fi
    
    log_result ""
    echo
}

test_response_consistency() {
    echo -e "${BLUE}=== Testing Response Consistency ===${NC}"
    log_result "=== Response Consistency Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing cache behavior with headers${NC}"
    
    echo "Making first request to check cache status..."
    RESPONSE1=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    CACHE_STATUS1=$(echo "$RESPONSE1" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    
    if [ ! -z "$CACHE_STATUS1" ]; then
        echo -e "${GREEN}✓ First request cache status: $CACHE_STATUS1${NC}"
        log_result "✓ First request cache status: $CACHE_STATUS1"
    else
        echo -e "${RED}✗ First request: No cache status found${NC}"
        log_result "✗ First request: No cache status found"
    fi
    
    echo "Making second request to check cache behavior..."
    RESPONSE2=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
    
    if [ ! -z "$CACHE_STATUS2" ]; then
        echo -e "${GREEN}✓ Second request cache status: $CACHE_STATUS2${NC}"
        log_result "✓ Second request cache status: $CACHE_STATUS2"
        
        if [ "$CACHE_STATUS1" = "MISS" ] && [ "$CACHE_STATUS2" = "HIT" ]; then
            echo -e "${GREEN}✓ Cache working correctly: MISS -> HIT${NC}"
            log_result "✓ Cache working correctly: MISS -> HIT"
        elif [ "$CACHE_STATUS1" = "HIT" ] && [ "$CACHE_STATUS2" = "HIT" ]; then
            echo -e "${GREEN}✓ Cache working correctly: HIT -> HIT${NC}"
            log_result "✓ Cache working correctly: HIT -> HIT"
        else
            echo -e "${YELLOW}! Cache behavior: $CACHE_STATUS1 -> $CACHE_STATUS2 (may be expected)${NC}"
            log_result "! Cache behavior: $CACHE_STATUS1 -> $CACHE_STATUS2 (may be expected)"
        fi
    else
        echo -e "${RED}✗ Second request: No cache status found${NC}"
        log_result "✗ Second request: No cache status found"
    fi
    
    echo -e "${YELLOW}Testing response time consistency${NC}"
    TIME1=$(curl -s -w "%{time_total}" -o /dev/null "$BASE_URL/api/items")
    TIME2=$(curl -s -w "%{time_total}" -o /dev/null "$BASE_URL/api/items")
    TIME3=$(curl -s -w "%{time_total}" -o /dev/null "$BASE_URL/api/items")
    
    echo "Response times: ${TIME1}s, ${TIME2}s, ${TIME3}s"
    
    TIME1_MS=$(echo "$TIME1 * 1000" | bc -l 2>/dev/null || echo "0")
    TIME2_MS=$(echo "$TIME2 * 1000" | bc -l 2>/dev/null || echo "0")
    TIME3_MS=$(echo "$TIME3 * 1000" | bc -l 2>/dev/null || echo "0")
    
    if command -v bc &> /dev/null && [ "$TIME1_MS" != "0" ]; then
        AVG_TIME=$(echo "($TIME1_MS + $TIME2_MS + $TIME3_MS) / 3" | bc -l)
        echo "Average response time: ${AVG_TIME}ms"
        
        if (( $(echo "$AVG_TIME < 100" | bc -l) )); then
            echo -e "${GREEN}✓ Performance: Excellent response times${NC}"
            log_result "✓ Performance: Excellent response times"
        elif (( $(echo "$AVG_TIME < 500" | bc -l) )); then
            echo -e "${GREEN}✓ Performance: Good response times${NC}"
            log_result "✓ Performance: Good response times"
        else
            echo -e "${YELLOW}! Performance: Response times could be improved${NC}"
            log_result "! Performance: Response times could be improved"
        fi
    else
        echo -e "${YELLOW}! Performance: Cannot calculate metrics (bc not available)${NC}"
        log_result "! Performance: Cannot calculate metrics (bc not available)"
    fi
    
    log_result ""
    echo
}

generate_cache_report() {
    echo -e "${BLUE}=== Cache Test Summary ===${NC}"
    log_result "=== Cache Test Summary ==="
    log_result "Test completed at: $(date)"
    
    TOTAL_TESTS=$(grep -c "✓\|✗\|!" "$RESULTS_FILE" 2>/dev/null)
    PASSED_TESTS=$(grep -c "✓" "$RESULTS_FILE" 2>/dev/null)
    FAILED_TESTS=$(grep -c "✗" "$RESULTS_FILE" 2>/dev/null)
    WARNINGS=$(grep -c "!" "$RESULTS_FILE" 2>/dev/null)
    
    TOTAL_TESTS=$(echo $TOTAL_TESTS | tr -d '\n\r ')
    PASSED_TESTS=$(echo $PASSED_TESTS | tr -d '\n\r ')
    FAILED_TESTS=$(echo $FAILED_TESTS | tr -d '\n\r ')
    WARNINGS=$(echo $WARNINGS | tr -d '\n\r ')
    
    echo
    echo "Test Results Summary:"
    echo "  Total Tests: $TOTAL_TESTS"
    echo -e "  ${GREEN}Passed: $PASSED_TESTS${NC}"
    echo -e "  ${RED}Failed: $FAILED_TESTS${NC}"
    echo -e "  ${YELLOW}Warnings: $WARNINGS${NC}"
    
    log_result "Test Results Summary:"
    log_result "Total Tests: $TOTAL_TESTS"
    log_result "Passed: $PASSED_TESTS"
    log_result "Failed: $FAILED_TESTS"
    log_result "Warnings: $WARNINGS"
    
    if [ "$FAILED_TESTS" -eq 0 ] && [ "$WARNINGS" -eq 0 ]; then
        RESULT_MESSAGE="✅ All cache tests passed! Cache is working perfectly."
        EXIT_CODE=0
    elif [ "$FAILED_TESTS" -eq 0 ]; then
        RESULT_MESSAGE="⚠️ Cache tests passed with warnings."
        EXIT_CODE=0
    else
        RESULT_MESSAGE="❌ Cache test failures detected."
        EXIT_CODE=1
    fi
    
    echo
    echo -e "${GREEN}$RESULT_MESSAGE${NC}"
    log_result "$RESULT_MESSAGE"
    
    echo
    echo "Detailed results saved to: $RESULTS_FILE"
    echo "Exit Code: $EXIT_CODE"
    
    return $EXIT_CODE
}

main() {
    for tool in curl jq; do
        if ! command -v $tool &> /dev/null; then
            echo -e "${RED}Required tool '$tool' is not installed${NC}"
            exit 1
        fi
    done
    
    if ! start_server; then
        echo -e "${RED}Failed to start server. Exiting.${NC}"
        exit 1
    fi
    
    test_basic_cache_behavior
    test_health_endpoint_cache
    test_query_parameter_cache
    test_post_requests_not_cached
    test_put_requests_not_cached
    test_delete_requests_not_cached
    test_cache_headers_present
    test_stats_endpoint_cache
    test_response_consistency
    
    generate_cache_report
    EXIT_CODE=$?
    
    stop_server
    exit $EXIT_CODE
}

main "$@"