/**
 * Eval runner for the LLM dictation cleanup pass.
 *
 * Usage:
 *   bun scripts/cleanup-eval.ts [--model slug] [--effort low] [--filter substr]
 *                               [--concurrency 4] [--dry-run]
 *
 * Requires OPENROUTER_API_KEY (not needed for --dry-run). Requests mirror the
 * app: cached system prompt, <context>/<transcript> user message, no sampling
 * params and no assistant prefill (Claude Haiku 5.5 rejects both).
 */
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const CASES_PATH = `${ROOT}tests/cleanup_cases.yaml`;
const PROMPT_PATH = `${ROOT}src-tauri/src/cleanup/system_prompt.md`;
const ENDPOINT = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL = "anthropic/claude-haiku-5.5";
const PASS_THRESHOLD = 0.9;

interface EvalCase {
  id: string;
  style: string;
  app?: string;
  window?: string;
  vocabulary?: string[];
  input: string;
  expect?: string;
  contains?: string[];
  not_contains?: string[];
  max_chars?: number;
}

interface Usage {
  prompt_tokens?: number;
  completion_tokens?: number;
  cost?: number;
  prompt_tokens_details?: { cached_tokens?: number };
}

interface ChatResponse {
  choices?: { message?: { content?: string | null } }[];
  usage?: Usage;
  error?: { message?: string };
}

interface ChatRequest {
  model: string;
  max_tokens: number;
  messages: unknown[];
  reasoning: { effort: string; exclude?: boolean };
}

interface Options {
  model: string;
  effort: string;
  filter: string;
  concurrency: number;
  dryRun: boolean;
}

interface Result {
  id: string;
  pass: boolean;
  failures: string[];
  output: string;
  latencyMs: number;
  usage: Usage;
}

function parseArgs(argv: string[]): Options {
  const opts: Options = {
    model: DEFAULT_MODEL,
    effort: "low",
    filter: "",
    concurrency: 4,
    dryRun: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = (): string => {
      const value = argv[++i];
      if (value === undefined) throw new Error(`Missing value for ${arg}`);
      return value;
    };
    if (arg === "--model") opts.model = next();
    else if (arg === "--effort") opts.effort = next();
    else if (arg === "--filter") opts.filter = next();
    else if (arg === "--concurrency")
      opts.concurrency = Math.max(1, Number.parseInt(next(), 10) || 4);
    else if (arg === "--dry-run") opts.dryRun = true;
    else throw new Error(`Unknown argument: ${arg}`);
  }
  return opts;
}

function buildUserMessage(c: EvalCase): string {
  const lines = [`STYLE: ${c.style}`, `APP: ${c.app ?? "Unknown"}`];
  if (c.window) lines.push(`WINDOW: ${c.window}`);
  if (c.vocabulary && c.vocabulary.length > 0)
    lines.push(`VOCABULARY: ${c.vocabulary.join(", ")}`);
  return `<context>\n${lines.join("\n")}\n</context>\n<transcript>\n${c.input}\n</transcript>`;
}

function buildRequest(c: EvalCase, system: string, opts: Options): ChatRequest {
  return {
    model: opts.model,
    max_tokens: 1024,
    messages: [
      {
        role: "system",
        content: [
          { type: "text", text: system, cache_control: { type: "ephemeral" } },
        ],
      },
      { role: "user", content: buildUserMessage(c) },
    ],
    // OpenRouter: effort "none" disables reasoning (rejected by models where
    // reasoning is mandatory); otherwise reason at the effort but hide it.
    reasoning:
      opts.effort === "none"
        ? { effort: "none" }
        : { effort: opts.effort, exclude: true },
  };
}

const norm = (s: string): string => s.replace(/\s+/g, " ").trim();

function check(c: EvalCase, output: string): string[] {
  const failures: string[] = [];
  if (c.expect !== undefined && norm(output) !== norm(c.expect))
    failures.push(`expected exactly ${JSON.stringify(norm(c.expect))}`);
  for (const s of c.contains ?? [])
    if (!output.includes(s)) failures.push(`missing ${JSON.stringify(s)}`);
  for (const s of c.not_contains ?? [])
    if (output.includes(s)) failures.push(`unexpected ${JSON.stringify(s)}`);
  if (c.max_chars !== undefined && output.length > c.max_chars)
    failures.push(`length ${output.length} > ${c.max_chars}`);
  return failures;
}

