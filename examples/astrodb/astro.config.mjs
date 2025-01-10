// @ts-check
import { defineConfig } from "astro/config";
import db from "@astrojs/db";
import node from "@astrojs/node";
import tailwind from "@astrojs/tailwind";

// https://astro.build/config
const config = defineConfig({
  integrations: [db(), tailwind()],
  adapter: node({
    mode: "standalone",
  }),
  security: {
    checkOrigin: false,
  },
});

export default config;
