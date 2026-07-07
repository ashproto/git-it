import type { ThemeRegistrationRaw } from "shiki";

// Minimal NERV-tuned dark theme: bone text on the void, orange keywords, phosphor
// strings, restrained accents. A raw object → no extra bundled-theme chunk, offline.
export const nervShikiTheme: ThemeRegistrationRaw = {
  name: "nerv",
  type: "dark",
  colors: { "editor.background": "#0A0C0F", "editor.foreground": "#EAE6DA" },
  settings: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#7C8794", fontStyle: "italic" } },
    { scope: ["string", "string.quoted", "constant.other.symbol"], settings: { foreground: "#46E88B" } },
    { scope: ["keyword", "storage", "storage.type", "keyword.control"], settings: { foreground: "#F2542D" } },
    { scope: ["entity.name.function", "support.function", "meta.function-call"], settings: { foreground: "#E8A33D" } },
    { scope: ["entity.name.type", "support.type", "support.class", "entity.name.class"], settings: { foreground: "#5AA9E6" } },
    { scope: ["constant.numeric", "constant.language"], settings: { foreground: "#9B7FE0" } },
    { scope: ["variable", "meta.definition.variable"], settings: { foreground: "#EAE6DA" } },
    { scope: ["entity.name.tag"], settings: { foreground: "#F2542D" } },
    { scope: ["entity.other.attribute-name"], settings: { foreground: "#E0608A" } },
  ],
};
