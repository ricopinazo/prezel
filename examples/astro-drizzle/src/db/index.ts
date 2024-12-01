import "dotenv/config";
import { drizzle } from "drizzle-orm/better-sqlite3";
import Database from "better-sqlite3";
import assert from "assert";

const url = process.env.DATABASE_URL;
assert(typeof url === "string");

const sqlite = new Database(url);
export const db = drizzle(sqlite);
