# Manual Validation Tests

Run the commands below after you have started running the server (`cargo run --bin server`):

## 🛡️ SQL Injection Tests (Should return 400)

```bash
curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "test'\'''; DROP TABLE items; --", "description": "test"}' \
  -w "\nStatus: %{http_code}\n"

# Test 2: SQL Injection in search
curl "http://localhost:3000/api/items/search?q=test%27%20UNION%20SELECT" \
  -w "\nStatus: %{http_code}\n"
```

## 🚫 XSS Tests (Should return 400)

```bash
curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "<script>alert(\"xss\")</script>", "description": "test"}' \
  -w "\nStatus: %{http_code}\n"

curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "test", "description": "javascript:alert(1)"}' \
  -w "\nStatus: %{http_code}\n"
```

## 📝 Form Validation Tests (Should return 400)

```bash
curl -X POST http://localhost:3000/api/form \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "name=test&email=invalid-email&message=test" \
  -w "\nStatus: %{http_code}\n"

curl -X POST http://localhost:3000/api/form \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "name=<script>alert(1)</script>&email=test@example.com&message=test" \
  -w "\nStatus: %{http_code}\n"
```

## 🔍 Input Validation Tests (Should return 400)

```bash
curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "", "description": "test"}' \
  -w "\nStatus: %{http_code}\n"

curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "'$(printf 'A%.0s' {1..300})'", "description": "test"}' \
  -w "\nStatus: %{http_code}\n"
```

## ✅ Valid Request Tests (Should return 200/201)

```bash
curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "Valid Test Item", "description": "This is a valid test"}' \
  -w "\nStatus: %{http_code}\n"

curl -X POST http://localhost:3000/api/form \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "name=John Doe&email=john@example.com&message=Hello" \
  -w "\nStatus: %{http_code}\n"

curl http://localhost:3000/api/items \
  -w "\nStatus: %{http_code}\n"

curl "http://localhost:3000/api/items/search?q=test" \
  -w "\nStatus: %{http_code}\n"
```

## 🎯 Expected Results

- **Tests 1-8**: Should return `400` (Bad Request) - Security validation working
- **Tests 9-12**: Should return `200` or `201` (Success) - Valid requests allowed