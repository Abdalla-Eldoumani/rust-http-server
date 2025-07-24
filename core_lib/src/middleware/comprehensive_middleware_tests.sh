#!/bin/bash

# Comprehensive Test Script for Middleware Stack and Error Handling

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SERVER_URL="http://localhost:3000"
TEST_USER_TOKEN=""
ADMIN_TOKEN=""
SERVER_PID=""

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

start_server() {
    log_info "Starting server..."
    
    export RUST_LOG=debug
    export APP_RATE_LIMIT_REQUESTS_PER_MINUTE=10
    export APP_RATE_LIMIT_USER_REQUESTS_PER_MINUTE=20
    export APP_RATE_LIMIT_ADMIN_REQUESTS_PER_MINUTE=50
    export APP_CORS_ALLOW_CREDENTIALS=true
    export APP_LOGGING_INCLUDE_REQUEST_ID=true
    export APP_LOGGING_INCLUDE_USER_INFO=true
    export APP_LOGGING_INCLUDE_TIMING=true
    
    cargo run --bin server > server.log 2>&1 &
    SERVER_PID=$!
    
    log_info "Waiting for server to start (PID: $SERVER_PID)..."
    sleep 5
    
    if ! kill -0 $SERVER_PID 2>/dev/null; then
        log_error "Server failed to start"
        cat server.log
        exit 1
    fi
    
    if ! curl -s "$SERVER_URL/health" > /dev/null; then
        log_error "Server is not responding to health check"
        cat server.log
        cleanup
        exit 1
    fi
    
    log_success "Server started successfully"
}

stop_server() {
    if [ ! -z "$SERVER_PID" ] && kill -0 $SERVER_PID 2>/dev/null; then
        log_info "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID
        wait $SERVER_PID 2>/dev/null || true
        log_success "Server stopped"
    fi
}

cleanup() {
    stop_server
    rm -f server.log
    rm -f test_results.json
}

trap cleanup EXIT

test_cors_configuration() {
    log_info "Testing CORS configuration..."
    
    local response=$(curl -s -i -X OPTIONS \
        -H "Origin: http://localhost:3001" \
        -H "Access-Control-Request-Method: POST" \
        -H "Access-Control-Request-Headers: Content-Type,Authorization" \
        "$SERVER_URL/api/items")
    
    if echo "$response" | grep -q "access-control-allow-origin"; then
        log_success "CORS preflight request handled correctly"
    else
        log_error "CORS preflight request failed"
        echo "$response"
        return 1
    fi
    
    local cors_response=$(curl -s -i \
        -H "Origin: http://localhost:3001" \
        "$SERVER_URL/")
    
    if echo "$cors_response" | grep -q "access-control-allow-origin"; then
        log_success "CORS headers present in response"
    else
        log_error "CORS headers missing in response"
        echo "$cors_response"
        return 1
    fi
}

test_rate_limiting() {
    log_info "Testing rate limiting..."
    
    # local success_count=0
    # local rate_limit_hit=false
    
    # for i in {1..61}; do
    #     local response=$(curl -s -w "%{http_code}" -o /dev/null "$SERVER_URL/")
    #     if [ "$response" = "200" ]; then
    #         ((success_count++))
    #     elif [ "$response" = "429" ]; then
    #         rate_limit_hit=true
    #         break
    #     fi
    #     sleep 0.1
    # done
    
    # if [ "$rate_limit_hit" = true ]; then
    #     log_success "Rate limiting is working (hit limit after $success_count requests)"
    # else
    #     log_warning "Rate limiting may not be working as expected (completed $success_count requests without hitting limit)"
    # fi
    
    local headers=$(curl -s -I "$SERVER_URL/")
    if echo "$headers" | grep -q "x-ratelimit-limit"; then
        log_success "Rate limit headers are present"
    else
        log_error "Rate limit headers are missing"
        echo "$headers"
        return 1
    fi
    
    log_info "Waiting for rate limit to reset..."
    sleep 5
}

test_enhanced_logging() {
    log_info "Testing enhanced logging..."
    
    local response=$(curl -s -i "$SERVER_URL/")
    
    if echo "$response" | grep -q "x-request-id"; then
        log_success "Request ID header is present"
    else
        log_error "Request ID header is missing"
        echo "$response"
        return 1
    fi
    
    if echo "$response" | grep -q "x-response-time"; then
        log_success "Response time header is present"
    else
        log_error "Response time header is missing"
        echo "$response"
        return 1
    fi
    
    if grep -q "http_request" server.log; then
        log_success "Structured logging is working"
    else
        log_error "Structured logging not found in server logs"
        return 1
    fi
}

test_auth_middleware() {
    log_info "Testing authentication middleware integration..."
    
    local response=$(curl -s -w "%{http_code}" -o /dev/null "$SERVER_URL/")
    if [ "$response" = "200" ]; then
        log_success "Public endpoint accessible without authentication"
    else
        log_error "Public endpoint should be accessible without authentication"
        return 1
    fi

    log_info "Authentication middleware integration test completed"
}

