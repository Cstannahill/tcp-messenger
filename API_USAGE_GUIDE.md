# OneWheel API Usage Guide

## Overview

The OneWheel API is a RESTful service for managing and analyzing OneWheel ride data. It provides endpoints for uploading ride data, retrieving rides, and performing analytics.

**Base URL**: `http://localhost:80` (when running locally)
**Content-Type**: `application/json`
**Authentication**: Bearer token (required for all endpoints except health check)

## Authentication

All API endpoints (except `/health`) require a Bearer token in the Authorization header:

```http
Authorization: Bearer your_token_here
```

**Demo Token**: `ow_demo_12345678901234567890123456789012`

> **Note**: In this implementation, the token is used directly as the user ID for simplicity. In production, you would validate JWT tokens and extract the actual user ID.

## Endpoints

### 1. Health Check

**Endpoint**: `GET /health`
**Authentication**: Not required
**Description**: Check if the API service is running

#### Request
```http
GET /health
```

#### Response
```json
{
  "status": "healthy",
  "timestamp": "2025-07-12T16:45:30.123Z",
  "service": "onewheel-api",
  "version": "1.0.0"
}
```

#### Status Codes
- `200 OK`: Service is healthy

---

### 2. Upload Ride Data

**Endpoint**: `POST /api/rides`
**Authentication**: Required
**Description**: Upload new ride data for analysis and storage

#### Request
```http
POST /api/rides
Authorization: Bearer ow_demo_12345678901234567890123456789012
Content-Type: application/json
```

#### Request Body
```json
{
  "id": "ride_20250712_001",
  "startTime": "2025-07-12T10:00:00Z",
  "endTime": "2025-07-12T10:45:00Z",
  "distance": 5.2,
  "maxSpeed": 18.5,
  "avgSpeed": 12.3,
  "duration": 2700,
  "route": [
    {
      "lat": 37.7749,
      "lng": -122.4194,
      "timestamp": "2025-07-12T10:00:00Z",
      "speed": 0.0,
      "battery": 95
    },
    {
      "lat": 37.7751,
      "lng": -122.4192,
      "timestamp": "2025-07-12T10:00:30Z",
      "speed": 15.2,
      "battery": 94
    }
  ],
  "startBattery": 95,
  "endBattery": 72,
  "notes": "Great ride through Golden Gate Park",
  "metadata": {
    "rideMode": "Classic",
    "firmwareVersion": "4149",
    "boardModel": "XR",
    "deviceId": "onewheel_abc123"
  }
}
```

#### Request Fields
- `id` (string, required): Unique identifier for the ride
- `startTime` (string, required): ISO 8601 timestamp of ride start
- `endTime` (string, optional): ISO 8601 timestamp of ride end
- `distance` (number, optional): Total distance in miles
- `maxSpeed` (number, optional): Maximum speed reached in mph
- `avgSpeed` (number, optional): Average speed in mph
- `duration` (number, optional): Ride duration in seconds
- `route` (array, optional): Array of GPS points
  - `lat` (number): Latitude coordinate
  - `lng` (number): Longitude coordinate
  - `timestamp` (string, optional): ISO 8601 timestamp for this point
  - `speed` (number, optional): Speed at this point in mph
  - `battery` (number, optional): Battery percentage at this point
- `startBattery` (number, optional): Starting battery percentage
- `endBattery` (number, optional): Ending battery percentage
- `notes` (string, optional): User notes about the ride
- `metadata` (object, optional): Additional ride metadata

#### Response
```json
{
  "rideId": "ride_20250712_001"
}
```

#### Status Codes
- `201 Created`: Ride uploaded successfully
- `400 Bad Request`: Invalid request data
- `401 Unauthorized`: Missing or invalid authentication
- `500 Internal Server Error`: Server error during processing

---

### 3. List User Rides

**Endpoint**: `GET /api/rides`
**Authentication**: Required
**Description**: Retrieve a paginated list of rides for the authenticated user

#### Request
```http
GET /api/rides?page=1&pageSize=20
Authorization: Bearer ow_demo_12345678901234567890123456789012
```

#### Query Parameters
- `page` (number, optional): Page number (default: 1)
- `pageSize` (number, optional): Number of rides per page (default: 20, max: 100)

#### Response
```json
{
  "rides": [
    {
      "id": "ride_20250712_001",
      "startTime": "2025-07-12T10:00:00Z",
      "endTime": "2025-07-12T10:45:00Z",
      "distance": 5.2,
      "duration": 2700,
      "maxSpeed": 18.5,
      "avgSpeed": 12.3,
      "topSpeed": 18.5
    },
    {
      "id": "ride_20250711_003",
      "startTime": "2025-07-11T15:30:00Z",
      "endTime": "2025-07-11T16:15:00Z",
      "distance": 3.8,
      "duration": 2100,
      "maxSpeed": 16.2,
      "avgSpeed": 10.8,
      "topSpeed": 16.2
    }
  ],
  "total": 2,
  "page": 1,
  "pageSize": 20
}
```

