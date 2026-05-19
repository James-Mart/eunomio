import { Agent, CursorAgentError } from "@cursor/sdk";

function emit(event) {
  process.stdout.write(JSON.stringify(event) + "\n");
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  const buf = Buffer.concat(chunks);
  return buf.toString("utf8");
}

export async function run() {
  let request;
  try {
    const raw = await readStdin();
    request = JSON.parse(raw);
  } catch (err) {
    emit({
      type: "error",
      runId: 0,
      code: "bad_request",
      message: err instanceof Error ? err.message : String(err),
    });
    process.exit(1);
  }

  const { model, cwd, prompt, runId } = request;
  if (typeof runId !== "number") {
    emit({ type: "error", runId: 0, code: "bad_request", message: "runId required" });
    process.exit(1);
  }

  let agent;
  let liveRun;
  let cancelled = false;
  let cleanupDone = false;

  const cleanup = async () => {
    if (cleanupDone) return;
    cleanupDone = true;
    if (agent) {
      try {
        await agent[Symbol.asyncDispose]?.();
      } catch (err) {
        // best effort
      }
    }
  };

  const onSignal = async () => {
    if (cancelled) return;
    cancelled = true;
    try {
      if (liveRun && liveRun.supports?.("cancel")) {
        await liveRun.cancel();
      }
    } catch (err) {
      // ignore
    }
    emit({ type: "cancelled", runId });
    await cleanup();
    process.exit(0);
  };

  process.on("SIGTERM", () => {
    onSignal().catch(() => process.exit(0));
  });
  process.on("SIGINT", () => {
    onSignal().catch(() => process.exit(0));
  });

  try {
    agent = await Agent.create({
      apiKey: process.env.CURSOR_API_KEY,
      model: { id: model },
      local: { cwd },
    });
    emit({ type: "started", runId, agentId: agent.agentId ?? "" });

    liveRun = await agent.send(prompt);
    const stream = liveRun.stream?.();
    if (stream) {
      for await (const event of stream) {
        if (cancelled) break;
        emit({ type: "sdkMessage", runId, message: event });
      }
    }
    const result = await liveRun.wait();
    if (cancelled) {
      await cleanup();
      process.exit(0);
    }
    if (result.status === "error") {
      emit({
        type: "error",
        runId,
        code: "run_error",
        message: result.error?.message ?? "run failed",
      });
    } else if (result.status === "cancelled") {
      emit({ type: "cancelled", runId });
    } else {
      const text = extractFinalText(result);
      emit({
        type: "finished",
        runId,
        result: text,
        durationMs: result.durationMs ?? null,
      });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const code = err instanceof CursorAgentError ? "startup_failed" : "internal";
    emit({ type: "error", runId, code, message });
  } finally {
    await cleanup();
  }
  process.exit(0);
}

function extractFinalText(result) {
  if (typeof result.result === "string") return result.result;
  const finalMessage = result.finalMessage ?? result.message;
  if (finalMessage?.content) {
    const parts = [];
    for (const block of finalMessage.content) {
      if (block.type === "text" && typeof block.text === "string") parts.push(block.text);
    }
    if (parts.length > 0) return parts.join("");
  }
  return "";
}
