// @ts-check
import { defineConfig } from "astro/config";

import db from "@astrojs/db";

import node from "@astrojs/node";

import tailwind from "@astrojs/tailwind";

console.log("------------------->>>>>>>>>>>");

// https://astro.build/config
const config = defineConfig({
  integrations: [db(), tailwind()],

  adapter: node({
    mode: "standalone",
  }),
});

console.log(config);
console.log("-------");
console.log(config.integrations?.at(0));

export default config;
