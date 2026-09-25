use clap::Parser;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};

pub const BYPASSED_LABEL: &str = "aye:bypassed";
const SNAPSHOT_RETRIES: usize = 3;

#[derive(Debug, Parser)]
#[command(
    name = "aye bypass",
    about = "Close completed implementation with explicitly waived checks",
    long_about = "Close a task as dependency-satisfying work while recording checks that could not be performed. This is intended for completed implementation that lacks an external verification condition, such as unavailable physical hardware. Unresolved task dependencies cannot be bypassed."
)]
struct BypassArgs {
    /// Task to bypass.
    id: String,

    /// Why the missing verification is being waived.
    #[arg(long)]
    reason: String,

    /// One check that remains unverified; repeat for additional checks.
    #[arg(long = "missing", required = true)]
    missing_checks: Vec<String>,

    /// Return the normal stable JSON envelope.
    #[arg(long, hide = true)]
    json: bool,

    /// Actor identity; equivalent to the global aye option.
    #[arg(long, hide = true)]
    actor: Option<String>,
}

#[derive(Debug)]
struct Failure {
    code: String,
    message: String,
    details: Option<Value>,
    exit: i32,
}

impl Failure {
    fn new(code: impl Into<String>, message: impl Into<String>, exit: i32) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            exit,
        }
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

struct Invocation {
    command_index: usize,
    json: bool,
    actor: Option<OsString>,
}

/// Intercept only the compatibility-preserving `aye bypass` command.
/// All other invocations continue through the canonical clap CLI.
pub fn run_if_requested() -> bool {
    let raw: Vec<OsString> = std::env::args_os().collect();
    let Some(invocation) = locate_invocation(&raw) else {
        return false;
    };
    if raw[invocation.command_index] != "bypass" {
        return false;
    }

    let mut parse_args = vec![OsString::from("aye bypass")];
    parse_args.extend(raw[invocation.command_index + 1..].iter().cloned());
    let args = match BypassArgs::try_parse_from(parse_args) {
        Ok(args) => args,
        Err(error) if error.exit_code() == 0 => {
            print!("{error}");
            return true;
        }
        Err(error) => fail(
            invocation.json,
            Failure::new("INVALID_ARGUMENT", error.to_string(), 2),
        ),
    };

    let json_mode = invocation.json || args.json;
    let actor = args.actor.map(OsString::from).or(invocation.actor);
    if args.reason.trim().is_empty() {
        fail(
            json_mode,
            Failure::new("INVALID_ARGUMENT", "Bypass reason cannot be empty", 2),
        );
    }
    if args
        .missing_checks
        .iter()
        .any(|check| check.trim().is_empty())
    {
        fail(
            json_mode,
            Failure::new("INVALID_ARGUMENT", "Missing checks cannot be empty", 2),
        );
    }

    let executable = std::env::current_exe().unwrap_or_else(|error| {
        fail(
            json_mode,
            Failure::new(
                "INTERNAL_ERROR",
                format!("Cannot locate the aye executable: {error}"),
                1,
            ),
        )
    });
    let (state_oid, shown) = stable_show(&executable, actor.as_ref(), &args.id)
        .unwrap_or_else(|error| fail(json_mode, error));
    let request = build_request(
        &state_oid,
        &shown,
        args.reason.trim(),
        &args
            .missing_checks
            .iter()
            .map(|value| value.trim().to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|error| fail(json_mode, error));
    let bytes = serde_json::to_vec(&request).unwrap_or_else(|error| {
        fail(
            json_mode,
            Failure::new(
                "INTERNAL_ERROR",
                format!("Cannot encode bypass request: {error}"),
                1,
            ),
        )
    });
    let envelope = invoke(
        &executable,
        actor.as_ref(),
        &["apply", "--file", "-"],
        Some(&bytes),
    )
    .unwrap_or_else(|error| fail(json_mode, error));

    if json_mode {
        println!(
            "{}",
            serde_json::to_string(&envelope).expect("JSON envelope serializes")
        );
    } else {
        println!("Bypassed {}.", args.id);
        println!("Reason: {}", args.reason.trim());
        println!("Missing checks:");
        for check in &args.missing_checks {
            println!("- {}", check.trim());
        }
        println!("Downstream dependencies now treat the task as completed.");
    }
    true
}

fn locate_invocation(args: &[OsString]) -> Option<Invocation> {
    let mut json = false;
    let mut actor = None;
    let mut index = 1;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        match value.as_ref() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--actor" => {
                actor = args.get(index + 1).cloned();
                index += 2;
            }
            "-h" | "--help" => return None,
            _ if value.starts_with("--actor=") => {
                actor = Some(OsString::from(&value[8..]));
                index += 1;
            }
            _ if value.starts_with('-') => return None,
            _ => {
                return Some(Invocation {
                    command_index: index,
                    json,
                    actor,
                });
            }
        }
    }
    None
}

