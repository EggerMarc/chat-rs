import alchemy from "alchemy";
import { TanStackStart } from "alchemy/cloudflare";

const app = await alchemy("chat-rs-docs");

export const website = await TanStackStart("website", {
  // Keep an existing Worker of the same name instead of erroring on deploy.
  adopt: true,
  // Serve from the custom domain. Cloudflare creates the DNS record and
  // issues the TLS cert automatically; the chat-rs.com zone must be on
  // this Cloudflare account.
  domains: ["chat-rs.com"],
});

console.log({
  url: website.url,
});

await app.finalize();
