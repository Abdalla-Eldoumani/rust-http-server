#!/bin/bash

# Backward Compatibility Test Script
# This script verifies that existing API clients continue to work with the enhanced server

BASE_URL="http://localhost:3000"
RESULTS_DIR="./compatibility_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Backward Compatibility Testing ===${NC}"
echo "Timestamp: $TIMESTAMP"
echo "Base URL: $BASE_URL"
echo

mkdir -p "$RESULTS_DIR"
RESULTS_FILE="$RESULTS_DIR/compatibility_test_$TIMESTAMP.log"

log_result() {
    echo "$1" >> "$RESULTS_FILE"
}

log_both() {
    echo "$1"
    echo "$1" >> "$RESULTS_FILE"
}

test_original_endpoints() {
    echo -e "${BLUE}=== Testing Original API Endpoints ===${NC}"
    log_result "=== Original API Endpoints Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing root endpoint (/)${NC}"
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/")
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Root endpoint works${NC}"
        log_result "✓ Root endpoint: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ Root endpoint failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Root endpoint: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing health endpoint (/health)${NC}"
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/health")
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Health endpoint works${NC}"
        log_result "✓ Health endpoint: HTTP $HTTP_CODE"
        
        HEALTH_DATA=$(curl -s "$BASE_URL/health")
        if echo "$HEALTH_DATA" | jq -e '.data.overall_status' > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Health response format is valid${NC}"
            log_result "✓ Health response format: Valid JSON"
        else
            echo -e "${RED}✗ Health response format changed${NC}"
            log_result "✗ Health response format: Invalid"
        fi
    else
        echo -e "${RED}✗ Health endpoint failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Health endpoint: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing stats endpoint (/api/stats)${NC}"
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/stats")
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Stats endpoint works${NC}"
        log_result "✓ Stats endpoint: HTTP $HTTP_CODE"
        
        STATS_DATA=$(curl -s "$BASE_URL/api/stats")
        if echo "$STATS_DATA" | jq -e '.data.total_items' > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Stats response format is compatible${NC}"
            log_result "✓ Stats response format: Compatible"
        else
            echo -e "${RED}✗ Stats response format changed${NC}"
            log_result "✗ Stats response format: Changed"
        fi
    else
        echo -e "${RED}✗ Stats endpoint failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Stats endpoint: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing metrics endpoint (/api/metrics)${NC}"
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/metrics")
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Metrics endpoint works${NC}"
        log_result "✓ Metrics endpoint: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ Metrics endpoint failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Metrics endpoint: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_legacy_item_operations() {
    echo -e "${BLUE}=== Testing Legacy Item Operations ===${NC}"
    log_result "=== Legacy Item Operations Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing item listing (/api/items)${NC}"
    RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/items")
    HTTP_CODE=$(echo $RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "401" ]; then
        if [ "$HTTP_CODE" = "200" ]; then
            echo -e "${GREEN}✓ Item listing works without auth (backward compatible)${NC}"
            log_result "✓ Item listing: HTTP $HTTP_CODE (no auth required)"
        else
            echo -e "${YELLOW}! Item listing requires auth (breaking change)${NC}"
            log_result "! Item listing: HTTP $HTTP_CODE (auth required)"
        fi
    else
        echo -e "${RED}✗ Item listing failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Item listing: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing item creation without auth (/api/items)${NC}"
    CREATE_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X POST "$BASE_URL/api/items" \
      -H "Content-Type: application/json" \
      -d '{
        "name": "Legacy Test Item",
        "description": "Created for backward compatibility testing"
      }')
    HTTP_CODE=$(echo $CREATE_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "201" ]; then
        echo -e "${GREEN}✓ Item creation works without auth (backward compatible)${NC}"
        log_result "✓ Item creation: HTTP $HTTP_CODE (no auth required)"
        
        ITEM_DATA=$(echo $CREATE_RESPONSE | sed 's/HTTP_[0-9]*$//')
        ITEM_ID=$(echo $ITEM_DATA | jq -r '.data.id' 2>/dev/null || echo $ITEM_DATA | jq -r '.id' 2>/dev/null)
        
        if [ "$ITEM_ID" != "null" ] && [ -n "$ITEM_ID" ]; then
            echo "Created item ID: $ITEM_ID"
            
            echo -e "${YELLOW}Testing item retrieval (/api/items/$ITEM_ID)${NC}"
            GET_RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/items/$ITEM_ID")
            GET_HTTP_CODE=$(echo $GET_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
            
            if [ "$GET_HTTP_CODE" = "200" ]; then
                echo -e "${GREEN}✓ Item retrieval works${NC}"
                log_result "✓ Item retrieval: HTTP $GET_HTTP_CODE"
                
                ITEM_DATA=$(echo $GET_RESPONSE | sed 's/HTTP_[0-9]*$//')
                if echo "$ITEM_DATA" | jq -e '.data.name' > /dev/null 2>&1 || echo "$ITEM_DATA" | jq -e '.name' > /dev/null 2>&1; then
                    echo -e "${GREEN}✓ Item response format is compatible${NC}"
                    log_result "✓ Item response format: Compatible"
                else
                    echo -e "${RED}✗ Item response format changed${NC}"
                    log_result "✗ Item response format: Changed"
                fi
            else
                echo -e "${RED}✗ Item retrieval failed: HTTP $GET_HTTP_CODE${NC}"
                log_result "✗ Item retrieval: HTTP $GET_HTTP_CODE"
            fi
            
            echo -e "${YELLOW}Testing item update (/api/items/$ITEM_ID)${NC}"
            UPDATE_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X PUT "$BASE_URL/api/items/$ITEM_ID" \
              -H "Content-Type: application/json" \
              -d '{
                "name": "Updated Legacy Test Item",
                "description": "Updated for backward compatibility testing"
              }')
            UPDATE_HTTP_CODE=$(echo $UPDATE_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
            
            if [ "$UPDATE_HTTP_CODE" = "200" ]; then
                echo -e "${GREEN}✓ Item update works${NC}"
                log_result "✓ Item update: HTTP $UPDATE_HTTP_CODE"
            elif [ "$UPDATE_HTTP_CODE" = "401" ]; then
                echo -e "${YELLOW}! Item update requires auth (breaking change)${NC}"
                log_result "! Item update: HTTP $UPDATE_HTTP_CODE (auth required)"
            else
                echo -e "${RED}✗ Item update failed: HTTP $UPDATE_HTTP_CODE${NC}"
                log_result "✗ Item update: HTTP $UPDATE_HTTP_CODE"
            fi
            
            echo -e "${YELLOW}Testing item deletion (/api/items/$ITEM_ID)${NC}"
            DELETE_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X DELETE "$BASE_URL/api/items/$ITEM_ID")
            DELETE_HTTP_CODE=$(echo $DELETE_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
            
            if [ "$DELETE_HTTP_CODE" = "200" ] || [ "$DELETE_HTTP_CODE" = "204" ]; then
                echo -e "${GREEN}✓ Item deletion works${NC}"
                log_result "✓ Item deletion: HTTP $DELETE_HTTP_CODE"
            elif [ "$DELETE_HTTP_CODE" = "401" ]; then
                echo -e "${YELLOW}! Item deletion requires auth (breaking change)${NC}"
                log_result "! Item deletion: HTTP $DELETE_HTTP_CODE (auth required)"
            else
                echo -e "${RED}✗ Item deletion failed: HTTP $DELETE_HTTP_CODE${NC}"
                log_result "✗ Item deletion: HTTP $DELETE_HTTP_CODE"
            fi
        fi
    elif [ "$HTTP_CODE" = "401" ]; then
        echo -e "${YELLOW}! Item creation requires auth (breaking change)${NC}"
        log_result "! Item creation: HTTP $HTTP_CODE (auth required)"
    else
        echo -e "${RED}✗ Item creation failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Item creation: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_form_handling() {
    echo -e "${BLUE}=== Testing Form Handling ===${NC}"
    log_result "=== Form Handling Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing form submission with JSON (/api/form)${NC}"
    FORM_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X POST "$BASE_URL/api/form" \
      -H "Content-Type: application/json" \
      -d '{
        "name": "Form Test",
        "email": "test@example.com",
        "message": "Testing form compatibility"
      }')
    HTTP_CODE=$(echo $FORM_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Form submission with JSON works${NC}"
        log_result "✓ Form JSON submission: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ Form submission with JSON failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Form JSON submission: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing form submission with URL-encoded data (/api/form)${NC}"
    FORM_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X POST "$BASE_URL/api/form" \
      -H "Content-Type: application/x-www-form-urlencoded" \
      -d "name=Form Test&email=test@example.com&message=Testing form compatibility")
    HTTP_CODE=$(echo $FORM_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Form submission with URL-encoded data works${NC}"
        log_result "✓ Form URL-encoded submission: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ Form submission with URL-encoded data failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Form URL-encoded submission: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_export_functionality() {
    echo -e "${BLUE}=== Testing Export Functionality ===${NC}"
    log_result "=== Export Functionality Test ==="
    log_result "Timestamp: $(date)"
    
    export_formats=("json" "csv" "yaml")
    
    for format in "${export_formats[@]}"; do
        echo -e "${YELLOW}Testing $format export (/api/items/export?format=$format)${NC}"
        EXPORT_RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/items/export?format=$format")
        HTTP_CODE=$(echo $EXPORT_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
        
        if [ "$HTTP_CODE" = "200" ]; then
            echo -e "${GREEN}✓ $format export works${NC}"
            log_result "✓ $format export: HTTP $HTTP_CODE"
        elif [ "$HTTP_CODE" = "401" ]; then
            echo -e "${YELLOW}! $format export requires auth (breaking change)${NC}"
            log_result "! $format export: HTTP $HTTP_CODE (auth required)"
        else
            echo -e "${RED}✗ $format export failed: HTTP $HTTP_CODE${NC}"
            log_result "✗ $format export: HTTP $HTTP_CODE"
        fi
    done
    
    log_result ""
    echo
}

test_http_methods() {
    echo -e "${BLUE}=== Testing HTTP Methods Support ===${NC}"
    log_result "=== HTTP Methods Support Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing HEAD method (/api/stats)${NC}"
    HEAD_RESPONSE=$(curl -s -I -w "HTTP_%{http_code}" "$BASE_URL/api/stats")
    HTTP_CODE=$(echo $HEAD_RESPONSE | grep -o "HTTP/[0-9.]* [0-9]*" | grep -o "[0-9]*$")
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ HEAD method works${NC}"
        log_result "✓ HEAD method: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ HEAD method failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ HEAD method: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing OPTIONS method (/api/items)${NC}"
    OPTIONS_RESPONSE=$(curl -s -X OPTIONS -w "HTTP_%{http_code}" "$BASE_URL/api/items")
    HTTP_CODE=$(echo $OPTIONS_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "204" ]; then
        echo -e "${GREEN}✓ OPTIONS method works${NC}"
        log_result "✓ OPTIONS method: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ OPTIONS method failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ OPTIONS method: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_cors_headers() {
    echo -e "${BLUE}=== Testing CORS Headers ===${NC}"
    log_result "=== CORS Headers Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing CORS preflight request${NC}"
    CORS_RESPONSE=$(curl -s -X OPTIONS "$BASE_URL/api/items" \
      -H "Origin: http://localhost:3001" \
      -H "Access-Control-Request-Method: POST" \
      -H "Access-Control-Request-Headers: Content-Type" \
      -D -)
    
    if echo "$CORS_RESPONSE" | grep -i "access-control-allow-origin" > /dev/null; then
        echo -e "${GREEN}✓ CORS headers present${NC}"
        log_result "✓ CORS headers: Present"
        
        if echo "$CORS_RESPONSE" | grep -i "access-control-allow-methods" > /dev/null; then
            echo -e "${GREEN}✓ CORS methods header present${NC}"
            log_result "✓ CORS methods header: Present"
        else
            echo -e "${RED}✗ CORS methods header missing${NC}"
            log_result "✗ CORS methods header: Missing"
        fi
        
        if echo "$CORS_RESPONSE" | grep -i "access-control-allow-headers" > /dev/null; then
            echo -e "${GREEN}✓ CORS headers header present${NC}"
            log_result "✓ CORS headers header: Present"
        else
            echo -e "${RED}✗ CORS headers header missing${NC}"
            log_result "✗ CORS headers header: Missing"
        fi
    else
        echo -e "${RED}✗ CORS headers missing${NC}"
        log_result "✗ CORS headers: Missing"
    fi
    
    log_result ""
    echo
}

test_pagination() {
    echo -e "${BLUE}=== Testing Pagination Compatibility ===${NC}"
    log_result "=== Pagination Compatibility Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing pagination parameters (/api/items?page=1&limit=5)${NC}"
    PAGINATION_RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/items?page=1&limit=5")
    HTTP_CODE=$(echo $PAGINATION_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "401" ]; then
        if [ "$HTTP_CODE" = "200" ]; then
            echo -e "${GREEN}✓ Pagination works${NC}"
            log_result "✓ Pagination: HTTP $HTTP_CODE"
            
            PAGINATION_DATA=$(echo $PAGINATION_RESPONSE | sed 's/HTTP_[0-9]*$//')
            if echo "$PAGINATION_DATA" | jq -e '.data.page' > /dev/null 2>&1 && echo "$PAGINATION_DATA" | jq -e '.data.page_size' > /dev/null 2>&1; then
                echo -e "${GREEN}✓ Pagination response format is compatible${NC}"
                log_result "✓ Pagination format: Compatible"
            else
                echo -e "${YELLOW}! Pagination response format may have changed${NC}"
                log_result "! Pagination format: May have changed"
            fi
        else
            echo -e "${YELLOW}! Pagination requires auth (breaking change)${NC}"
            log_result "! Pagination: HTTP $HTTP_CODE (auth required)"
        fi
    else
        echo -e "${RED}✗ Pagination failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Pagination: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_dashboard() {
    echo -e "${BLUE}=== Testing Dashboard Endpoint ===${NC}"
    log_result "=== Dashboard Endpoint Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing dashboard endpoint (/dashboard)${NC}"
    DASHBOARD_RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/dashboard")
    HTTP_CODE=$(echo $DASHBOARD_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓ Dashboard endpoint works${NC}"
        log_result "✓ Dashboard: HTTP $HTTP_CODE"
        
        DASHBOARD_DATA=$(echo $DASHBOARD_RESPONSE | sed 's/HTTP_[0-9]*$//')
        if echo "$DASHBOARD_DATA" | grep -i "html" > /dev/null; then
            echo -e "${GREEN}✓ Dashboard returns HTML${NC}"
            log_result "✓ Dashboard format: HTML"
        else
            echo -e "${YELLOW}! Dashboard format may have changed${NC}"
            log_result "! Dashboard format: May have changed"
        fi
    else
        echo -e "${RED}✗ Dashboard endpoint failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ Dashboard: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

test_error_responses() {
    echo -e "${BLUE}=== Testing Error Response Format ===${NC}"
    log_result "=== Error Response Format Test ==="
    log_result "Timestamp: $(date)"
    
    echo -e "${YELLOW}Testing 404 error response (/api/items/99999)${NC}"
    ERROR_RESPONSE=$(curl -s -w "HTTP_%{http_code}" "$BASE_URL/api/items/99999")
    HTTP_CODE=$(echo $ERROR_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "404" ] || [ "$HTTP_CODE" = "401" ]; then
        echo -e "${GREEN}✓ 404 error response works${NC}"
        log_result "✓ 404 error: HTTP $HTTP_CODE"
        
        ERROR_DATA=$(echo $ERROR_RESPONSE | sed 's/HTTP_[0-9]*$//')
        if echo "$ERROR_DATA" | jq -e '.error' > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Error response format is compatible${NC}"
            log_result "✓ Error format: Compatible"
        else
            echo -e "${YELLOW}! Error response format may have changed${NC}"
            log_result "! Error format: May have changed"
        fi
    else
        echo -e "${RED}✗ 404 error response failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ 404 error: HTTP $HTTP_CODE"
    fi
    
    echo -e "${YELLOW}Testing 400 error response (invalid JSON)${NC}"
    BAD_REQUEST_RESPONSE=$(curl -s -w "HTTP_%{http_code}" -X POST "$BASE_URL/api/items" \
      -H "Content-Type: application/json" \
      -d '{"invalid": json}')
    HTTP_CODE=$(echo $BAD_REQUEST_RESPONSE | grep -o "HTTP_[0-9]*" | cut -d'_' -f2)
    
    if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "401" ]; then
        echo -e "${GREEN}✓ 400 error response works${NC}"
        log_result "✓ 400 error: HTTP $HTTP_CODE"
    else
        echo -e "${RED}✗ 400 error response failed: HTTP $HTTP_CODE${NC}"
        log_result "✗ 400 error: HTTP $HTTP_CODE"
    fi
    
    log_result ""
    echo
}

generate_compatibility_report() {
    echo -e "${BLUE}=== Backward Compatibility Summary ===${NC}"
    log_result "=== Backward Compatibility Summary ==="
    log_result "Test completed at: $(date)"
    
    TOTAL_TESTS=$(grep -c "✓\|✗\|!" "$RESULTS_FILE" 2>/dev/null)
    PASSED_TESTS=$(grep -c "✓" "$RESULTS_FILE" 2>/dev/null)
    FAILED_TESTS=$(grep -c "✗" "$RESULTS_FILE" 2>/dev/null)
    BREAKING_CHANGES=$(grep -c "!" "$RESULTS_FILE" 2>/dev/null)
    
    TOTAL_TESTS=$(echo $TOTAL_TESTS | tr -d '\n\r ')
    PASSED_TESTS=$(echo $PASSED_TESTS | tr -d '\n\r ')
    FAILED_TESTS=$(echo $FAILED_TESTS | tr -d '\n\r ')
    BREAKING_CHANGES=$(echo $BREAKING_CHANGES | tr -d '\n\r ')
    
    echo
    echo "Test Results Summary:"
    echo "  Total Tests: $TOTAL_TESTS"
    echo -e "  ${GREEN}Passed: $PASSED_TESTS${NC}"
    echo -e "  ${RED}Failed: $FAILED_TESTS${NC}"
    echo -e "  ${YELLOW}Breaking Changes: $BREAKING_CHANGES${NC}"
    
    log_result "Test Results Summary:"
    log_result "Total Tests: $TOTAL_TESTS"
    log_result "Passed: $PASSED_TESTS"
    log_result "Failed: $FAILED_TESTS"
    log_result "Breaking Changes: $BREAKING_CHANGES"
    
    if [ "$FAILED_TESTS" -eq 0 ] && [ "$BREAKING_CHANGES" -eq 0 ]; then
        RESULT_MESSAGE="✅ All tests passed! No backward compatibility issues detected."
        EXIT_CODE=0
    elif [ "$FAILED_TESTS" -eq 0 ]; then
        RESULT_MESSAGE="⚠️ Tests passed but breaking changes detected."
        EXIT_CODE=1
    else
        RESULT_MESSAGE="❌ Backward compatibility issues detected."
        EXIT_CODE=1
    fi
    
    echo
    echo -e "${GREEN}$RESULT_MESSAGE${NC}"
    log_result "$RESULT_MESSAGE"
    
    echo
    echo "Detailed results saved to: $RESULTS_FILE"
    echo
    echo "Breaking changes (if any) require:"
    echo "  - API documentation updates"
    echo "  - Client migration guides"
    echo "  - Version deprecation notices"
    echo "Exit Code: $EXIT_CODE"
    
    return $EXIT_CODE
}

main() {
    if ! curl -s "$BASE_URL/health" > /dev/null; then
        echo -e "${RED}Server is not running at $BASE_URL${NC}"
        echo "Please start the server first: cargo run --bin server"
        exit 1
    fi
    
    for tool in curl jq; do
        if ! command -v $tool &> /dev/null; then
            echo -e "${RED}Required tool '$tool' is not installed${NC}"
            exit 1
        fi
    done
    
    test_original_endpoints
    test_legacy_item_operations
    test_form_handling
    test_export_functionality
    test_http_methods
    test_cors_headers
    test_pagination
    test_dashboard
    test_error_responses
    generate_compatibility_report
}

main "$@"