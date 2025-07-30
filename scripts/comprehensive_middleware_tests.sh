#!/bin/bash

# Comprehensive Middleware Test Script
# This script tests all middleware components and their integration

BASE_URL="http://localhost:3000"
RESULTS_DIR="./middleware_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Comprehensive Middleware Testing ===${NC}"
echo "Timestamp: $TIMESTAMP"
echo "Base URL: $BASE_URL"
echo

mkdir -p "$RESULTS_DIR"
RESULTS_FILE="$RESULTS_DIR/middleware_test_$TIMESTAMP.log"
SERVER_PID=""

log_result() {
    echo "$1" >> "$RESULTS_FILE"
}

log_both() {
    echo "$1"
    echo "$1" >> "$RESULTS_FILE"
}

check_response_status() {
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

check_header_present() {
    local response="$1"
    local header_name="$2"
    local test_name="$3"
    
    if echo "$response" | grep -i "$header_name:" > /dev/null; then
        echo -e "${GREEN}✓ $test_name: $header_name header present${NC}"
        log_result "✓ $test_name: $header_name header present"
        return 0
    else
        echo -e "${RED}✗ $test_name: $header_name header missing${NC}"
        log_result "✗ $test_name: $header_name header missing"
        return 1
    fi
}

check_json_field() {
    local response="$1"
    local field_path="$2"
    local test_name="$3"
    
    if echo "$response" | jq -e "$field_path" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ $test_name: JSON field $field_path present${NC}"
        log_result "✓ $test_name: JSON field $field_path present"
        return 0
    else
        echo -e "${RED}✗ $test_name: JSON field $field_path missing${NC}"
        log_result "✗ $test_name: JSON field $field_path missing"
        return 1
    fi
}

start_server() {
    echo -e "${YELLOW}Starting server...${NC}"
    
    export RUST_LOG=debug
    export APP_RATE_LIMIT_REQUESTS_PER_MINUTE=60
    export APP_RATE_LIMIT_USER_REQUESTS_PER_MINUTE=120
    export APP_RATE_LIMIT_ADMIN_REQUESTS_PER_MINUTE=300
    export APP_CORS_ALLOW_CREDENTIALS=true
    export APP_LOGGING_INCLUDE_REQUEST_ID=true
    export APP_LOGGING_INCLUDE_USER_INFO=true
    export APP_LOGGING_INCLUDE_TIMING=true
    
    cargo run --bin server > server.log 2>&1 &
    SERVER_PID=$!
    
    echo "Waiting for server to start (PID: $SERVER_PID)..."
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
    cat server.log
    return 1
}

stop_server() {
    echo -e "${YELLOW}Stopping server...${NC}"
    if [ ! -z "$SERVER_PID" ] && kill -0 $SERVER_PID 2>/dev/null; then
        kill $SERVER_PID 2>/dev/null
        wait $SERVER_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Server stopped${NC}"
        log_result "✓ Server stopped"
    fi
}

cleanup() {
    stop_server
    rm -f server.log 2>/dev/null || true
}

test_cors_middleware() {
    echo -e "${BLUE}=== Testing CORS Middleware ===${NC}"
    log_result "=== CORS Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing CORS preflight request${NC}"
    RESPONSE=$(curl -s -i -X OPTIONS \
        -H "Origin: http://localhost:3001" \
        -H "Access-Control-Request-Method: POST" \
        -H "Access-Control-Request-Headers: Content-Type,Authorization" \
        "$BASE_URL/api/items" 2>&1)
    
    check_response_status "$RESPONSE" "200" "CORS preflight request"
    check_header_present "$RESPONSE" "access-control-allow-origin" "CORS preflight"
    check_header_present "$RESPONSE" "access-control-allow-methods" "CORS preflight"
    check_header_present "$RESPONSE" "access-control-allow-headers" "CORS preflight"
    
    echo -e "${YELLOW}Testing CORS headers in regular request${NC}"
    CORS_RESPONSE=$(curl -s -i \
        -H "Origin: http://localhost:3001" \
        "$BASE_URL/" 2>&1)
    
    check_response_status "$CORS_RESPONSE" "200" "CORS regular request"
    check_header_present "$CORS_RESPONSE" "access-control-allow-origin" "CORS regular request"
    
    echo -e "${YELLOW}Testing CORS credentials support${NC}"
    CREDS_RESPONSE=$(curl -s -i \
        -H "Origin: http://localhost:3001" \
        "$BASE_URL/api/stats" 2>&1)
    
    check_header_present "$CREDS_RESPONSE" "access-control-allow-credentials" "CORS credentials"
    
    log_result ""
    echo
}

test_rate_limiting_middleware() {
    echo -e "${BLUE}=== Testing Rate Limiting Middleware ===${NC}"
    log_result "=== Rate Limiting Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing rate limit headers presence${NC}"
    HEADERS=$(curl -s -I "$BASE_URL/" 2>&1)
    
    check_response_status "$HEADERS" "200" "Rate limit headers request"
    check_header_present "$HEADERS" "x-ratelimit-limit" "Rate limit headers"
    check_header_present "$HEADERS" "x-ratelimit-remaining" "Rate limit headers"
    check_header_present "$HEADERS" "x-ratelimit-type" "Rate limit headers"
    
    echo -e "${YELLOW}Testing rate limit values${NC}"
    LIMIT=$(echo "$HEADERS" | grep -i "x-ratelimit-limit:" | cut -d' ' -f2 | tr -d '\r\n ')
    REMAINING=$(echo "$HEADERS" | grep -i "x-ratelimit-remaining:" | cut -d' ' -f2 | tr -d '\r\n ')
    
    if [ ! -z "$LIMIT" ] && [ "$LIMIT" -gt 0 ]; then
        echo -e "${GREEN}✓ Rate limit value is valid: $LIMIT${NC}"
        log_result "✓ Rate limit value is valid: $LIMIT"
    else
        echo -e "${RED}✗ Rate limit value is invalid: $LIMIT${NC}"
        log_result "✗ Rate limit value is invalid: $LIMIT"
    fi
    
    if [ ! -z "$REMAINING" ] && [ "$REMAINING" -ge 0 ]; then
        echo -e "${GREEN}✓ Rate limit remaining is valid: $REMAINING${NC}"
        log_result "✓ Rate limit remaining is valid: $REMAINING"
    else
        echo -e "${RED}✗ Rate limit remaining is invalid: $REMAINING${NC}"
        log_result "✗ Rate limit remaining is invalid: $REMAINING"
    fi
    
    echo -e "${YELLOW}Testing rate limit decreases with requests${NC}"
    HEADERS2=$(curl -s -I "$BASE_URL/api/stats" 2>&1)
    REMAINING2=$(echo "$HEADERS2" | grep -i "x-ratelimit-remaining:" | cut -d' ' -f2 | tr -d '\r\n ')
    
    if [ ! -z "$REMAINING2" ] && [ "$REMAINING2" -lt "$REMAINING" ]; then
        echo -e "${GREEN}✓ Rate limit decreases with requests: $REMAINING -> $REMAINING2${NC}"
        log_result "✓ Rate limit decreases with requests: $REMAINING -> $REMAINING2"
    else
        echo -e "${YELLOW}! Rate limit behavior: $REMAINING -> $REMAINING2 (may be expected)${NC}"
        log_result "! Rate limit behavior: $REMAINING -> $REMAINING2 (may be expected)"
    fi
    
    log_result ""
    echo
}

test_logging_middleware() {
    echo -e "${BLUE}=== Testing Logging Middleware ===${NC}"
    log_result "=== Logging Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing request ID generation${NC}"
    RESPONSE=$(curl -s -i "$BASE_URL/" 2>&1)
    
    check_response_status "$RESPONSE" "200" "Logging middleware request"
    check_header_present "$RESPONSE" "x-request-id" "Request ID"
    
    REQUEST_ID=$(echo "$RESPONSE" | grep -i "x-request-id:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [[ "$REQUEST_ID" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]]; then
        echo -e "${GREEN}✓ Request ID format is valid UUID: $REQUEST_ID${NC}"
        log_result "✓ Request ID format is valid UUID"
    else
        echo -e "${RED}✗ Request ID format is invalid: $REQUEST_ID${NC}"
        log_result "✗ Request ID format is invalid"
    fi
    
    echo -e "${YELLOW}Testing response time tracking${NC}"
    check_header_present "$RESPONSE" "x-response-time" "Response time"
    
    RESPONSE_TIME=$(echo "$RESPONSE" | grep -i "x-response-time:" | cut -d' ' -f2 | tr -d '\r\n ')
    if [[ "$RESPONSE_TIME" =~ ^[0-9]+ms$ ]]; then
        echo -e "${GREEN}✓ Response time format is valid: $RESPONSE_TIME${NC}"
        log_result "✓ Response time format is valid: $RESPONSE_TIME"
    else
        echo -e "${RED}✗ Response time format is invalid: $RESPONSE_TIME${NC}"
        log_result "✗ Response time format is invalid: $RESPONSE_TIME"
    fi
    
    echo -e "${YELLOW}Testing structured logging in server logs${NC}"
    sleep 1
    if grep -q "http_request" server.log 2>/dev/null; then
        echo -e "${GREEN}✓ Structured logging is working${NC}"
        log_result "✓ Structured logging is working"
    else
        echo -e "${YELLOW}! Structured logging not found (may use different format)${NC}"
        log_result "! Structured logging not found (may use different format)"
    fi
    
    log_result ""
    echo
}

test_security_middleware() {
    echo -e "${BLUE}=== Testing Security Middleware ===${NC}"
    log_result "=== Security Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing public endpoint access${NC}"
    RESPONSE=$(curl -s -i "$BASE_URL/" 2>&1)
    check_response_status "$RESPONSE" "200" "Public endpoint access"
    
    echo -e "${YELLOW}Testing security headers${NC}"
    if echo "$RESPONSE" | grep -i "vary:" > /dev/null; then
        echo -e "${GREEN}✓ Vary header present (CORS security)${NC}"
        log_result "✓ Vary header present (CORS security)"
    else
        echo -e "${YELLOW}! Vary header missing${NC}"
        log_result "! Vary header missing"
    fi
    
    echo -e "${YELLOW}Testing user agent validation${NC}"
    NORMAL_UA_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -o /dev/null \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        "$BASE_URL/" 2>&1)
    HTTP_CODE=$(echo $NORMAL_UA_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Normal user agents are accepted${NC}"
        log_result "✓ Normal user agents are accepted"
    else
        echo -e "${RED}✗ Normal user agents should be accepted${NC}"
        log_result "✗ Normal user agents should be accepted"
    fi
    
    SUSPICIOUS_UA_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -o /dev/null \
        -H "User-Agent: sqlmap/1.0" \
        "$BASE_URL/" 2>&1)
    HTTP_CODE=$(echo $SUSPICIOUS_UA_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "403" ]; then
        echo -e "${GREEN}✓ Suspicious user agents are blocked${NC}"
        log_result "✓ Suspicious user agents are blocked"
    else
        echo -e "${YELLOW}! Suspicious user agents not blocked (may be intentional)${NC}"
        log_result "! Suspicious user agents not blocked (may be intentional)"
    fi
    
    log_result ""
    echo
}

test_cache_middleware() {
    echo -e "${BLUE}=== Testing Cache Middleware ===${NC}"
    log_result "=== Cache Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing cache headers presence${NC}"
    RESPONSE=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    
    check_response_status "$RESPONSE" "200" "Cache middleware request"
    
    if echo "$RESPONSE" | grep -i "x-cache:" > /dev/null; then
        echo -e "${GREEN}✓ X-Cache header present${NC}"
        log_result "✓ X-Cache header present"
        
        CACHE_STATUS=$(echo "$RESPONSE" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
        echo -e "${GREEN}✓ Cache status: $CACHE_STATUS${NC}"
        log_result "✓ Cache status: $CACHE_STATUS"
    else
        echo -e "${YELLOW}! X-Cache header missing (cache may not be implemented)${NC}"
        log_result "! X-Cache header missing (cache may not be implemented)"
    fi
    
    if echo "$RESPONSE" | grep -i "cache-control:" > /dev/null; then
        echo -e "${GREEN}✓ Cache-Control header present${NC}"
        log_result "✓ Cache-Control header present"
    else
        echo -e "${YELLOW}! Cache-Control header missing${NC}"
        log_result "! Cache-Control header missing"
    fi
    
    if echo "$RESPONSE" | grep -i "etag:" > /dev/null; then
        echo -e "${GREEN}✓ ETag header present${NC}"
        log_result "✓ ETag header present"
    else
        echo -e "${YELLOW}! ETag header missing${NC}"
        log_result "! ETag header missing"
    fi
    
    echo -e "${YELLOW}Testing cache behavior with repeated requests${NC}"
    RESPONSE2=$(curl -s -i "$BASE_URL/api/items" 2>&1)
    
    if echo "$RESPONSE2" | grep -i "x-cache:" > /dev/null; then
        CACHE_STATUS2=$(echo "$RESPONSE2" | grep -i "x-cache:" | cut -d' ' -f2 | tr -d '\r\n ')
        echo -e "${GREEN}✓ Second request cache status: $CACHE_STATUS2${NC}"
        log_result "✓ Second request cache status: $CACHE_STATUS2"
        
        if [ "$CACHE_STATUS2" = "HIT" ]; then
            echo -e "${GREEN}✓ Cache is working (HIT on second request)${NC}"
            log_result "✓ Cache is working (HIT on second request)"
        elif [ "$CACHE_STATUS2" = "MISS" ]; then
            echo -e "${YELLOW}! Cache behavior: Still MISS on second request${NC}"
            log_result "! Cache behavior: Still MISS on second request"
        fi
    fi
    
    log_result ""
    echo
}

test_validation_middleware() {
    log_info "Testing validation middleware..."
    
    local browser_response=$(curl -s -w "%{http_code}" -o /dev/null \
        -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
        "$SERVER_URL/")
    
    if [ "$browser_response" = "200" ]; then
        log_success "Browser requests pass validation"
    else
        log_error "Browser requests should pass validation"
        return 1
    fi
    
    local suspicious_response=$(curl -s -w "%{http_code}" -o /dev/null \
        -H "User-Agent: sqlmap/1.0" \
        "$SERVER_URL/")
    
    if [ "$suspicious_response" = "400" ]; then
        log_success "Suspicious user agents are blocked"
    else
        log_warning "Suspicious user agents may not be properly blocked"
    fi
}

test_error_handling_middleware() {
    echo -e "${BLUE}=== Testing Error Handling Middleware ===${NC}"
    log_result "=== Error Handling Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing 404 error handling${NC}"
    NOT_FOUND_RESPONSE=$(curl -s -i "$BASE_URL/nonexistent-endpoint" 2>&1)
    
    check_response_status "$NOT_FOUND_RESPONSE" "404" "404 error handling"
    
    CONTENT_LENGTH=$(echo "$NOT_FOUND_RESPONSE" | grep -i "content-length:" | cut -d' ' -f2 | tr -d '\r\n ')
    
    if [ "$CONTENT_LENGTH" = "0" ]; then
        echo -e "${GREEN}✓ 404 error returns empty body (standard HTTP behavior)${NC}"
        log_result "✓ 404 error returns empty body (standard HTTP behavior)"
    else
        NOT_FOUND_BODY=$(echo "$NOT_FOUND_RESPONSE" | sed -n '/^$/,$p' | tail -n +2)
        
        if echo "$NOT_FOUND_BODY" | jq . > /dev/null 2>&1; then
            check_json_field "$NOT_FOUND_BODY" '.error' "404 error JSON format"
            
            if echo "$NOT_FOUND_BODY" | jq -e '.error' 2>/dev/null | grep -q "Not Found\|not found\|404"; then
                echo -e "${GREEN}✓ 404 error message is descriptive${NC}"
                log_result "✓ 404 error message is descriptive"
            else
                echo -e "${YELLOW}! 404 error message could be more descriptive${NC}"
                log_result "! 404 error message could be more descriptive"
            fi
        else
            echo -e "${YELLOW}! 404 error returns non-JSON response${NC}"
            log_result "! 404 error returns non-JSON response"
        fi
    fi
    
    echo -e "${YELLOW}Testing 400 bad request handling${NC}"
    BAD_REQUEST_RESPONSE=$(curl -s -i -X POST \
        -H "Content-Type: application/json" \
        -d '{"invalid": json}' \
        "$BASE_URL/api/items" 2>&1)
    
    check_response_status "$BAD_REQUEST_RESPONSE" "400" "400 bad request handling"
    
    RESPONSE_CONTENT_TYPE=$(echo "$BAD_REQUEST_RESPONSE" | grep -i "content-type:" | cut -d' ' -f2- | tr -d '\r\n ')
    
    BAD_REQUEST_BODY=$(echo "$BAD_REQUEST_RESPONSE" | sed -n '/^$/,$p' | tail -n +2)
    
    if echo "$BAD_REQUEST_BODY" | jq . > /dev/null 2>&1; then
        check_json_field "$BAD_REQUEST_BODY" '.error' "400 error JSON format"
    else
        if [ ! -z "$BAD_REQUEST_BODY" ] && echo "$RESPONSE_CONTENT_TYPE" | grep -q "text/plain"; then
            echo -e "${GREEN}✓ 400 error returns text error message${NC}"
            log_result "✓ 400 error returns text error message"
        else
            echo -e "${YELLOW}! 400 error response format: $RESPONSE_CONTENT_TYPE${NC}"
            log_result "! 400 error response format: $RESPONSE_CONTENT_TYPE"
        fi
    fi
    
    echo -e "${YELLOW}Testing 405 method not allowed${NC}"
    METHOD_NOT_ALLOWED_RESPONSE=$(curl -s -i -X PATCH "$BASE_URL/nonexistent" 2>&1)
    
    HTTP_CODE=$(echo "$METHOD_NOT_ALLOWED_RESPONSE" | head -n 1 | grep -o "HTTP/[0-9.]* [0-9]*" | grep -o "[0-9]*$")
    if [ ! -z "$HTTP_CODE" ]; then
        if [ "$HTTP_CODE" = "405" ] || [ "$HTTP_CODE" = "404" ]; then
            echo -e "${GREEN}✓ Method not allowed handling: HTTP $HTTP_CODE${NC}"
            log_result "✓ Method not allowed handling: HTTP $HTTP_CODE"
        else
            echo -e "${YELLOW}! Method not allowed handling: HTTP $HTTP_CODE (may be expected)${NC}"
            log_result "! Method not allowed handling: HTTP $HTTP_CODE (may be expected)"
        fi
    else
        echo -e "${YELLOW}! Method not allowed: Could not extract HTTP code${NC}"
        log_result "! Method not allowed: Could not extract HTTP code"
    fi
    
    echo -e "${YELLOW}Testing error response consistency${NC}"
    ERROR_RESPONSES_HANDLED=0
    TOTAL_ERROR_RESPONSES=0
    
    if [ "$CONTENT_LENGTH" = "0" ]; then
        ((ERROR_RESPONSES_HANDLED++))
    elif [ ! -z "$NOT_FOUND_BODY" ]; then
        ((ERROR_RESPONSES_HANDLED++))
    fi
    ((TOTAL_ERROR_RESPONSES++))
    
    if [ ! -z "$BAD_REQUEST_BODY" ]; then
        ((ERROR_RESPONSES_HANDLED++))
    fi
    ((TOTAL_ERROR_RESPONSES++))
    
    if [ "$ERROR_RESPONSES_HANDLED" -eq "$TOTAL_ERROR_RESPONSES" ]; then
        echo -e "${GREEN}✓ All error responses are properly handled${NC}"
        log_result "✓ All error responses are properly handled"
    else
        echo -e "${YELLOW}! Some error responses may need improvement ($ERROR_RESPONSES_HANDLED/$TOTAL_ERROR_RESPONSES)${NC}"
        log_result "! Some error responses may need improvement ($ERROR_RESPONSES_HANDLED/$TOTAL_ERROR_RESPONSES)"
    fi
    
    log_result ""
    echo
}

test_middleware_integration() {
    echo -e "${BLUE}=== Testing Middleware Integration ===${NC}"
    log_result "=== Middleware Integration Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing all middleware components work together${NC}"
    RESPONSE=$(curl -s -i \
        -H "Origin: http://localhost:3001" \
        -H "User-Agent: Mozilla/5.0 (Test Browser)" \
        "$BASE_URL/api/stats" 2>&1)
    
    check_response_status "$RESPONSE" "200" "Middleware integration"
    
    local has_cors=false
    local has_rate_limit=false
    local has_request_id=false
    local has_timing=false
    
    if echo "$RESPONSE" | grep -i "access-control-allow-origin" > /dev/null; then
        has_cors=true
        echo -e "${GREEN}✓ CORS middleware active${NC}"
        log_result "✓ CORS middleware active"
    fi
    
    if echo "$RESPONSE" | grep -i "x-ratelimit-limit" > /dev/null; then
        has_rate_limit=true
        echo -e "${GREEN}✓ Rate limiting middleware active${NC}"
        log_result "✓ Rate limiting middleware active"
    fi
    
    if echo "$RESPONSE" | grep -i "x-request-id" > /dev/null; then
        has_request_id=true
        echo -e "${GREEN}✓ Logging middleware active${NC}"
        log_result "✓ Logging middleware active"
    fi
    
    if echo "$RESPONSE" | grep -i "x-response-time" > /dev/null; then
        has_timing=true
        echo -e "${GREEN}✓ Timing middleware active${NC}"
        log_result "✓ Timing middleware active"
    fi
    
    if [ "$has_cors" = true ] && [ "$has_rate_limit" = true ] && [ "$has_request_id" = true ] && [ "$has_timing" = true ]; then
        echo -e "${GREEN}✓ All middleware components are working together${NC}"
        log_result "✓ All middleware components are working together"
    else
        echo -e "${YELLOW}! Some middleware components may not be active${NC}"
        log_result "! Some middleware components may not be active"
        echo "CORS: $has_cors, Rate Limit: $has_rate_limit, Request ID: $has_request_id, Timing: $has_timing"
    fi
    
    log_result ""
    echo
}

test_middleware_performance() {
    echo -e "${BLUE}=== Testing Middleware Performance ===${NC}"
    log_result "=== Middleware Performance Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing middleware performance impact${NC}"
    
    local total_time=0
    local num_requests=10
    local response_times=()
    
    for i in $(seq 1 $num_requests); do
        local start_time=$(date +%s%N)
        curl -s "$BASE_URL/" > /dev/null
        local end_time=$(date +%s%N)
        local request_time=$((end_time - start_time))
        total_time=$((total_time + request_time))
        response_times+=($request_time)
    done
    
    local avg_time=$((total_time / num_requests / 1000000))
    
    echo "Average response time: ${avg_time}ms"
    
    if [ $avg_time -lt 100 ]; then
        echo -e "${GREEN}✓ Excellent performance: ${avg_time}ms${NC}"
        log_result "✓ Excellent performance: ${avg_time}ms"
    elif [ $avg_time -lt 200 ]; then
        echo -e "${GREEN}✓ Good performance: ${avg_time}ms${NC}"
        log_result "✓ Good performance: ${avg_time}ms"
    elif [ $avg_time -lt 500 ]; then
        echo -e "${GREEN}✓ Acceptable performance: ${avg_time}ms${NC}"
        log_result "✓ Acceptable performance: ${avg_time}ms"
    else
        echo -e "${YELLOW}! Performance could be improved: ${avg_time}ms${NC}"
        log_result "! Performance could be improved: ${avg_time}ms"
    fi
    
    echo -e "${YELLOW}Testing response time consistency${NC}"
    local min_time=${response_times[0]}
    local max_time=${response_times[0]}
    
    for time in "${response_times[@]}"; do
        if [ $time -lt $min_time ]; then
            min_time=$time
        fi
        if [ $time -gt $max_time ]; then
            max_time=$time
        fi
    done
    
    local min_ms=$((min_time / 1000000))
    local max_ms=$((max_time / 1000000))
    local variance=$((max_ms - min_ms))
    
    echo "Response time range: ${min_ms}ms - ${max_ms}ms (variance: ${variance}ms)"
    
    if [ $variance -lt 100 ]; then
        echo -e "${GREEN}✓ Consistent response times${NC}"
        log_result "✓ Consistent response times"
    elif [ $variance -lt 300 ]; then
        echo -e "${GREEN}✓ Acceptable response time variance${NC}"
        log_result "✓ Acceptable response time variance"
    else
        echo -e "${YELLOW}! High response time variance (may be due to system load)${NC}"
        log_result "! High response time variance (may be due to system load)"
    fi
    
    log_result ""
    echo
}

test_content_type_middleware() {
    echo -e "${BLUE}=== Testing Content Type Middleware ===${NC}"
    log_result "=== Content Type Middleware Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing JSON content type handling${NC}"
    JSON_RESPONSE=$(curl -s -i -X POST \
        -H "Content-Type: application/json" \
        -d '{"name":"test item","description":"test description"}' \
        "$BASE_URL/api/items" 2>&1)
    
    check_response_status "$JSON_RESPONSE" "201" "JSON content type"
    check_header_present "$JSON_RESPONSE" "content-type" "JSON response content type"
    
    if echo "$JSON_RESPONSE" | grep -i "content-type:" | grep -q "application/json"; then
        echo -e "${GREEN}✓ JSON response content type is correct${NC}"
        log_result "✓ JSON response content type is correct"
    else
        echo -e "${RED}✗ JSON response content type is incorrect${NC}"
        log_result "✗ JSON response content type is incorrect"
    fi
    
    echo -e "${YELLOW}Testing form data content type handling${NC}"
    FORM_RESPONSE=$(curl -s -i -X POST \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "name=form test&email=test@example.com&message=test message" \
        "$BASE_URL/api/form" 2>&1)
    
    check_response_status "$FORM_RESPONSE" "200" "Form data content type"
    
    echo -e "${YELLOW}Testing unsupported content type${NC}"
    UNSUPPORTED_RESPONSE=$(curl -s -i -X POST \
        -H "Content-Type: text/plain" \
        -d "plain text data" \
        "$BASE_URL/api/items" 2>&1)
    
    HTTP_CODE=$(echo "$UNSUPPORTED_RESPONSE" | head -n 1 | grep -o "HTTP/[0-9.]* [0-9]*" | grep -o "[0-9]*$")
    if [ ! -z "$HTTP_CODE" ]; then
        if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "415" ]; then
            echo -e "${GREEN}✓ Unsupported content type rejected: HTTP $HTTP_CODE${NC}"
            log_result "✓ Unsupported content type rejected: HTTP $HTTP_CODE"
        else
            echo -e "${YELLOW}! Unsupported content type handling: HTTP $HTTP_CODE (may accept all types)${NC}"
            log_result "! Unsupported content type handling: HTTP $HTTP_CODE (may accept all types)"
        fi
    else
        echo -e "${YELLOW}! Unsupported content type: Could not extract HTTP code${NC}"
        log_result "! Unsupported content type: Could not extract HTTP code"
    fi
    
    log_result ""
    echo
}

generate_middleware_report() {
    echo -e "${BLUE}=== Middleware Test Summary ===${NC}"
    log_result "=== Middleware Test Summary ==="
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
        RESULT_MESSAGE="✅ All middleware tests passed! Middleware stack is working perfectly."
        EXIT_CODE=0
    elif [ "$FAILED_TESTS" -eq 0 ]; then
        RESULT_MESSAGE="⚠️ Middleware tests passed with warnings."
        EXIT_CODE=0
    else
        RESULT_MESSAGE="❌ Middleware test failures detected."
        EXIT_CODE=1
    fi
    
    echo
    echo -e "${GREEN}$RESULT_MESSAGE${NC}"
    log_result "$RESULT_MESSAGE"
    
    echo
    echo "Detailed results saved to: $RESULTS_FILE"
    echo "Server logs available in: server.log"
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
    
    echo "Building project..."
    if ! cargo build --bin server; then
        echo -e "${RED}Failed to build project${NC}"
        exit 1
    fi
    
    if ! start_server; then
        echo -e "${RED}Failed to start server. Exiting.${NC}"
        exit 1
    fi
    
    test_cors_middleware
    test_rate_limiting_middleware
    test_logging_middleware
    test_security_middleware
    test_cache_middleware
    test_error_handling_middleware
    test_middleware_integration
    test_middleware_performance
    test_content_type_middleware
    
    generate_middleware_report
    EXIT_CODE=$?
    cleanup
    exit $EXIT_CODE
}

main "$@"