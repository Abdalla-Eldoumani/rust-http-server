#!/bin/bash

# Essential Middleware Tests
set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SERVER_PID=""

cleanup() {
    if [ ! -z "$SERVER_PID" ] && kill -0 $SERVER_PID 2>/dev/null; then
        kill $SERVER_PID
        wait $SERVER_PID 2>/dev/null || true
    fi
    rm -f server.log
}

trap cleanup EXIT

main() {
    log_info "Quick Task 12 Middleware Test"
    log_info "=============================="
    
    log_info "Building..."
    cargo build --bin server --quiet
    
    log_info "Starting server..."
    export RUST_LOG=info
    export APP_RATE_LIMIT_REQUESTS_PER_MINUTE=3
    cargo run --bin server > server.log 2>&1 &
    SERVER_PID=$!
    sleep 3
    
    log_info "Testing basic functionality..."
    
    if curl -s http://localhost:3000/ > /dev/null; then
        log_success "✓ Server is responding"
    else
        log_error "✗ Server not responding"
        exit 1
    fi
    
    local cors_headers=$(curl -s -I http://localhost:3000/ 2>/dev/null || echo "")
    if echo "$cors_headers" | grep -qi "access-control"; then
        log_success "✓ CORS headers present"
    else
        log_error "✗ CORS headers missing"
        echo "Headers received: $cors_headers"
    fi
    
    local rate_headers=$(curl -s -I http://localhost:3000/ 2>/dev/null || echo "")
    if echo "$rate_headers" | grep -qi "x-ratelimit"; then
        log_success "✓ Rate limit headers present"
    else
        log_error "✗ Rate limit headers missing"
        echo "Headers received: $rate_headers"
    fi
    
    local req_headers=$(curl -s -I http://localhost:3000/ 2>/dev/null || echo "")
    if echo "$req_headers" | grep -qi "x-request-id"; then
        log_success "✓ Request ID header present"
    else
        log_error "✗ Request ID header missing"
        echo "Headers received: $req_headers"
    fi
    
    log_info "Testing rate limiting..."
    local hit_limit=false
    for i in {1..61}; do
        local response=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:3000/ 2>/dev/null)
        if [ "$response" = "429" ]; then
            hit_limit=true
            break
        fi
        sleep 0.3
    done
    
    if [ "$hit_limit" = true ]; then
        log_success "✓ Rate limiting is working"
    else
        log_error "✗ Rate limiting not working"
    fi
    
    if grep -q "Configuration loaded successfully" server.log; then
        log_success "✓ Configuration loaded"
    else
        log_error "✗ Configuration loading failed"
    fi
    
    if grep -q "Rate limiter cleanup task" server.log 2>/dev/null; then
        log_success "✓ Rate limiter cleanup task started"
    elif grep -q "cleanup task" server.log 2>/dev/null; then
        log_success "✓ Rate limiter cleanup task started"
    else
        log_error "✗ Rate limiter cleanup task not started"
        echo "Log contents:"
        tail -5 server.log 2>/dev/null || echo "No server.log found"
    fi
    
    log_info "=============================="
    log_success "Quick test completed!"
}

main "$@"