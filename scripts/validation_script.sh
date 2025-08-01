#!/bin/bash

# Comprehensive Validation Test Script
# This script tests input validation, security measures, and data sanitization

BASE_URL="http://localhost:3000"
RESULTS_DIR="./validation_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Comprehensive Validation Testing ===${NC}"
echo "Timestamp: $TIMESTAMP"
echo "Base URL: $BASE_URL"
echo

mkdir -p "$RESULTS_DIR"
RESULTS_FILE="$RESULTS_DIR/validation_test_$TIMESTAMP.log"
SERVER_PID=""

log_result() {
    echo "$1" >> "$RESULTS_FILE"
}

start_server() {
    echo -e "${YELLOW}Starting server...${NC}"
    RUST_LOG=info cargo run --bin server > server.log 2>&1 &
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

test_request() {
    local name="$1"
    local expected="$2"
    local url="$3"
    local method="$4"
    local data="$5"
    local headers="$6"
    
    echo -e "${YELLOW}Testing: $name${NC}"
    
    local status
    if [ "$method" = "POST" ]; then
        if [ -n "$headers" ]; then
            status=$(curl -s -w "%{http_code}" -X POST "$url" -H "$headers" -d "$data" -o /dev/null 2>/dev/null)
        else
            status=$(curl -s -w "%{http_code}" -X POST "$url" -d "$data" -o /dev/null 2>/dev/null)
        fi
    elif [ "$method" = "PUT" ]; then
        if [ -n "$headers" ]; then
            status=$(curl -s -w "%{http_code}" -X PUT "$url" -H "$headers" -d "$data" -o /dev/null 2>/dev/null)
        else
            status=$(curl -s -w "%{http_code}" -X PUT "$url" -d "$data" -o /dev/null 2>/dev/null)
        fi
    elif [ "$method" = "DELETE" ]; then
        status=$(curl -s -w "%{http_code}" -X DELETE "$url" -o /dev/null 2>/dev/null)
    else
        status=$(curl -s -w "%{http_code}" "$url" -o /dev/null 2>/dev/null)
    fi
    
    if [ "$status" = "$expected" ]; then
        echo -e "${GREEN}✓ $name: HTTP $status${NC}"
        log_result "✓ $name: HTTP $status"
        return 0
    else
        echo -e "${RED}✗ $name: HTTP $status (Expected: $expected)${NC}"
        log_result "✗ $name: HTTP $status (Expected: $expected)"
        return 1
    fi
}

run_all_tests() {
    echo -e "${BLUE}=== Testing SQL Injection Protection ===${NC}"
    log_result "=== SQL Injection Protection Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "SQL Injection in Item Name" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test; DROP TABLE items; --", "description": "test"}' \
        "Content-Type: application/json"
    
    test_request "SQL Injection in Search Query" "400" \
        "$BASE_URL/api/items/search?q=test%27%20UNION%20SELECT" \
        "GET"
    
    test_request "SQL-like Text in Description (Valid)" "201" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": "test OR 1=1; --"}' \
        "Content-Type: application/json"
    
    echo
    echo -e "${BLUE}=== Testing XSS Protection ===${NC}"
    log_result "=== XSS Protection Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "XSS Script Tag in Name" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "<script>alert(\"xss\")</script>", "description": "test"}' \
        "Content-Type: application/json"
    
    test_request "XSS JavaScript URL in Description" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": "javascript:alert(1)"}' \
        "Content-Type: application/json"
    
    test_request "XSS Event Handler" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": "<img src=x onerror=alert(1)>"}' \
        "Content-Type: application/json"
    
    test_request "XSS in Form Name" "400" \
        "$BASE_URL/api/form" \
        "POST" \
        "name=<script>alert(1)</script>&email=test@example.com&message=test" \
        "Content-Type: application/x-www-form-urlencoded"
    
    echo
    echo -e "${BLUE}=== Testing Input Validation ===${NC}"
    log_result "=== Input Validation Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "Empty Item Name" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "", "description": "test"}' \
        "Content-Type: application/json"
    
    test_request "Empty Item Description (Valid)" "201" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": ""}' \
        "Content-Type: application/json"
    
    LONG_NAME=$(printf 'A%.0s' {1..300})
    test_request "Oversized Item Name" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        "{\"name\": \"$LONG_NAME\", \"description\": \"test\"}" \
        "Content-Type: application/json"
    
    echo
    echo -e "${BLUE}=== Testing Form Validation ===${NC}"
    log_result "=== Form Validation Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "Invalid Email Format" "400" \
        "$BASE_URL/api/form" \
        "POST" \
        "name=test&email=invalid-email&message=test" \
        "Content-Type: application/x-www-form-urlencoded"
    
    test_request "Missing Form Name" "400" \
        "$BASE_URL/api/form" \
        "POST" \
        "email=test@example.com&message=test" \
        "Content-Type: application/x-www-form-urlencoded"
    
    test_request "Missing Form Email" "400" \
        "$BASE_URL/api/form" \
        "POST" \
        "name=test&message=test" \
        "Content-Type: application/x-www-form-urlencoded"
    
    echo
    echo -e "${BLUE}=== Testing Content Type Validation ===${NC}"
    log_result "=== Content Type Validation Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "Missing Content Type" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": "test"}'
    
    test_request "Invalid Content Type" "415" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": "test"}' \
        "Content-Type: text/plain"
    
    test_request "Malformed JSON" "400" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "test", "description": invalid}' \
        "Content-Type: application/json"
    
    echo
    echo -e "${BLUE}=== Testing Valid Requests ===${NC}"
    log_result "=== Valid Requests Test ==="
    log_result "Timestamp: $(date)"
    
    test_request "Valid Item Creation" "201" \
        "$BASE_URL/api/items" \
        "POST" \
        '{"name": "Valid Test Item", "description": "This is a valid test item"}' \
        "Content-Type: application/json"
    
    test_request "Valid Form Submission" "200" \
        "$BASE_URL/api/form" \
        "POST" \
        "name=John Doe&email=john@example.com&message=Hello world" \
        "Content-Type: application/x-www-form-urlencoded"
    
    test_request "Valid Item List" "200" \
        "$BASE_URL/api/items" \
        "GET"
    
    test_request "Valid Search Query" "200" \
        "$BASE_URL/api/items/search?q=test" \
        "GET"
    
    test_request "Valid Health Check" "200" \
        "$BASE_URL/health" \
        "GET"
    
    test_request "Valid Stats Endpoint" "200" \
        "$BASE_URL/api/stats" \
        "GET"
}

