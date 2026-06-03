import alchemy from "alchemy";
import { TanStackStart } from "alchemy/cloudflare";

const app = await alchemy("chat-rs-docs");

export const website = await TanStackStart("website", {
  adopt: true,
  domains: ["chat-rs.com"],
});

console.log({
  url: website.url,
});

await app.finalize();
