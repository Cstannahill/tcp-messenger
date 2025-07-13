# OneWheel Server Implementation Guide

## 📋 **Overview**

This guide outlines the complete server-side implementation needed to support the OneWheel mobile app's ride analytics and AI analysis features. The server handles ride data storage, AI processing, and provides comprehensive insights back to the mobile app.

## 🏗️ **Architecture Requirements**

### **Core Components**
- **REST API Server** (Node.js/Python/Go)
- **Database** (PostgreSQL/MongoDB)
- **AI Analysis Engine** (OpenAI/Custom ML)
- **Authentication System**
- **Rate Limiting & Security**

### **Deployment**
- **Cloud Platform** (AWS/Heroku/Railway)
- **Container Support** (Docker)
- **Auto-scaling** (for high load)
- **Monitoring & Logging**

## 🔌 **API Endpoints**

### **1. Upload Ride Data**
```
POST /api/rides
Content-Type: application/json
Authorization: Bearer {api_key}
```

**Request Body:**
```json
{
  "id": "1673123456789",
  "startTime": "2025-01-08T10:30:00.000Z",
  "endTime": "2025-01-08T11:15:00.000Z",
  "distance": 8.5,
  "maxSpeed": 18.2,
  "avgSpeed": 12.4,
  "duration": 2700,
  "route": [
    {"lat": 37.7749, "lng": -122.4194},
    {"lat": 37.7750, "lng": -122.4195},
    {"lat": 37.7751, "lng": -122.4196}
  ],
  "startBattery": 95,
  "endBattery": 68,
  "notes": "Beach ride - amazing sunset",
  "metadata": {
    "appVersion": "1.0.0",
    "deviceType": "mobile",
    "uploadTime": "2025-01-08T11:16:00.000Z"
  }
}
```

**Response:**
```json
{
  "success": true,
  "rideId": "1673123456789",
  "message": "Ride data uploaded successfully",
  "analysisQueued": true,
  "estimatedAnalysisTime": "30-45 seconds"
}
```

### **2. Request AI Analysis**
```
POST /api/analyze-ride
Content-Type: application/json
Authorization: Bearer {api_key}
```

**Request Body:**
```json
{
  "rideId": "1673123456789",
  "analysisType": "comprehensive",
  "includeComparisons": true,
  "includeSuggestions": true
}
```

**Response (Complete Analysis):**
```json
{
  "rideId": "1673123456789",
  "overallScore": 78.5,
  "speedEfficiency": 68.2,
  "energyEfficiency": 85.3,
  "routeQuality": 92.1,
  "rideStyle": "Balanced Rider",
  "insights": [
    "🚀 Excellent speed consistency throughout the ride",
    "🔋 Great battery efficiency - you're getting max range",
    "🗺️ Complex route with good GPS tracking quality"
  ],
  "suggestions": [
    "Try maintaining steady acceleration for even better efficiency",
    "Consider exploring longer routes to build endurance",
    "Your braking technique is smooth - keep it up!"
  ],
  "comparisons": [
    "This ride was 15% longer than your average",
    "Speed was slightly above your typical pace",
    "Battery efficiency improved by 8% compared to last week"
  ],
  "generatedAt": "2025-01-08T11:16:45.000Z",
  "aiMetrics": {
    "accelerationPattern": "smooth",
    "brakingScore": 85,
    "terrainDifficulty": "moderate",
    "weatherImpact": "minimal"
  },
  "moodAnalysis": "Relaxed and confident riding style with good control",
  "safetyTips": [
    "Great job maintaining safe speeds in traffic areas",
    "Consider wearing additional protective gear for longer rides",
    "Your route planning shows good safety awareness"
  ],
  "skillProgression": {
    "balance": 82.0,
    "speed_control": 75.5,
    "efficiency": 88.2,
    "route_planning": 79.0
  }
}
```

### **3. Get User Statistics** (Optional)
```
GET /api/users/{userId}/stats
Authorization: Bearer {api_key}
```