generate_validation_report() {
    echo
    echo -e "${BLUE}=== Validation Test Summary ===${NC}"
    log_result "=== Validation Test Summary ==="
    log_result "Test completed at: $(date)"
    
    TOTAL_TESTS=$(grep -c "✓\|✗" "$RESULTS_FILE" 2>/dev/null)
    PASSED_TESTS=$(grep -c "✓" "$RESULTS_FILE" 2>/dev/null)
    FAILED_TESTS=$(grep -c "✗" "$RESULTS_FILE" 2>/dev/null)
    
    TOTAL_TESTS=$(echo $TOTAL_TESTS | tr -d '\n\r ')
    PASSED_TESTS=$(echo $PASSED_TESTS | tr -d '\n\r ')
    FAILED_TESTS=$(echo $FAILED_TESTS | tr -d '\n\r ')
    
    echo
    echo "Test Results Summary:"
    echo "  Total Tests: $TOTAL_TESTS"
    echo -e "  ${GREEN}Passed: $PASSED_TESTS${NC}"
    echo -e "  ${RED}Failed: $FAILED_TESTS${NC}"
    
    log_result "Test Results Summary:"
    log_result "Total Tests: $TOTAL_TESTS"
    log_result "Passed: $PASSED_TESTS"
    log_result "Failed: $FAILED_TESTS"
    
    if [ "$FAILED_TESTS" -eq 0 ]; then
        RESULT_MESSAGE="✅ All validation tests passed! Input validation is working perfectly."
        EXIT_CODE=0
    else
        RESULT_MESSAGE="❌ Some validation tests failed. Security vulnerabilities may exist."
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
    for tool in curl; do
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
    
    run_all_tests
    
    generate_validation_report
    EXIT_CODE=$?
    cleanup
    exit $EXIT_CODE
}

main "$@"