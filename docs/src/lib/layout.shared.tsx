import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

/**
 * Shared layout options (nav title, links) used by the docs layout.
 */
export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: "chat-rs",
    },
    links: [
      {
        text: "Documentation",
        url: "/docs",
      },
    ],
    githubUrl: "https://github.com/EggerMarc/chat-rs",
  };
}
