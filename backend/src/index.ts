import express from "express";
import cors from "cors";
import helmet from "helmet";
import { config } from "./config";
import routes from "./routes";
import { apiRateLimiter, authRateLimiter } from "./middleware/rate-limit";
import { sanitizeInput } from "./middleware/sanitize";
import { errorHandler } from "./middleware/error";

const app = express();

const corsOptions: cors.CorsOptions = {
  origin: (origin, callback) => {
    if (!origin || origin === config.frontendUrl) {
      callback(null, true);
      return;
    }

    callback(new Error("Not allowed by CORS"));
  },
};

// Security middleware
app.use(helmet());
app.use(cors(corsOptions));
app.use(express.json());
app.use(sanitizeInput);

// Health check
app.get("/health", (_req, res) => {
  res.json({ status: "ok", service: "stellarmarket-api" });
});

// Rate limiting
app.use("/api/auth/login", authRateLimiter);
app.use("/api/auth/register", authRateLimiter);
app.use("/api", apiRateLimiter);

// API routes
app.use("/api", routes);

// 404 handler
app.use((_req, res) => {
  res.status(404).json({ error: "Route not found." });
});

// Error handler
app.use(errorHandler);

app.listen(config.port, () => {
  console.log(`StellarMarket API running on port ${config.port}`);
});

export default app;
