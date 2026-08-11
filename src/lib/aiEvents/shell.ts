import { sandboxApi } from "../api/sandbox";

/**
 * Run a command on behalf of an AI event handler, through the shell sandbox.
 *
 * These handlers used to call a Tauri command named `execute_shell`, which
 * never existed on the Rust side — so every one of them failed with "Command
 * execute_shell not found", in the desktop app and in browser mode alike.
 *
 * Re-adding `execute_shell` would have given the agent an ungated arbitrary
 * command runner, which is precisely what `shell_sandbox.rs` exists to prevent.
 * They go through the sandbox instead.
 *
 * `executeApproved` rather than `execute` is deliberate: `agentCore` only
 * auto-runs events whose category is `SAFE`, so anything reaching this function
 * is `NORMAL` or `DANGEROUS` and the user has already accepted it in the
 * approval dialog. The approved path still refuses `Banned` commands — the
 * injection characters `; \` $` and the sequences `&& || $( ${` — so consent
 * cannot be used to smuggle in command chaining.
 */
export async function runSandboxed(command: string, args: string[] = []): Promise<string> {
  // The sandbox executes by splitting the string on whitespace, with no quote
  // handling, so an argument containing a space would silently become two
  // arguments. Rejecting is better than running something other than what was
  // asked for.
  const bad = args.find((a) => /\s/.test(a));
  if (bad !== undefined) {
    throw new Error(
      `Argument contains whitespace, which the command sandbox cannot represent: "${bad}"`,
    );
  }
  if (/\s/.test(command)) {
    throw new Error(`Command name contains whitespace: "${command}"`);
  }

  const line = [command, ...args].join(" ").trim();
  if (!line) throw new Error("Empty command");

  const result = await sandboxApi.executeApproved(line);

  // Handlers return a string that becomes tool context for the model, so the
  // exit code and stderr have to survive — a non-zero exit with the output
  // dropped reads to the model as success.
  const parts: string[] = [];
  if (result.stdout.trim()) parts.push(result.stdout.trim());
  if (result.stderr.trim()) parts.push(`stderr:\n${result.stderr.trim()}`);
  if (result.exit_code !== 0) parts.push(`(exit code ${result.exit_code})`);
  return parts.join("\n\n") || `(no output, exit code ${result.exit_code})`;
}
