# Solana Transaction Bundler API Documentation

This document provides comprehensive documentation for the Solana Transaction Bundler HTTP API.

## Base URL

```
http://localhost:8080
```

## Authentication

Currently, the API does not require authentication. In production deployments, consider implementing:
- API key authentication
- JWT tokens
- IP whitelisting
- Rate limiting per client

## Content Type

All requests and responses use `application/json` content type unless otherwise specified.

## Error Handling

The API uses standard HTTP status codes and returns structured error responses:

```json
{
  "error": "Brief error description",
  "details": "Detailed error information (optional)"
}
```

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | Bad Request - Invalid input |
| 404 | Not Found - Resource not found |
| 500 | Internal Server Error |
| 503 | Service Unavailable - Health check failed |

## Endpoints

### 1. Submit Bundle

Submit a bundle of transactions for processing.

**Endpoint:** `POST /v1/bundle`

**Request Body:**

```json
{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "atomic": true,
  "compute": {
    "limit": "auto",
    "price": "auto", 
    "max_price_lamports": 10000
  },
  "alt_tables": [],
  "instructions": [
    {
      "program_id": "11111111111111111111111111111112",
      "keys": [
        {
          "pubkey": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
          "is_signer": true,
          "is_writable": true
        },
        {
          "pubkey": "2nr1bHFT86W9tGnyvmYW4vcHKsQB3sVQfnddasz4kExM",
          "is_signer": false,
          "is_writable": true
        }
      ],
      "data_b64": "AgAAAOgDAAAAAAAA"
    }
  ],
  "signers": [],
  "metadata": {
    "description": "Simple SOL transfer",
    "priority": "normal"
  }
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `request_id` | UUID | Yes | Unique identifier for the bundle request |
| `atomic` | boolean | Yes | If true, all transactions must succeed |
| `compute.limit` | string/number | Yes | "auto" or specific compute unit limit |
| `compute.price` | string/number | Yes | "auto" or specific price in lamports |
| `compute.max_price_lamports` | number | Yes | Maximum price willing to pay |
| `alt_tables` | array | No | Address lookup table pubkeys |
| `instructions` | array | Yes | Array of instruction data |
| `signers` | array | No | Additional signers configuration |
| `metadata` | object | No | Optional metadata for tracking |

**Instruction Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `program_id` | string | Yes | Program ID as base58 string |
| `keys` | array | Yes | Account metadata array |
| `data_b64` | string | Yes | Instruction data as base64 |

**Account Metadata Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pubkey` | string | Yes | Account public key as base58 |
| `is_signer` | boolean | Yes | Whether account must sign |
| `is_writable` | boolean | Yes | Whether account is writable |

**Response:**

```json
{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "Success",
  "transactions": [
    {
      "signature": "5j7s8K9jGzQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQ",
      "slot": 123456789,
      "status": "Finalized",
      "compute_units_consumed": 50000,
      "fee_paid_lamports": 5000,
      "logs": ["Program log: Success"],
      "error": null
    }
  ],
  "logs_url": "/logs/550e8400-e29b-41d4-a716-446655440000",
  "metrics": {
    "total_latency_ms": 1500,
    "simulation_time_ms": 200,
    "signing_time_ms": 50,
    "submission_time_ms": 800,
    "confirmation_time_ms": 450,
    "retry_attempts": 0,
    "rpc_endpoints_used": ["https://api.mainnet-beta.solana.com"]
  },
  "completed_at": "2024-01-15T10:30:00Z"
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `request_id` | UUID | Original request identifier |
| `status` | string | Bundle status: "Success", "Partial", "Failed" |
| `transactions` | array | Array of transaction results |
| `logs_url` | string | URL to detailed logs |
| `metrics` | object | Performance metrics |
| `completed_at` | string | ISO 8601 completion timestamp |

### 2. Simulate Bundle

Simulate a bundle without submitting to the network.

**Endpoint:** `POST /v1/bundle/simulate`

**Request Body:** Same as submit bundle

**Response:**

```json
{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "simulations": [
    {
      "instruction_index": 0,
      "success": true,
      "compute_units_consumed": 50000,
      "estimated_fee": 5000,
      "logs": ["Program log: Success"],
      "error": null
    }
  ]
}
```

### 3. Transaction Status

Get the status of a specific transaction.

**Endpoint:** `GET /v1/status/{signature}`

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `signature` | string | Yes | Transaction signature as base58 |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `verbose` | boolean | No | Include detailed logs and metadata |

**Response:**

```json
{
  "signature": "5j7s8K9jGzQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQZQtjbCjjjQQ",
  "status": "finalized",
  "slot": 123456789,
  "fee": 5000,
  "compute_units": 50000,
  "logs": ["Program log: Success"],
  "error": null
}
```

### 4. Health Check

Check the health status of the bundler service.

**Endpoint:** `GET /v1/health`

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `verbose` | boolean | No | Include detailed component status |

**Response (Healthy):**

```json
{
  "healthy": true,
  "timestamp": "2024-01-15T10:30:00Z",
  "components": {
    "rpc": {
      "healthy": true,
      "message": "All endpoints responding",
      "last_success": "2024-01-15T10:29:45Z"
    },
    "signing": {
      "healthy": true,
      "message": "Fee payer accessible",
      "last_success": "2024-01-15T10:29:50Z"
    },
    "fees": {
      "healthy": true,
      "message": "Fee calculation operational",
      "last_success": "2024-01-15T10:29:55Z"
    }
  }
}
```

**Response (Unhealthy):**

HTTP Status: 503 Service Unavailable

```json
{
  "healthy": false,
  "timestamp": "2024-01-15T10:30:00Z",
  "components": {
    "rpc": {
      "healthy": false,
      "message": "Primary endpoint timeout",
      "last_success": "2024-01-15T10:25:00Z"
    }
  }
}
```

### 5. Service Information

Get general information about the bundler service.

**Endpoint:** `GET /v1/info`

**Response:**

```json
{
  "name": "Solana Transaction Bundler",
  "version": "0.1.0",
  "description": "Production-ready Solana transaction bundler with low latency and high success rate",
  "endpoints": {
    "submit": "POST /v1/bundle",
    "simulate": "POST /v1/bundle/simulate",
    "status": "GET /v1/status/{signature}",
    "health": "GET /v1/health",
    "info": "GET /v1/info"
  },
  "documentation": "https://github.com/your-org/solana-bundler"
}
```

### 6. Root Endpoint

Basic service information at the root path.

**Endpoint:** `GET /`

**Response:**

```json
{
  "message": "Solana Transaction Bundler API",
  "version": "0.1.0",
  "endpoints": {
    "info": "GET /v1/info",
    "health": "GET /v1/health"
  }
}
```

## Examples

### Basic SOL Transfer

```bash
curl -X POST http://localhost:8080/v1/bundle \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "atomic": true,
    "compute": {
      "limit": "auto",
      "price": "auto",
      "max_price_lamports": 10000
    },
    "instructions": [
      {
        "program_id": "11111111111111111111111111111112",
        "keys": [
          {
            "pubkey": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "is_signer": true,
            "is_writable": true
          },
          {
            "pubkey": "2nr1bHFT86W9tGnyvmYW4vcHKsQB3sVQfnddasz4kExM",
            "is_signer": false,
            "is_writable": true
          }
        ],
        "data_b64": "AgAAAOgDAAAAAAAA"
      }
    ]
  }'
