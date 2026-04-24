import { promises as fs } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { materializeSourceSnapshot } from "./source-snapshot.js";
import { WorkflowControlClient } from "./workflow-control-client.js";
import type {
  GatewayAutomationTaskLogStream,
  GatewayWorkflowRunResource,
  WorkflowRunnerCredentialBinding,
  WorkflowRunnerContext,
  WorkflowRunnerWorkspaceInput,
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
      const workspaceInputs = await this.materializeWorkspaceInputs(
        run,
        automationToken,
        path.join(workDir, "workspace-inputs"),
      );
      const credentialMaterialization = await this.materializeCredentialBindings(
        run,
        automationToken,
        path.join(workDir, "credential-bindings"),
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
        credentialBindings: credentialMaterialization.bindings,
        credentialBindingFiles: credentialMaterialization.files,
        workspaceInputs,
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
    credentialBindings: WorkflowRunnerCredentialBinding[];
    credentialBindingFiles: Record<string, string>;
    workspaceInputs: WorkflowRunnerWorkspaceInput[];
    resultPath: string;
  }): Promise<WorkflowExecutionResult> {
    await fs.mkdir(path.dirname(request.resultPath), { recursive: true });
    const contextPath = path.join(this.resolveWorkDir(request.run.id), "context.json");
    const runnerContext: WorkflowRunnerContext = {
      gatewayApiUrl: this.controlClient.getGatewayApiUrl(),
      endpointUrl: request.endpointUrl,
      authHeader: request.authHeader,
      authToken: request.authToken,
      entrypointPath: request.entrypointPath,
      sourceRoot: request.sourceRoot,
      credentialBindings: request.credentialBindings,
      credentialBindingFiles: request.credentialBindingFiles,
      workspaceInputs: request.workspaceInputs,
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

  private async materializeWorkspaceInputs(
    run: GatewayWorkflowRunResource,
    automationToken: string,
    destinationRoot: string,
  ): Promise<WorkflowRunnerWorkspaceInput[]> {
    if (!run.workspace_inputs.length) {
      return [];
    }

    const resolvedRoot = path.resolve(destinationRoot);
    await fs.mkdir(resolvedRoot, { recursive: true });
    const materialized: WorkflowRunnerWorkspaceInput[] = [];
    for (const input of run.workspace_inputs) {
      const bytes = await this.controlClient.downloadWorkspaceInput(
        run.id,
        input.id,
        automationToken,
      );
      const localPath = resolveWorkspaceInputPath(resolvedRoot, input.mount_path);
      await fs.mkdir(path.dirname(localPath), { recursive: true });
      await fs.writeFile(localPath, Buffer.from(bytes));
      materialized.push({
        id: input.id,
        workspaceId: input.workspace_id,
        fileId: input.file_id,
        fileName: input.file_name,
        mediaType: input.media_type,
        byteCount: input.byte_count,
        sha256Hex: input.sha256_hex,
        provenance: input.provenance,
        mountPath: input.mount_path,
        localPath,
      });
      await this.controlClient.appendWorkflowRunLog(
        run.id,
        automationToken,
        "system",
        `materialized workflow workspace input ${input.mount_path} from workspace ${input.workspace_id}`,
      );
    }
    return materialized;
  }

  private async materializeCredentialBindings(
    run: GatewayWorkflowRunResource,
    automationToken: string,
    destinationRoot: string,
  ): Promise<{ bindings: WorkflowRunnerCredentialBinding[]; files: Record<string, string> }> {
    if (!run.credential_bindings.length) {
      return { bindings: [], files: {} };
    }

    const resolvedRoot = path.resolve(destinationRoot);
    await fs.mkdir(resolvedRoot, { recursive: true });
    const bindings: WorkflowRunnerCredentialBinding[] = [];
    const files: Record<string, string> = {};
    for (const binding of run.credential_bindings) {
      const resolved = await this.controlClient.resolveCredentialBinding(
        run.id,
        binding.id,
        automationToken,
      );
      const localPath = path.join(resolvedRoot, `${binding.id}.json`);
      await fs.writeFile(
        localPath,
        `${JSON.stringify({ payload: resolved.payload }, null, 2)}\n`,
        "utf8",
      );
      files[binding.id] = localPath;
      bindings.push({
        id: binding.id,
        name: binding.name,
        provider: binding.provider,
        namespace: binding.namespace,
        allowedOrigins: binding.allowed_origins,
        injectionMode: binding.injection_mode,
        totp: binding.totp,
      });
      await this.controlClient.appendWorkflowRunLog(
        run.id,
        automationToken,
        "system",
        `materialized workflow credential binding ${binding.id} (${binding.name})`,
      );
    }
    return { bindings, files };
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

function resolveWorkspaceInputPath(root: string, mountPath: string): string {
  const segments = mountPath
    .split("/")
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0);
  if (!segments.length) {
    throw new Error("workflow workspace input mount path must not be empty");
  }
  const localPath = path.resolve(root, ...segments);
  if (localPath !== root && !localPath.startsWith(`${root}${path.sep}`)) {
    throw new Error(`workflow workspace input path escapes work root: ${mountPath}`);
  }
  return localPath;
}