#### Response Fields
- `rides` (array): Array of simplified ride objects
  - `id` (string): Ride identifier
  - `startTime` (string): Ride start time
  - `endTime` (string): Ride end time
  - `distance` (number): Distance in miles
  - `duration` (number): Duration in seconds
  - `maxSpeed` (number): Maximum speed in mph
  - `avgSpeed` (number): Average speed in mph
  - `topSpeed` (number): Top speed reached in mph
- `total` (number): Total number of rides returned
- `page` (number): Current page number
- `pageSize` (number): Number of items per page

#### Status Codes
- `200 OK`: Rides retrieved successfully
- `401 Unauthorized`: Missing or invalid authentication
- `500 Internal Server Error`: Server error during retrieval

---

### 4. Get Specific Ride

**Endpoint**: `GET /api/rides/{ride_id}`
**Authentication**: Required
**Description**: Retrieve detailed information for a specific ride

#### Request
```http
GET /api/rides/ride_20250712_001
Authorization: Bearer ow_demo_12345678901234567890123456789012
```

#### Path Parameters
- `ride_id` (string, required): The unique identifier of the ride

#### Response
```json
{
  "id": "ride_20250712_001",
  "startTime": "2025-07-12T10:00:00Z",
  "endTime": "2025-07-12T10:45:00Z",
  "distance": 5.2,
  "maxSpeed": 18.5,
  "avgSpeed": 12.3,
  "duration": 2700,
  "route": [
    {
      "lat": 37.7749,
      "lng": -122.4194,
      "timestamp": "2025-07-12T10:00:00Z",
      "speed": 0.0,
      "battery": 95
    }
  ],
  "startBattery": 95,
  "endBattery": 72,
  "notes": "Great ride through Golden Gate Park",
  "metadata": {
    "rideMode": "Classic",
    "firmwareVersion": "4149",
    "boardModel": "XR",
    "deviceId": "onewheel_abc123"
  }
}
```

#### Status Codes
- `200 OK`: Ride retrieved successfully
- `401 Unauthorized`: Missing or invalid authentication
- `403 Forbidden`: Ride belongs to another user
- `404 Not Found`: Ride not found
- `500 Internal Server Error`: Server error during retrieval

---

## Usage Examples

### cURL Examples

#### 1. Health Check
```bash
curl -X GET http://localhost:80/health
```

#### 2. Upload a Ride
```bash
curl -X POST http://localhost:80/api/rides \
  -H "Authorization: Bearer ow_demo_12345678901234567890123456789012" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "ride_example_001",
    "startTime": "2025-07-12T10:00:00Z",
    "endTime": "2025-07-12T10:45:00Z",
    "distance": 5.2,
    "maxSpeed": 18.5,
    "avgSpeed": 12.3,
    "duration": 2700
  }'
```

#### 3. List Rides
```bash
curl -X GET "http://localhost:80/api/rides?page=1&pageSize=10" \
  -H "Authorization: Bearer ow_demo_12345678901234567890123456789012"
```

#### 4. Get Specific Ride
```bash
curl -X GET http://localhost:80/api/rides/ride_example_001 \
  -H "Authorization: Bearer ow_demo_12345678901234567890123456789012"
```

### JavaScript/Fetch Examples

#### Upload a Ride
```javascript
const rideData = {
  id: "ride_js_001",
  startTime: "2025-07-12T10:00:00Z",
  endTime: "2025-07-12T10:45:00Z",
  distance: 5.2,
  maxSpeed: 18.5,
  avgSpeed: 12.3,
  duration: 2700
};

const response = await fetch('http://localhost:80/api/rides', {
  method: 'POST',
  headers: {
    'Authorization': 'Bearer ow_demo_12345678901234567890123456789012',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify(rideData)
});

const result = await response.json();
console.log('Ride uploaded:', result.rideId);
```

#### List Rides
```javascript
const response = await fetch('http://localhost:80/api/rides?page=1&pageSize=10', {
  headers: {
    'Authorization': 'Bearer ow_demo_12345678901234567890123456789012'
  }
});

const data = await response.json();
console.log(`Found ${data.total} rides:`);
data.rides.forEach(ride => {
  console.log(`- ${ride.id}: ${ride.distance} miles, ${ride.duration}s`);
});
```

### Python Requests Examples