**Response:**
```json
{
  "totalRides": 24,
  "totalDistance": 156.8,
  "totalTime": 43200,
  "averageSpeed": 11.2,
  "bestRide": {
    "rideId": "1673123456789",
    "score": 92.1,
    "date": "2025-01-08T10:30:00.000Z"
  },
  "skillTrends": {
    "balance": {"current": 82.0, "trend": "+5.2"},
    "efficiency": {"current": 88.2, "trend": "+12.1"}
  }
}
```

## 🗄️ **Database Schema**

### **Rides Table**
```sql
CREATE TABLE rides (
    id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255),
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP,
    distance DECIMAL(8,2),
    max_speed DECIMAL(5,2),
    avg_speed DECIMAL(5,2),
    duration INTEGER,
    route JSON,
    start_battery INTEGER,
    end_battery INTEGER,
    notes TEXT,
    metadata JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_rides_user_id ON rides(user_id);
CREATE INDEX idx_rides_start_time ON rides(start_time);
CREATE INDEX idx_rides_created_at ON rides(created_at);
```

### **Ride Insights Table**
```sql
CREATE TABLE ride_insights (
    id SERIAL PRIMARY KEY,
    ride_id VARCHAR(255) REFERENCES rides(id) ON DELETE CASCADE,
    overall_score DECIMAL(5,2),
    speed_efficiency DECIMAL(5,2),
    energy_efficiency DECIMAL(5,2),
    route_quality DECIMAL(5,2),
    ride_style VARCHAR(100),
    insights JSON,
    suggestions JSON,
    comparisons JSON,
    ai_metrics JSON,
    mood_analysis TEXT,
    safety_tips JSON,
    skill_progression JSON,
    analysis_type VARCHAR(20) DEFAULT 'ai',
    generated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processing_time_ms INTEGER
);

-- Indexes
CREATE INDEX idx_insights_ride_id ON ride_insights(ride_id);
CREATE INDEX idx_insights_generated_at ON ride_insights(generated_at);
```

### **Users Table** (Optional)
```sql
CREATE TABLE users (
    id VARCHAR(255) PRIMARY KEY,
    email VARCHAR(255) UNIQUE,
    api_key VARCHAR(255) UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_active TIMESTAMP,
    settings JSON
);
```

## 🧠 **AI Analysis Logic**

### **Core Analysis Functions**

#### **1. Speed Analysis**
```javascript
function analyzeSpeedPatterns(rideData) {
  const speeds = extractSpeedDataFromRoute(rideData.route);
  
  return {
    consistency: calculateSpeedConsistency(speeds),
    accelerationPattern: analyzeAcceleration(speeds),
    brakingPattern: analyzeBraking(speeds),
    speedZones: categorizeSpeedZones(speeds)
  };
}
```

#### **2. Energy Efficiency**
```javascript
function calculateEnergyEfficiency(rideData) {
  const batteryUsed = rideData.startBattery - rideData.endBattery;
  const efficiency = rideData.distance / batteryUsed;
  
  return {
    efficiency: efficiency,
    grade: getEfficiencyGrade(efficiency),
    comparison: compareToAverage(efficiency),
    tips: getEfficiencyTips(efficiency)
  };
}
```

#### **3. Route Quality Assessment**
```javascript
function assessRouteQuality(route) {
  return {
    gpsAccuracy: calculateGPSAccuracy(route),
    pointDensity: calculatePointDensity(route),
    smoothness: calculateRouteSmoothness(route),
    complexity: assessRouteComplexity(route)
  };
}
```

### **AI Integration Options**