fn stable_show(
    executable: &std::path::Path,
    actor: Option<&OsString>,
    id: &str,
) -> Result<(String, Value), Failure> {
    for _ in 0..SNAPSHOT_RETRIES {
        let before = invoke_data(executable, actor, &["status"], None)?;
        let shown = invoke_data(executable, actor, &["show", id], None)?;
        let after = invoke_data(executable, actor, &["status"], None)?;
        let before_oid = state_oid(&before)?;
        let after_oid = state_oid(&after)?;
        if before_oid == after_oid {
            return Ok((before_oid.to_string(), shown));
        }
    }
    Err(Failure::new(
        "STALE_STATE",
        "Task state changed repeatedly while preparing the bypass; retry the command",
        3,
    ))
}

fn state_oid(data: &Value) -> Result<&str, Failure> {
    data.get("state_oid")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "aye status omitted state_oid", 1))
}

fn invoke_data(
    executable: &std::path::Path,
    actor: Option<&OsString>,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Value, Failure> {
    let envelope = invoke(executable, actor, args, input)?;
    envelope
        .get("data")
        .cloned()
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "aye omitted reply data", 1))
}

fn invoke(
    executable: &std::path::Path,
    actor: Option<&OsString>,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Value, Failure> {
    let mut command = Command::new(executable);
    command.arg("--json");
    if let Some(actor) = actor {
        command.arg("--actor").arg(actor);
    }
    command.args(args).stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        Failure::new(
            "INTERNAL_ERROR",
            format!("Cannot start aye subprocess: {error}"),
            1,
        )
    })?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input)
            .map_err(|error| {
                Failure::new(
                    "INTERNAL_ERROR",
                    format!("Cannot write bypass request: {error}"),
                    1,
                )
            })?;
    }
    let output = child.wait_with_output().map_err(|error| {
        Failure::new(
            "INTERNAL_ERROR",
            format!("Cannot wait for aye subprocess: {error}"),
            1,
        )
    })?;
    let envelope: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        Failure::new(
            "INTERNAL_ERROR",
            format!(
                "aye subprocess returned invalid JSON: {error}; stderr: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            1,
        )
    })?;
    if output.status.success() && envelope.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(envelope);
    }
    let error = envelope.get("error").unwrap_or(&Value::Null);
    let mut failure = Failure::new(
        error
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("INTERNAL_ERROR"),
        error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("aye subprocess failed"),
        output.status.code().unwrap_or(1),
    );
    failure.details = error.get("details").cloned();
    Err(failure)
}

