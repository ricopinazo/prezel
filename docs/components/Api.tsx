import { useEffect, useState } from "react";
import { ApiReferenceReact } from "@scalar/api-reference-react";
import "@scalar/api-reference-react/style.css";
import { ThemeSwitch } from "nextra-theme-docs";
import { Footer } from "./Footer";

export default function Api() {
  const [darkMode, setDarkMode] = useState(false);

  const mqListener = (e: MediaQueryListEvent) => {
    console.log("system:", e.matches);
    setDarkMode(e.matches);
  };

  // TODO: read changes to 'theme' local storage key coming from nextra

  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: dark)");
    system.addListener(mqListener);
    return () => system.removeListener(mqListener);
  }, []);

  useEffect(() => {
    const footers = document.getElementsByTagName("footer");
    for (const element of footers) {
      const typed: HTMLDivElement = element as HTMLDivElement; // TODO: type as Footer instead of as Div
      if (typed.id !== "api") {
        typed.style.display = "none";
      }
    }
    return () => {
      const footers = document.getElementsByTagName("footer");
      for (const element of footers) {
        const typed: HTMLDivElement = element as HTMLDivElement; // TODO: type as Footer instead of as Div
        if (typed.id !== "api") {
          typed.style.display = "block";
        }
      }
    };
  });

  return (
    <div style={{ height: "calc(100vh - 64px)" }}>
      <ApiReferenceReact
        // style={{ height: "200px" }}
        configuration={{
          spec: {
            url: "/openapi.json",
          },
          hideDarkModeToggle: true,
          darkMode,
        }}
      />
      <Footer menu api />
      {/* <div
        // TODO: render the footer inside here
        style={{
          display: "flex",
          marginLeft: "auto",
          marginRight: "auto",
          paddingBottom: "10px",
        }}
      >
        <div style={{ position: "relative" }}>
          <ThemeSwitch />
        </div>
      </div> */}
    </div>
  );
}
