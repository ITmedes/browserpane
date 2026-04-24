import { promises as fs } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { materializeSourceSnapshot } from "./source-snapshot.js";
import { WorkflowControlClient } from "./workflow-control-client.js";
import type {
  GatewayAutomationTaskLogStream,
  GatewayWorkflowRunResource,
  WorkflowRunnerContext,
} from "./types.js";

type WorkflowWorkerServiceOptions = {
  runId: string;
  workRoot: string;
  controlClient: WorkflowControlClient;
};

type WorkflowExecutionResult = {
  output: unknown | null;
  stdoutLines: string[];
  stderrLines: string[];
  error: string | null;
};

export class WorkflowWorkerService {
  private readonly runId: string;
  private readonly workRoot: string;
  private readonly controlClient: WorkflowControlClient;

  constructor(options: WorkflowWorkerServiceOptions) {
    this.runId = options.runId.trim();
    this.workRoot = options.workRoot;
    this.controlClient = options.controlClient;
  }

  async run(): Promise<void> {
    const run = await this.controlClient.getWorkflowRun(this.runId);
    const version = await this.controlClient.getWorkflowDefinitionVersion(
      run.workflow_definition_id,
      run.workflow_version,
    );
    if (version.executor !== "playwright") {
      throw new Error(
        `workflow worker only supports executor=playwright, got ${version.executor}`,
      );
    }
    if (!run.source_snapshot) {
      throw new Error(`workflow run ${run.id} does not expose a source snapshot`);
    }

    const automationAccess = await this.controlClient.issueAutomationAccess(run.session_id);
    const automationToken = automationAccess.token;
    const workDir = this.resolveWorkDir(run.id);

    try {
      await this.controlClient.transitionWorkflowRun(run.id, automationToken, {
        state: "starting",
        message: "workflow worker bootstrapping",
      });
      const snapshotBytes = await this.controlClient.downloadSourceSnapshot(
        run.id,
        automationToken,
      );
      const sourceRoot = (
        await materializeSourceSnapshot({
          archiveBytes: snapshotBytes,
          destinationRoot: path.join(workDir, "source"),
        })
      ).rootPath;
      const entrypointPath = path.join(
        sourceRoot,
        run.source_snapshot.entrypoint.replaceAll("/", path.sep),
      );
      await fs.access(entrypointPath);
      await this.controlClient.appendWorkflowRunLog(
        run.id,
        automationToken,
        "system",
        `materialized workflow source snapshot to ${sourceRoot}`,
      );

      await this.controlClient.transitionWorkflowRun(run.id, automationToken, {
        state: "running",
        message: "workflow worker executing entrypoint",
      });
      await this.controlClient.appendWorkflowRunLog(
        run.id,
        automationToken,
        "system",
        `executing workflow entrypoint ${run.source_snapshot.entrypoint}`,
      );

      const result = await this.executeWorkflowEntrypoint({
        run,
        endpointUrl: automationAccess.automation.endpoint_url,
        authHeader: automationAccess.automation.auth_header,
        authToken: automationToken,
        sourceRoot,
        entrypointPath,
        resultPath: path.join(workDir, "result.json"),
      });

      await this.appendCapturedLogs(
        run.automation_task_id,
        automationToken,
        "stdout",
        result.stdoutLines,
      );
      await this.appendCapturedLogs(
        run.automation_task_id,
        automationToken,
        "stderr",
        result.stderrLines,
      );
      if (result.error) {
        throw new Error(result.error);
      }
      await this.controlClient.appendWorkflowRunLog(
        run.id,
        automationToken,
        "system",
        "workflow entrypoint completed successfully",
      );

      await this.controlClient.transitionWorkflowRun(run.id, automationToken, {
        state: "succeeded",
        output: result.output,
        artifact_refs: [],
        message: "workflow worker completed successfully",
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await this.controlClient
        .appendWorkflowRunLog(run.id, automationToken, "system", `workflow worker failed: ${message}`)
        .catch(() => {});
      await this.safeFailRun(run.id, automationToken, message);
      throw error;
    } finally {
      await fs.rm(workDir, { recursive: true, force: true }).catch(() => {});
    }
  }

  private async safeFailRun(
    runId: string,
    automationToken: string,
    error: string,
  ): Promise<void> {
    await this.controlClient
      .transitionWorkflowRun(runId, automationToken, {
        state: "failed",
        error,
        artifact_refs: [],
        message: "workflow worker failed",
      })
      .catch(() => {});
  }

  private async appendCapturedLogs(
    taskId: string,
    automationToken: string,
    stream: GatewayAutomationTaskLogStream,
    lines: string[],
  ): Promise<void> {
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) {
        continue;
      }
      await this.controlClient.appendAutomationTaskLog(taskId, automationToken, stream, trimmed);
    }
  }

  private async executeWorkflowEntrypoint(request: {
    run: GatewayWorkflowRunResource;
    endpointUrl: string;
    authHeader: string;
    authToken: string;
    sourceRoot: string;
    entrypointPath: string;
    resultPath: string;
  }): Promise<WorkflowExecutionResult> {
    await fs.mkdir(path.dirname(request.resultPath), { recursive: true });
    const contextPath = path.join(this.resolveWorkDir(request.run.id), "context.json");
    const runnerContext: WorkflowRunnerContext = {
      endpointUrl: request.endpointUrl,
      authHeader: request.authHeader,
      authToken: request.authToken,
      entrypointPath: request.entrypointPath,
      sourceRoot: request.sourceRoot,
      input: request.run.input,
      sessionId: request.run.session_id,
      workflowRunId: request.run.id,
      automationTaskId: request.run.automation_task_id,
      resultPath: request.resultPath,
    };
    await fs.writeFile(contextPath, `${JSON.stringify(runnerContext, null, 2)}\n`, "utf8");

    const runnerPath = fileURLToPath(new URL("./entrypoint-runner.ts", import.meta.url));
    const child = spawn(
      process.execPath,
      ["--import", "tsx/esm", runnerPath, contextPath],
      {
        stdio: ["ignore", "pipe", "pipe"],
      },
    );

    const [stdout, stderr, exitCode] = await Promise.all([
      readStream(child.stdout),
      readStream(child.stderr),
      new Promise<number | null>((resolve, reject) => {
        child.once("error", reject);
        child.once("close", resolve);
      }),
    ]);

    const stdoutLines = splitLogLines(stdout);
    const stderrLines = splitLogLines(stderr);

    if (exitCode !== 0) {
      return {
        output: null,
        stdoutLines,
        stderrLines,
        error:
          stderrLines[stderrLines.length - 1] ??
          stdoutLines[stdoutLines.length - 1] ??
          `workflow entrypoint exited with code ${exitCode}`,
      };
    }

    try {
      const resultPayload = JSON.parse(await fs.readFile(request.resultPath, "utf8")) as {
        output: unknown;
      };
      return {
        output: resultPayload.output ?? null,
        stdoutLines,
        stderrLines,
        error: null,
      };
    } catch (error) {
      return {
        output: null,
        stdoutLines,
        stderrLines,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  private resolveWorkDir(runId: string): string {
    return path.join(this.workRoot, runId);
  }
}

async function readStream(stream: NodeJS.ReadableStream | null): Promise<string> {
  if (!stream) {
    return "";
  }
  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function splitLogLines(value: string): string[] {
  return value
    .split(/\r?\n/u)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0);
}