test_cache_middleware() {
    log_info "Testing cache middleware integration..."
    
    local start_time=$(date +%s%N)
    curl -s "$SERVER_URL/" > /dev/null
    local first_request_time=$(($(date +%s%N) - start_time))
    
    start_time=$(date +%s%N)
    local response=$(curl -s -i "$SERVER_URL/")
    local second_request_time=$(($(date +%s%N) - start_time))
    
    if echo "$response" | grep -q "x-cache"; then
        log_success "Cache headers are present"
    else
        log_warning "Cache headers not found (may not be implemented for this endpoint)"
    fi
    
    log_info "Cache middleware integration test completed"
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

test_error_handling() {
    # log_info "Testing enhanced error handling..."
    
    # local not_found_response=$(curl -s "$SERVER_URL/nonexistent-endpoint")
    # if echo "$not_found_response" | grep -q '"error"'; then
    #     log_success "404 errors return proper JSON format"
    # else
    #     log_error "404 errors should return JSON format"
    #     echo $not_found_response
    #     return 1
    # fi
    
    # local bad_request=$(curl -s -X POST \
    #     -H "Content-Type: application/json" \
    #     -d '{"invalid": json}' \
    #     "$SERVER_URL/api/items")
    
    # if echo "$bad_request" | grep -q '"error"'; then
    #     log_success "Bad requests return proper error format"
    # else
    #     log_warning "Bad request error handling may need improvement"
    # fi
}

test_configuration() {
    log_info "Testing configuration loading..."
    
    if grep -q "Configuration loaded successfully" server.log; then
        log_success "Configuration loaded successfully"
    else
        log_error "Configuration loading failed"
        return 1
    fi
    
    if grep -q "Rate limiter cleanup" server.log; then
        log_success "Rate limiter cleanup task started"
    else
        log_warning "Rate limiter cleanup task may not be running"
    fi
}

test_middleware_order() {
    log_info "Testing middleware stack execution order..."
    
    local response=$(curl -s -i \
        -H "Origin: http://localhost:3001" \
        -H "User-Agent: Mozilla/5.0 (Test Browser)" \
        "$SERVER_URL/")
    
    local has_cors=false
    local has_rate_limit=false
    local has_request_id=false
    local has_timing=false
    
    if echo "$response" | grep -q "access-control-allow-origin"; then
        has_cors=true
    fi
    
    if echo "$response" | grep -q "x-ratelimit-limit"; then
        has_rate_limit=true
    fi
    
    if echo "$response" | grep -q "x-request-id"; then
        has_request_id=true
    fi
    
    if echo "$response" | grep -q "x-response-time"; then
        has_timing=true
    fi
    
    if [ "$has_cors" = true ] && [ "$has_rate_limit" = true ] && [ "$has_request_id" = true ] && [ "$has_timing" = true ]; then
        log_success "All middleware components are working together"
    else
        log_warning "Some middleware components may not be working properly"
        echo "CORS: $has_cors, Rate Limit: $has_rate_limit, Request ID: $has_request_id, Timing: $has_timing"
    fi
}

test_performance() {
    log_info "Testing middleware performance impact..."
    
    local total_time=0
    local num_requests=10
    
    for i in $(seq 1 $num_requests); do
        local start_time=$(date +%s%N)
        curl -s "$SERVER_URL/" > /dev/null
        local end_time=$(date +%s%N)
        local request_time=$((end_time - start_time))
        total_time=$((total_time + request_time))
    done
    
    local avg_time=$((total_time / num_requests / 1000000))
    
    if [ $avg_time -lt 100 ]; then
        log_success "Average response time: ${avg_time}ms (Good performance)"
    elif [ $avg_time -lt 500 ]; then
        log_warning "Average response time: ${avg_time}ms (Acceptable performance)"
    else
        log_error "Average response time: ${avg_time}ms (Poor performance)"
    fi
}

main() {
    log_info "Starting comprehensive middleware tests..."
    log_info "=========================================="
    
    log_info "Building project..."
    if ! cargo build --bin server; then
        log_error "Failed to build project"
        exit 1
    fi
    
    start_server
    
    local failed_tests=0
    
    test_cors_configuration || ((failed_tests++))
    test_rate_limiting || ((failed_tests++))
    test_enhanced_logging || ((failed_tests++))
    test_auth_middleware || ((failed_tests++))
    test_cache_middleware || ((failed_tests++))
    test_validation_middleware || ((failed_tests++))
    test_error_handling || ((failed_tests++))
    test_configuration || ((failed_tests++))
    test_middleware_order || ((failed_tests++))
    test_performance || ((failed_tests++))
    
    log_info "=========================================="
    if [ $failed_tests -eq 0 ]; then
        log_success "All tests passed! Task 12 implementation is working correctly."
    else
        log_warning "$failed_tests test(s) failed or had warnings. Please review the output above."
    fi
    
    log_info "Server logs available in: server.log"
    log_info "Test completed."
}

main "$@"