```

### Token Transfer

```bash
curl -X POST http://localhost:8080/v1/bundle \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "550e8400-e29b-41d4-a716-446655440001",
    "atomic": true,
    "compute": {
      "limit": 200000,
      "price": 2000,
      "max_price_lamports": 20000
    },
    "instructions": [
      {
        "program_id": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "keys": [
          {
            "pubkey": "TokenAccountFrom",
            "is_signer": false,
            "is_writable": true
          },
          {
            "pubkey": "TokenAccountTo", 
            "is_signer": false,
            "is_writable": true
          },
          {
            "pubkey": "AuthorityPubkey",
            "is_signer": true,
            "is_writable": false
          }
        ],
        "data_b64": "AwAAAOgDAAAAAAAA"
      }
    ]
  }'
```

### Multiple Instructions (Atomic)

```bash
curl -X POST http://localhost:8080/v1/bundle \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "550e8400-e29b-41d4-a716-446655440002",
    "atomic": true,
    "compute": {
      "limit": "auto",
      "price": "auto",
      "max_price_lamports": 50000
    },
    "instructions": [
      {
        "program_id": "11111111111111111111111111111112",
        "keys": [...],
        "data_b64": "..."
      },
      {
        "program_id": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "keys": [...],
        "data_b64": "..."
      }
    ],
    "metadata": {
      "description": "Swap operation",
      "priority": "high"
    }
  }'
```

## Rate Limiting

The API implements rate limiting to prevent abuse:

- **Default**: 1000 requests per minute per IP
- **Burst**: Up to 100 requests in 10 seconds
- **Headers**: Rate limit information in response headers

```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1642248000
```

## WebSocket Support (Future)

Real-time updates for bundle status will be available via WebSocket:

```javascript
const ws = new WebSocket('ws://localhost:8080/v1/ws');
ws.send(JSON.stringify({
  type: 'subscribe',
  request_id: '550e8400-e29b-41d4-a716-446655440000'
}));
```

## SDK Integration

Official SDKs are planned for:

- **TypeScript/JavaScript**
- **Python**
- **Go**
- **Rust**

## Error Codes

| Code | Description | Resolution |
|------|-------------|------------|
| `INVALID_INSTRUCTION` | Malformed instruction data | Check base64 encoding and program ID |
| `SIMULATION_FAILED` | Transaction simulation failed | Review instruction parameters |
| `INSUFFICIENT_FUNDS` | Not enough SOL for fees | Add more SOL to fee payer |
| `PROGRAM_NOT_WHITELISTED` | Program not in whitelist | Contact administrator |
| `RATE_LIMIT_EXCEEDED` | Too many requests | Implement backoff strategy |
| `SERVICE_UNAVAILABLE` | Bundler service unhealthy | Check service status |

## Best Practices

1. **Always include request_id** for tracking and debugging
2. **Use simulation first** for testing new instruction combinations
3. **Implement retry logic** with exponential backoff
4. **Monitor rate limits** and implement client-side throttling
5. **Set reasonable timeouts** for HTTP requests (30-60 seconds)
6. **Handle partial failures** gracefully when atomic=false
7. **Use verbose mode** for debugging transaction failures

## Support

For API support and questions:
- **Documentation**: [GitHub Wiki](https://github.com/your-org/solana-bundler/wiki)
- **Issues**: [GitHub Issues](https://github.com/your-org/solana-bundler/issues)
- **API Support**: api-support@yourorg.com
