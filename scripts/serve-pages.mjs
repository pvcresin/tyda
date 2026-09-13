import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const portIndex = process.argv.indexOf("--port");
const port = Number(portIndex === -1 ? process.env.PORT || 8123 : process.argv[portIndex + 1]);
const root = resolve(fileURLToPath(new URL("../pages-dist", import.meta.url)));

const contentTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".woff2": "font/woff2",
};

function requestedPath(requestUrl) {
  const pathname = decodeURIComponent(new URL(requestUrl, "http://localhost").pathname);
  const filePath = resolve(root, `.${pathname}`);
  if (filePath !== root && !filePath.startsWith(`${root}${sep}`)) return null;
  return filePath;
}

async function findFile(filePath) {
  const candidates = [filePath];
  if (!filePath.endsWith(sep)) {
    candidates.push(`${filePath}.html`, join(filePath, "index.html"));
  } else {
    candidates.push(join(filePath, "index.html"));
  }

  for (const candidate of candidates) {
    try {
      if ((await stat(candidate)).isFile()) return candidate;
    } catch {
      // Try the next static route candidate.
    }
  }
  return null;
}

const server = createServer(async (request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { Allow: "GET, HEAD" });
    response.end();
    return;
  }

  try {
    const requested = requestedPath(request.url || "/");
    const filePath = requested && (await findFile(requested));
    if (!filePath) {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("Not found\n");
      return;
    }

    const body = await readFile(filePath);
    response.writeHead(200, {
      "Content-Length": body.byteLength,
      "Content-Type": contentTypes[extname(filePath)] || "application/octet-stream",
    });
    if (request.method === "HEAD") response.end();
    else response.end(body);
  } catch {
    response.writeHead(400, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Bad request\n");
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Serving ${relative(process.cwd(), root)} at http://localhost:${port}/`);
});
