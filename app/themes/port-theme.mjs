// Copies one theme's colour tokens between tokenwhy and rich presence+.
// Usage: node port-theme.mjs <to-rp|from-rp> <theme id> [rich presence+ desktop dir]
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const [dir, id, rp = "D:/CODE_PROJECTS/rich_presence_mod/apps/desktop"] = process.argv.slice(2);
if (!["to-rp", "from-rp"].includes(dir) || !/^[a-z]+(-dark)?$/.test(id || "")) {
  console.error("usage: node port-theme.mjs <to-rp|from-rp> <theme id> [rich presence+ desktop dir]");
  process.exit(1);
}
const tw = join(import.meta.dirname, "tokens", `${id}.css`);
const other = join(rp, "src", "themes", `${id}.css`);
const [src, dst] = dir === "to-rp" ? [tw, other] : [other, tw];
if (!existsSync(src)) { console.error(`not found: ${src}`); process.exit(1); }

const root = `:root[data-theme="${id}"]`;
const both = `${root}, [data-preview="${id}"]`;
const sel = (css) => (dir === "from-rp" ? css.replaceAll(both, root).replaceAll(root, both) : css.replaceAll(both, root));
const TOKEN = /(--[a-z0-9-]+)\s*:\s*([^;]+);/g;
const srcCss = readFileSync(src, "utf8");
let css;
if (!existsSync(dst)) css = sel(srcCss);
else {
  const old = readFileSync(dst, "utf8");
  writeFileSync(`${dst}.bak`, old);
  const vals = new Map([...srcCss.matchAll(TOKEN)].map((m) => [m[1], m[2].trim()]));
  const seen = new Set();
  css = old.replace(TOKEN, (all, name) => {
    seen.add(name);
    return vals.has(name) ? `${name}: ${vals.get(name)};` : all;
  });
  const added = [...vals].filter(([n]) => !seen.has(n)).map(([n, v]) => `  ${n}: ${v};`);
  if (added.length) css = css.replace(/\n\}/, `\n${added.join("\n")}\n}`);
}
writeFileSync(dst, css);
console.log(`${src} -> ${dst}${existsSync(`${dst}.bak`) ? ` (previous kept as ${id}.css.bak)` : ""}`);
