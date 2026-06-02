import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

/**
 * Shared layout options (nav title, links) used by the docs layout.
 */
export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="flex items-center gap-2">
          <img
            src="/icon-light.webp"
            alt=""
            className="brand-on-dark h-5 w-5"
          />
          <img
            src="/icon-dark.webp"
            alt=""
            className="brand-on-light h-5 w-5"
          />
          <span className="font-semibold tracking-tight">chat-rs</span>
        </span>
      ),
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