async function runCase(
  c: EvalCase,
  system: string,
  opts: Options,
  apiKey: string,
): Promise<Result> {
  const started = performance.now();
  let output = "";
  let usage: Usage = {};
  let failures: string[];
  try {
    const res = await fetch(ENDPOINT, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(buildRequest(c, system, opts)),
    });
    const body = (await res.json()) as ChatResponse;
    if (!res.ok || body.error) {
      failures = [`HTTP ${res.status}: ${body.error?.message ?? "error"}`];
    } else {
      output = body.choices?.[0]?.message?.content ?? "";
      usage = body.usage ?? {};
      failures = check(c, output);
    }
  } catch (e) {
    failures = [
      `request failed: ${e instanceof Error ? e.message : String(e)}`,
    ];
  }
  return {
    id: c.id,
    pass: failures.length === 0,
    failures,
    output,
    latencyMs: performance.now() - started,
    usage,
  };
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.ceil(p * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

async function main(): Promise<number> {
  const opts = parseArgs(process.argv.slice(2));
  const apiKey = process.env.OPENROUTER_API_KEY;
  if (!apiKey && !opts.dryRun) {
    console.error(
      "OPENROUTER_API_KEY is not set. Export it (https://openrouter.ai/keys) and re-run,\nor use --dry-run to inspect the requests without sending them.",
    );
    return 2;
  }

  const system = (await readFile(PROMPT_PATH, "utf8")).trim();
  const parsed: unknown = parse(await readFile(CASES_PATH, "utf8"));
  if (!Array.isArray(parsed))
    throw new Error("cleanup_cases.yaml must be a list");
  const cases = (parsed as EvalCase[]).filter((c) =>
    c.id.includes(opts.filter),
  );
  if (cases.length === 0) {
    console.error(`No cases match filter "${opts.filter}"`);
    return 2;
  }

  if (opts.dryRun) {
    console.log(`Parsed ${cases.length} cases. First 2 requests:`);
    for (const c of cases.slice(0, 2)) {
      const req = buildRequest(c, system, opts);
      const printable = JSON.parse(JSON.stringify(req)) as ChatRequest;
      console.log(`\n# ${c.id}\n${JSON.stringify(printable, null, 2)}`);
    }
    return 0;
  }

  console.log(
    `Model ${opts.model}, effort ${opts.effort}, ${cases.length} cases\n`,
  );
  const results: Result[] = new Array<Result>(cases.length);
  let next = 0;
  const worker = async (): Promise<void> => {
    while (next < cases.length) {
      const i = next++;
      const r = await runCase(cases[i], system, opts, apiKey ?? "");
      results[i] = r;
      const ms = `${Math.round(r.latencyMs)}ms`.padStart(7);
      console.log(`${r.pass ? "PASS" : "FAIL"} ${ms}  ${r.id}`);
      if (!r.pass) {
        console.log(`       ${r.failures.join("; ")}`);
        console.log(`       output: ${JSON.stringify(r.output)}`);
      }
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(opts.concurrency, cases.length) }, worker),
  );

  const passed = results.filter((r) => r.pass).length;
  const rate = passed / results.length;
  const lat = results.map((r) => r.latencyMs).sort((a, b) => a - b);
  const sum = (f: (u: Usage) => number | undefined): number =>
    results.reduce((acc, r) => acc + (f(r.usage) ?? 0), 0);
  const costs = results.filter((r) => r.usage.cost !== undefined);

  console.log("\n--- summary ---");
  console.log(
    `pass rate: ${passed}/${results.length} (${(rate * 100).toFixed(1)}%)`,
  );
  console.log(
    `latency p50 ${Math.round(percentile(lat, 0.5))}ms, p95 ${Math.round(percentile(lat, 0.95))}ms`,
  );
  console.log(
    `tokens: prompt ${sum((u) => u.prompt_tokens)}, cached ${sum((u) => u.prompt_tokens_details?.cached_tokens)}, completion ${sum((u) => u.completion_tokens)}`,
  );
  if (costs.length > 0) console.log(`cost: $${sum((u) => u.cost).toFixed(6)}`);
  return rate < PASS_THRESHOLD ? 1 : 0;
}

main().then(
  (code) => process.exit(code),
  (e: unknown) => {
    console.error(e instanceof Error ? e.message : e);
    process.exit(2);
  },
);