fn build_request(
    state_oid: &str,
    shown: &Value,
    reason: &str,
    missing_checks: &[String],
) -> Result<Value, Failure> {
    let task = shown
        .get("task")
        .and_then(Value::as_object)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "aye show omitted task data", 1))?;
    let computed = shown
        .get("computed")
        .and_then(Value::as_object)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "aye show omitted computed data", 1))?;
    let id = task
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "Task omitted id", 1))?;
    let status = task
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "Task omitted status", 1))?;
    if status == "closed" {
        return Err(Failure::new(
            "INVALID_STATE_TRANSITION",
            "A closed task cannot be bypassed",
            3,
        ));
    }
    let blocked_by = computed
        .get("blocked_by")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "aye show omitted blocked_by", 1))?;
    if !blocked_by.is_empty() {
        return Err(Failure::new(
            "BYPASS_UNRESOLVED_DEPENDENCIES",
            "Complete or explicitly restructure unresolved task dependencies before bypassing",
            3,
        )
        .with_details(json!({"task_id": id, "blocked_by": blocked_by})));
    }

    let mut labels = task
        .get("labels")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::new("INTERNAL_ERROR", "Task omitted labels", 1))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| Failure::new("INTERNAL_ERROR", "Task label is not text", 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !labels.iter().any(|label| label == BYPASSED_LABEL) {
        labels.push(BYPASSED_LABEL.to_string());
        labels.sort();
    }

    let mut note = format!(
        "[aye:bypass]\nReason: {reason}\nMissing checks that were not verified:"
    );
    for check in missing_checks {
        note.push_str("\n- ");
        note.push_str(check);
    }
    note.push_str(
        "\nThis authorization satisfies downstream dependencies without claiming those checks passed.",
    );

    let mut operations = Vec::new();
    if status == "deferred" {
        operations.push(json!({"op":"resume","id":id}));
    }
    if task.get("manual_block").is_some_and(|value| !value.is_null()) {
        operations.push(json!({"op":"unblock","id":id}));
    }
    operations.push(json!({"op":"update","id":id,"labels":labels}));
    operations.push(json!({"op":"close","id":id,"note":note}));

    Ok(json!({
        "version": 1,
        "expected_state_oid": state_oid,
        "operations": operations
    }))
}

fn fail(json_mode: bool, failure: Failure) -> ! {
    if json_mode {
        let mut error = json!({"code":failure.code,"message":failure.message});
        if let Some(details) = failure.details {
            error["details"] = details;
        }
        println!("{}", json!({"ok":false,"error":error}));
    } else {
        eprintln!("{}: {}", failure.code, failure.message);
        if let Some(details) = failure.details {
            eprintln!("{details}");
        }
    }
    std::process::exit(failure.exit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_atomic_bypass_without_skipping_task_dependencies() {
        let shown = json!({
            "task": {
                "id": "t-00000000000000000001",
                "status": "open",
                "labels": ["feature"],
                "manual_block": {"reason":"device unavailable"}
            },
            "computed": {"blocked_by": []}
        });
        let request = build_request(
            &"a".repeat(40),
            &shown,
            "No target device",
            &["Physical-device smoke test".into()],
        )
        .unwrap();
        assert_eq!(request["expected_state_oid"], "a".repeat(40));
        assert_eq!(request["operations"][0]["op"], "unblock");
        assert_eq!(
            request["operations"][1]["labels"],
            json!(["aye:bypassed", "feature"])
        );
        assert_eq!(request["operations"][2]["op"], "close");
        assert!(
            request["operations"][2]["note"]
                .as_str()
                .unwrap()
                .contains("Physical-device smoke test")
        );
    }

    #[test]
    fn rejects_unresolved_task_dependencies() {
        let shown = json!({
            "task": {
                "id": "t-00000000000000000001",
                "status": "open",
                "labels": [],
                "manual_block": null
            },
            "computed": {"blocked_by": ["t-00000000000000000002"]}
        });
        let error = build_request(
            &"a".repeat(40),
            &shown,
            "No target device",
            &["Physical-device smoke test".into()],
        )
        .unwrap_err();
        assert_eq!(error.code, "BYPASS_UNRESOLVED_DEPENDENCIES");
    }
}