#### **Option 1: OpenAI Integration**
```javascript
async function generateAIInsights(rideData, basicAnalysis) {
  const prompt = `
    Analyze this OneWheel ride data and provide insights:
    
    Distance: ${rideData.distance} miles
    Duration: ${rideData.duration} seconds
    Max Speed: ${rideData.maxSpeed} mph
    Battery: ${rideData.startBattery}% → ${rideData.endBattery}%
    
    Basic Analysis:
    - Speed Efficiency: ${basicAnalysis.speedEfficiency}%
    - Energy Efficiency: ${basicAnalysis.energyEfficiency}%
    
    Provide:
    1. 3 key insights about riding performance
    2. 3 specific improvement suggestions
    3. Safety assessment and tips
    4. Riding mood/style analysis
    
    Format as JSON with insights, suggestions, safetyTips, and moodAnalysis arrays.
  `;

  const response = await openai.chat.completions.create({
    model: "gpt-4",
    messages: [
      {
        role: "system",
        content: "You are an expert OneWheel riding coach and safety instructor."
      },
      {
        role: "user", 
        content: prompt
      }
    ],
    temperature: 0.7,
    max_tokens: 1000
  });

  return JSON.parse(response.choices[0].message.content);
}
```

#### **Option 2: Custom ML Models**
```python
import tensorflow as tf
import numpy as np

class RideAnalysisModel:
    def __init__(self):
        self.model = tf.keras.models.load_model('ride_analysis_model.h5')
    
    def analyze_ride(self, ride_data):
        # Extract features from ride data
        features = self.extract_features(ride_data)
        
        # Run through ML model
        predictions = self.model.predict(features)
        
        return {
            'overall_score': float(predictions[0][0]),
            'ride_style': self.classify_style(predictions[0][1:]),
            'efficiency_score': float(predictions[0][5])
        }
    
    def extract_features(self, ride_data):
        # Convert ride data to feature vector
        return np.array([[
            ride_data['distance'],
            ride_data['avg_speed'],
            ride_data['max_speed'],
            ride_data['duration'],
            len(ride_data['route'])
        ]])
```

## 🔐 **Security Implementation**

### **API Authentication**
```javascript
const jwt = require('jsonwebtoken');

function authenticateAPI(req, res, next) {
  const token = req.headers.authorization?.split(' ')[1];
  
  if (!token) {
    return res.status(401).json({ error: 'No API key provided' });
  }
  
  try {
    const decoded = jwt.verify(token, process.env.JWT_SECRET);
    req.user = decoded;
    next();
  } catch (error) {
    return res.status(401).json({ error: 'Invalid API key' });
  }
}
```

### **Rate Limiting**
```javascript
const rateLimit = require('express-rate-limit');

const apiLimiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100, // Limit each IP to 100 requests per windowMs
  message: 'Too many requests, please try again later.',
  standardHeaders: true,
  legacyHeaders: false,
});

const analysisLimiter = rateLimit({
  windowMs: 60 * 1000, // 1 minute
  max: 5, // Limit analysis requests to 5 per minute
  message: 'Analysis rate limit exceeded',
});
```

### **Data Validation**
```javascript
const Joi = require('joi');

const rideSchema = Joi.object({
  id: Joi.string().required(),
  startTime: Joi.date().iso().required(),
  endTime: Joi.date().iso(),
  distance: Joi.number().min(0).max(1000),
  maxSpeed: Joi.number().min(0).max(50),
  avgSpeed: Joi.number().min(0).max(50),
  duration: Joi.number().min(0),
  route: Joi.array().items(
    Joi.object({
      lat: Joi.number().min(-90).max(90).required(),
      lng: Joi.number().min(-180).max(180).required()
    })
  ),
  startBattery: Joi.number().min(0).max(100),
  endBattery: Joi.number().min(0).max(100)
});
```

## 📊 **Performance Requirements**

### **Response Time Targets**
- **Ride Upload**: < 2 seconds
- **AI Analysis**: < 45 seconds
- **Cached Insights**: < 500ms

### **Scalability Targets**
- **Concurrent Users**: 1,000+
- **Daily Rides**: 10,000+
- **Analysis Queue**: Process 100 rides/minute

