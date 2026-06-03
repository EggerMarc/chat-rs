import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

/**
 * Shared layout options (nav title, links) used by the docs layout.
 */
export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: <span className="font-label text-base tracking-tight">chat-rs</span>,
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
