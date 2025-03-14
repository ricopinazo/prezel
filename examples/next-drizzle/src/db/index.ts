import "dotenv/config";
import { drizzle } from "drizzle-orm/libsql";
import * as schema from "./schema";

export const db = drizzle({
  connection: {
    url: process.env.PREZEL_LIBSQL_URL!,
    authToken: process.env.PREZEL_LIBSQL_AUTH_TOKEN,
  },
  schema,
});
