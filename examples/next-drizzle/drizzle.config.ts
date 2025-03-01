import "dotenv/config";
import { defineConfig } from "drizzle-kit";

export default defineConfig({
  out: "./drizzle",
  schema: "./src/db/schema.ts",
  dialect: "turso",
  dbCredentials: {
    url: process.env.PREZEL_DB_URL!,
    authToken: process.env.PREZEL_DB_AUTH_TOKEN,
  },
});
