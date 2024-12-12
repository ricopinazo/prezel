import { defineDb, defineTable, column } from "astro:db";

const Message = defineTable({
  columns: {
    id: column.number({ primaryKey: true }),
    created: column.date(),
    content: column.text(),
  },
});

// https://astro.build/db/config
export default defineDb({
  tables: { Message },
});
