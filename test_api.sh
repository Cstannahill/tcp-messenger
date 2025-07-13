#!/bin/bash

# OneWheel API Test Script
# This script demonstrates how to use the OneWheel API endpoints

API_BASE="http://localhost:80"
AUTH_TOKEN="ow_demo_12345678901234567890123456789012"

echo "=== OneWheel API Test Script ==="
echo ""

# 1. Test Health Check
echo "1. Testing Health Check..."
echo "   Request: GET /health"
curl -s -X GET "$API_BASE/health" | jq '.'
echo ""

# 2. Test Upload Ride
echo "2. Testing Ride Upload..."
echo "   Request: POST /api/rides"

RIDE_DATA='{
  "id": "test_ride_001",
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
  "notes": "Test ride for API demonstration",
  "metadata": {
    "rideMode": "Classic",
    "firmwareVersion": "4149",
    "boardModel": "XR",
    "deviceId": "test_device_001"
  }
}'

curl -s -X POST "$API_BASE/api/rides" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -H "Content-Type: application/json" \
  -d "$RIDE_DATA" | jq '.'
echo ""

# 3. Test another ride upload
echo "3. Testing Second Ride Upload..."
RIDE_DATA_2='{
  "id": "test_ride_002",
  "startTime": "2025-07-12T14:00:00Z",
  "endTime": "2025-07-12T14:30:00Z",
  "distance": 3.1,
  "maxSpeed": 16.8,
  "avgSpeed": 11.2,
  "duration": 1800,
  "startBattery": 88,
  "endBattery": 65,
  "notes": "Second test ride"
}'

curl -s -X POST "$API_BASE/api/rides" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -H "Content-Type: application/json" \
  -d "$RIDE_DATA_2" | jq '.'
echo ""

# 4. Test List Rides
echo "4. Testing List Rides..."
echo "   Request: GET /api/rides?page=1&pageSize=10"
curl -s -X GET "$API_BASE/api/rides?page=1&pageSize=10" \
  -H "Authorization: Bearer $AUTH_TOKEN" | jq '.'
echo ""

# 5. Test Get Specific Ride
echo "5. Testing Get Specific Ride..."
echo "   Request: GET /api/rides/test_ride_001"
curl -s -X GET "$API_BASE/api/rides/test_ride_001" \
  -H "Authorization: Bearer $AUTH_TOKEN" | jq '.'
echo ""

# 6. Test Authentication Error
echo "6. Testing Authentication Error..."
echo "   Request: GET /api/rides (without auth)"
curl -s -X GET "$API_BASE/api/rides" | jq '.'
echo ""

# 7. Test Not Found Error
echo "7. Testing Not Found Error..."
echo "   Request: GET /api/rides/nonexistent_ride"
curl -s -X GET "$API_BASE/api/rides/nonexistent_ride" \
  -H "Authorization: Bearer $AUTH_TOKEN" | jq '.'
echo ""

echo "=== Test Complete ==="
echo ""
echo "For more detailed documentation, see API_USAGE_GUIDE.md"
