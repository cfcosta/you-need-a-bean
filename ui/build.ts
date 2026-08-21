// Production bundle: `bun run build` → dist/. Uses the API rather than the
// `bun build` CLI because the Tailwind plugin can only be passed here.
import tailwind from "bun-plugin-tailwind";

const result = await Bun.build({
  entrypoints: ["./src/index.html"],
  outdir: "./dist",
  target: "browser",
  minify: true,
  sourcemap: "none",
  define: { "process.env.NODE_ENV": '"production"' },
  plugins: [tailwind],
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
for (const artifact of result.outputs) {
  console.log(`${artifact.path} (${(artifact.size / 1024).toFixed(1)} KiB)`);
}