#### Upload a Ride
```python
import requests
import json
from datetime import datetime

ride_data = {
    "id": "ride_python_001",
    "startTime": "2025-07-12T10:00:00Z",
    "endTime": "2025-07-12T10:45:00Z",
    "distance": 5.2,
    "maxSpeed": 18.5,
    "avgSpeed": 12.3,
    "duration": 2700
}

headers = {
    "Authorization": "Bearer ow_demo_12345678901234567890123456789012",
    "Content-Type": "application/json"
}

response = requests.post(
    "http://localhost:80/api/rides",
    headers=headers,
    json=ride_data
)

if response.status_code == 201:
    result = response.json()
    print(f"Ride uploaded successfully: {result['rideId']}")
else:
    print(f"Error: {response.status_code} - {response.text}")
```

#### Get Rides List
```python
import requests

headers = {
    "Authorization": "Bearer ow_demo_12345678901234567890123456789012"
}

params = {
    "page": 1,
    "pageSize": 20
}

response = requests.get(
    "http://localhost:80/api/rides",
    headers=headers,
    params=params
)

if response.status_code == 200:
    data = response.json()
    print(f"Found {data['total']} rides:")
    for ride in data['rides']:
        print(f"- {ride['id']}: {ride['distance']} miles")
else:
    print(f"Error: {response.status_code} - {response.text}")
```

## Error Handling

### Common Error Responses

#### 401 Unauthorized
```json
{
  "error": "Authentication required"
}
```

#### 403 Forbidden
```json
{
  "error": "Access denied"
}
```

#### 404 Not Found
```json
{
  "error": "Ride not found"
}
```

#### 500 Internal Server Error
```json
{
  "error": "Failed to store ride"
}
```

### Best Practices

1. **Always include the Authorization header** for protected endpoints
2. **Use proper Content-Type** (`application/json`) for POST requests
3. **Handle HTTP status codes** appropriately in your client code
4. **Implement retry logic** for temporary server errors (5xx)
5. **Validate data** before sending to reduce 400 errors
6. **Use pagination** for listing endpoints to avoid large responses

## Development Setup

### Database Requirements

The OneWheel API requires PostgreSQL 12+ with JSONB support. The API will automatically create the required tables and indexes on first startup.

#### Required Database Setup

1. **Install PostgreSQL** (version 12 or higher)
2. **Create a database**:
   ```sql
   CREATE DATABASE onewheel;
   CREATE USER onewheel_user WITH PASSWORD 'your_password';
   GRANT ALL PRIVILEGES ON DATABASE onewheel TO onewheel_user;
   ```

3. **Set the DATABASE_URL environment variable**:
   ```bash
   export DATABASE_URL="postgresql://onewheel_user:your_password@localhost:5432/onewheel"
   ```

#### Database Schema

The API automatically creates these tables:
- **`rides`**: Main ride data with user isolation
  - Uses BIGINT timestamps (Unix epoch seconds) for reliable timezone handling
  - JSONB fields for route GPS data and metadata
  - Indexed on user_id, start_time, and created_at for performance
  
- **`ride_insights`**: Analysis results and AI-generated insights
  - Links to rides with CASCADE delete
  - Stores efficiency scores, suggestions, and analysis metadata
  
- **`users`**: User management and API key storage
  - Supports email and API key authentication
  - JSONB settings field for user preferences

### Starting the Server

```bash
# With default database settings
DATABASE_URL="postgresql://postgres:password@localhost:5432/onewheel" \
cargo run --bin tcp-messaging onewheel

# With custom database URL
DATABASE_URL="postgresql://user:pass@host:port/db" \
cargo run --bin tcp-messaging onewheel

# Without database (in-memory only, no persistence)
DATABASE_URL="none" cargo run --bin tcp-messaging onewheel
```

### Environment Variables
- `DATABASE_URL`: PostgreSQL connection string (optional)
  - Default: `postgresql://postgres:password@localhost/onewheel`
  - Set to `none` to disable database persistence

### Database Schema
The API uses PostgreSQL with the following table structure:
- `rides`: Stores ride data with JSON fields for route and metadata
- Automatic UUID generation for ride IDs
- User isolation through user_id field

## Rate Limiting and Limits

- **Maximum page size**: 100 rides per request
- **Default page size**: 20 rides per request
- **Authentication**: Required for all endpoints except health check
- **Request size**: No explicit limit, but keep GPS routes reasonable

## Future Endpoints (Coming Soon)

- `POST /api/analyze-ride` - Analyze ride for insights
- `GET /api/users/:id/stats` - Get user riding statistics
- `GET /api/rides/:id/analysis` - Get ride analysis results
- `POST /api/rides/:id/share` - Generate shareable ride links

For questions or support, please refer to the project documentation or create an issue in the repository.
