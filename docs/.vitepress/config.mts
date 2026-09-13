import { defineConfig } from "vitepress";

const repository = "https://github.com/pvcresin/tyda";

export default defineConfig({
  lang: "en-US",
  title: "Tyda",
  description: "Type inference tool for lazy Rubyists",
  base: "/tyda/",
  cleanUrls: true,
  rewrites: (id) =>
    id === "index.md" ? "index.md" : id === "README.md" ? "docs/index.md" : `docs/${id}`,
  themeConfig: {
    nav: [
      { text: "Home", link: "/" },
      { text: "Playground", link: "/play/", target: "_self" },
      { text: "Documentation", link: "/docs/" },
    ],
    sidebar: {
      "/docs/": [
        {
          text: "Documentation",
          items: [
            { text: "Index", link: "/docs/" },
            { text: "Design", link: "/docs/design" },
            { text: "Architecture", link: "/docs/architecture" },
            { text: "Features", link: "/docs/features" },
            { text: "Capability matrix", link: "/docs/capability-matrix" },
            { text: "Testing", link: "/docs/testing" },
            { text: "Performance", link: "/docs/performance" },
            { text: "Development guide", link: "/docs/development" },
            { text: "Incomplete code policy", link: "/docs/incomplete-code-policy" },
            { text: "Roadmap", link: "/docs/roadmap" },
          ],
        },
      ],
    },
    socialLinks: [{ icon: "github", link: repository }],
  },
});
