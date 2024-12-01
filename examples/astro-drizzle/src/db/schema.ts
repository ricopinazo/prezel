import { text, int, sqliteTable } from "drizzle-orm/sqlite-core";

export const test = sqliteTable("test", {
  id: int("id").primaryKey(),
  test: text("test"),
});
