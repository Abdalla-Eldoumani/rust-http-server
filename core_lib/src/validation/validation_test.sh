#!/bin/bash

echo "🔒 VALIDATION SECURITY TEST"
echo "==================================="
echo ""

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

test_request() {
    local name="$1"
    local expected="$2"
    local url="$3"
    local method="$4"
    local data="$5"
    local headers="$6"
    
    echo -e "${BLUE}Testing: $name${NC}"
    
    if [ "$method" = "POST" ]; then
        if [ -n "$headers" ]; then
            status=$(curl -s -w "%{http_code}" -X POST "$url" -H "$headers" -d "$data" -o /dev/null)
        else
            status=$(curl -s -w "%{http_code}" -X POST "$url" -d "$data" -o /dev/null)
        fi
    else
        status=$(curl -s -w "%{http_code}" "$url" -o /dev/null)
    fi
    
    if [ "$status" = "$expected" ]; then
        echo -e "${GREEN}✅ PASS - Status: $status${NC}"
    else
        echo -e "${RED}❌ FAIL - Status: $status (Expected: $expected)${NC}"
    fi
    echo ""
}

echo "🛡️ SQL INJECTION TESTS"
echo "======================"

test_request "SQL Injection in Item Name" "400" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "test'\'''; DROP TABLE items; --", "description": "test"}' \
    "Content-Type: application/json""

test_request "SQL Injection in Search" "400" \
    "http://localhost:3000/api/items/search?q=test%27%20UNION%20SELECT" \
    "GET"

echo "🚫 XSS TESTS"
echo "============"

test_request "XSS Script Tag" "400" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "<script>alert(\"xss\")</script>", "description": "test"}' \
    "Content-Type: application/json"

test_request "XSS JavaScript URL" "400" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "test", "description": "javascript:alert(1)"}' \
    "Content-Type: application/json"

echo "📝 FORM VALIDATION TESTS"
echo "========================"

test_request "Invalid Email Format" "400" \
    "http://localhost:3000/api/form" \
    "POST" \
    "name=test&email=invalid-email&message=test" \
    "Content-Type: application/x-www-form-urlencoded"

test_request "XSS in Form Name" "400" \
    "http://localhost:3000/api/form" \
    "POST" \
    "name=<script>alert(1)</script>&email=test@example.com&message=test" \
    "Content-Type: application/x-www-form-urlencoded"

echo "🔍 INPUT VALIDATION TESTS"
echo "========================="

test_request "Empty Item Name" "400" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "", "description": "test"}' \
    "Content-Type: application/json"

test_request "Oversized Item Name" "400" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "'$(printf 'A%.0s' {1..300})'", "description": "test"}' \
    "Content-Type: application/json"

echo "✅ VALID REQUEST TESTS"
echo "======================"

test_request "Valid Item Creation" "201" \
    "http://localhost:3000/api/items" \
    "POST" \
    '{"name": "Valid Test Item", "description": "This is a valid test item"}' \
    "Content-Type: application/json"

test_request "Valid Form Submission" "200" \
    "http://localhost:3000/api/form" \
    "POST" \
    "name=John Doe&email=john@example.com&message=Hello world" \
    "Content-Type: application/x-www-form-urlencoded"

test_request "Valid Item List" "200" \
    "http://localhost:3000/api/items" \
    "GET"

test_request "Valid Search Query" "200" \
    "http://localhost:3000/api/items/search?q=test" \
    "GET"