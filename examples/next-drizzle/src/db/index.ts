import "dotenv/config";
import { drizzle } from "drizzle-orm/libsql";
import * as schema from "./schema";

export const db = drizzle({
  connection: {
    url: process.env.PREZEL_DB_URL!,
    authToken: process.env.PREZEL_DB_AUTH_TOKEN,
  },
  schema,
});
