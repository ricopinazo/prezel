import React from "react";
import type { DocsThemeConfig } from "nextra-theme-docs";
import { Footer } from "./components/Footer";

const config: DocsThemeConfig = {
  head: (
    <>
      <link rel="icon" href="https://prezel.app/icon.svg" />
    </>
  ),
  // logo: <span>Prezel</span>,
  logo: (
    <>
      <img
        style={{ height: 28, width: 28 }}
        src="https://prezel.app/icon.svg"
      />
      {/* <Logo
        style={{ height: 28, width: 28 }}
        // className="h-12 w-12"
      /> */}
      <span
        style={{
          marginLeft: "1em",
          fontWeight: 600,
        }}
      >
        prezel
      </span>
    </>
  ),
  project: {
    link: "https://github.com/ricopinazo/prezel",
  },
  // chat: {
  //   link: "https://discord.com",
  // },
  docsRepositoryBase: "https://github.com/shuding/nextra-docs-template",
  navbar: {
    // component: <div className="red-500 h-20 w-20" />,
    extraContent: <div className="red-500 h-20 w-20" />,
  },
  footer: {
    content: "Nextra Docs Template",
    component: Footer,
    // text: "",
    // component: () => {
    //   return <Footer />;
    // },
  },
};

export default config;