### **Caching Strategy**
```javascript
const Redis = require('redis');
const redis = Redis.createClient();

// Cache insights for 24 hours
async function cacheInsights(rideId, insights) {
  await redis.setex(`insights:${rideId}`, 86400, JSON.stringify(insights));
}

async function getCachedInsights(rideId) {
  const cached = await redis.get(`insights:${rideId}`);
  return cached ? JSON.parse(cached) : null;
}
```

## 🚀 **Deployment Guide**

### **Docker Configuration**
```dockerfile
FROM node:18-alpine

WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production

COPY . .
EXPOSE 3000

CMD ["npm", "start"]
```

### **Environment Variables**
```env
NODE_ENV=production
PORT=3000
DATABASE_URL=postgresql://user:pass@host:5432/onewheel
REDIS_URL=redis://host:6379
JWT_SECRET=your-secret-key
OPENAI_API_KEY=your-openai-key
API_RATE_LIMIT=100
ANALYSIS_RATE_LIMIT=5
```

### **Health Check Endpoint**
```javascript
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    timestamp: new Date().toISOString(),
    version: process.env.APP_VERSION,
    database: 'connected',
    redis: 'connected'
  });
});
```

## 📈 **Monitoring & Analytics**

### **Metrics to Track**
- API response times
- Analysis processing time
- Database query performance
- AI service availability
- Error rates by endpoint
- User activity patterns

### **Logging Structure**
```javascript
const winston = require('winston');

const logger = winston.createLogger({
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json()
  ),
  transports: [
    new winston.transports.File({ filename: 'error.log', level: 'error' }),
    new winston.transports.File({ filename: 'combined.log' })
  ]
});

// Log ride analysis
logger.info('Ride analysis completed', {
  rideId: rideData.id,
  userId: rideData.userId,
  processingTime: endTime - startTime,
  analysisType: 'ai'
});
```

## 🔄 **Backup & Recovery**

### **Database Backups**
```bash
# Daily automated backup
pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql

# Backup rotation (keep 30 days)
find /backups -name "backup_*.sql" -mtime +30 -delete
```

### **Data Retention Policy**
- **Ride Data**: Keep indefinitely (user owns their data)
- **Insights**: Keep indefinitely (valuable for user progress)
- **Logs**: Keep 90 days
- **API Keys**: Rotate every 365 days

## 📋 **Development Checklist**

### **Phase 1: Core API**
- [ ] Set up basic Express/FastAPI server
- [ ] Implement ride upload endpoint
- [ ] Set up database schema
- [ ] Add basic authentication
- [ ] Implement rate limiting

### **Phase 2: Analysis Engine**
- [ ] Create local analysis functions
- [ ] Integrate OpenAI API
- [ ] Implement insights generation
- [ ] Add caching layer
- [ ] Create analysis endpoint

### **Phase 3: Production Ready**
- [ ] Add comprehensive error handling
- [ ] Implement monitoring & logging
- [ ] Set up automated testing
- [ ] Configure CI/CD pipeline
- [ ] Deploy to cloud platform

### **Phase 4: Advanced Features**
- [ ] User management system
- [ ] Advanced ML models
- [ ] Real-time notifications
- [ ] Analytics dashboard
- [ ] Performance optimization

## 🧪 **Testing Strategy**

### **Unit Tests**
```javascript
describe('Ride Analysis', () => {
  test('calculates speed efficiency correctly', () => {
    const rideData = {
      avgSpeed: 12.0,
      maxSpeed: 15.0
    };
    
    const efficiency = calculateSpeedEfficiency(rideData);
    expect(efficiency).toBe(80.0);
  });
});
```

### **Integration Tests**
```javascript
describe('API Endpoints', () => {
  test('POST /api/rides uploads ride successfully', async () => {
    const response = await request(app)
      .post('/api/rides')
      .set('Authorization', 'Bearer valid-token')
      .send(validRideData);
    
    expect(response.status).toBe(201);
    expect(response.body.success).toBe(true);
  });
});
```

This comprehensive guide provides everything needed to implement a production-ready server that seamlessly integrates with your OneWheel mobile app's analytics features.
