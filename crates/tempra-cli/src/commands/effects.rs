use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tempra_driver::{
    capability_name, ContractSource, EffectContractOrigin, EffectDiagnostic,
    EffectDirectOriginKind, EffectFunctionIdentity, EffectInspection, EffectQueryError,
    EffectWitness, FunctionCapabilityContract, FunctionEffectInfo,
};
use tempra_span::Span;

pub fn effects_command(path: PathBuf, json_output: bool) -> Result<(), String> {
    match tempra_driver::inspect_effects_file(&path) {
        Ok(inspection) => {
            if json_output {
                print_json(json!({
                    "diagnostics": [],
                    "functions": inspection.functions.iter().map(function_json).collect::<Vec<_>>(),
                }))?;
            } else {
                print!("{}", render_inspection(&inspection));
            }
            Ok(())
        }
        Err(EffectQueryError::Diagnostics(diagnostics)) => {
            if json_output {
                print_json(json!({
                    "diagnostics": diagnostics.iter().map(effect_diagnostic_json).collect::<Vec<_>>(),
                    "functions": [],
                }))?;
            } else {
                eprint!("{}", render_effect_diagnostics(&diagnostics));
            }
            Err(String::new())
        }
        Err(error) => {
            if json_output {
                print_json(json!({
                    "diagnostics": [compiler_error_json(&error)],
                    "functions": [],
                }))?;
                Err(String::new())
            } else {
                Err(error.to_string())
            }
        }
    }
}

fn print_json(value: Value) -> Result<(), String> {
    let document = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    println!("{document}");
    Ok(())
}

fn capability_set_json(set: tempra_effects::CapabilitySet) -> Value {
    Value::Array(
        set.iter()
            .map(|capability| Value::String(capability_name(capability).into()))
            .collect(),
    )
}

fn span_json(span: Span) -> Value {
    json!({ "start": span.start, "end": span.end })
}

fn optional_span_json(span: Option<Span>) -> Value {
    span.map(span_json).unwrap_or(Value::Null)
}

fn source_json(source: &ContractSource) -> Value {
    match source {
        ContractSource::InMemory => json!({ "kind": "in_memory" }),
        ContractSource::Module { name, path } => json!({
            "kind": "module",
            "module": name,
            "path": path.to_string_lossy(),
        }),
    }
}

fn identity_json(identity: &EffectFunctionIdentity) -> Value {
    json!({
        "id": identity.id.0,
        "name": identity.display_name,
    })
}

fn contract_json(contract: &FunctionCapabilityContract) -> Value {
    json!({
        "effective_denied": capability_set_json(contract.denied),
        "explicit_denied": capability_set_json(contract.explicit_denied),
        "pure": contract.pure_span.is_some(),
        "source": source_json(&contract.source),
        "declaration_span": span_json(contract.declaration_span),
        "deny_attribute_spans": contract.attribute_spans.iter().copied().map(span_json).collect::<Vec<_>>(),
        "pure_span": optional_span_json(contract.pure_span),
    })
}

fn contract_origin_name(origin: EffectContractOrigin) -> &'static str {
    match origin {
        EffectContractOrigin::Deny => "deny",
        EffectContractOrigin::Pure => "pure",
    }
}

fn origin_kind_name(kind: EffectDirectOriginKind) -> &'static str {
    match kind {
        EffectDirectOriginKind::StringConstant => "string_constant",
        EffectDirectOriginKind::Make => "make",
        EffectDirectOriginKind::MakeStruct => "make_struct",
        EffectDirectOriginKind::Division => "division",
        EffectDirectOriginKind::IntegerAddition => "integer_addition",
        EffectDirectOriginKind::IntegerSubtraction => "integer_subtraction",
        EffectDirectOriginKind::IntegerMultiplication => "integer_multiplication",
        EffectDirectOriginKind::Unwrap => "unwrap",
        EffectDirectOriginKind::ConstantIndexGet => "constant_index_get",
        EffectDirectOriginKind::DynamicIndexGet => "dynamic_index_get",
        EffectDirectOriginKind::Slice => "slice",
        EffectDirectOriginKind::Bytes => "bytes",
        EffectDirectOriginKind::ArrayPush => "array_push",
        EffectDirectOriginKind::ExternCall => "extern_call",
    }
}

fn location_json(location: tempra_effects::TirLocation) -> Value {
    json!({
        "function_id": location.function.0,
        "block_id": location.block.0,
        "instruction_index": location.instruction_index,
    })
}

fn witness_json(witness: &EffectWitness) -> Value {
    json!({
        "capability": capability_name(witness.capability),
        "calls": witness.calls.iter().map(|call| json!({
            "caller": identity_json(&call.caller),
            "callee": identity_json(&call.callee),
            "callsite": location_json(call.callsite),
        })).collect::<Vec<_>>(),
        "origin": {
            "function": identity_json(&witness.origin.function),
            "capability": capability_name(witness.origin.capability),
            "location": location_json(witness.origin.location),
            "kind": origin_kind_name(witness.origin.kind),
            "external": witness.origin.external.as_ref().map(identity_json),
        },
    })
}

fn effect_diagnostic_json(diagnostic: &EffectDiagnostic) -> Value {
    json!({
        "code": diagnostic.code,
        "severity": "error",
        "message": diagnostic.to_diagnostic().message,
        "function": identity_json(&diagnostic.function),
        "capability": capability_name(diagnostic.capability),
        "contract_origins": diagnostic.contract_origins.iter().copied().map(contract_origin_name).collect::<Vec<_>>(),
        "contract": contract_json(&diagnostic.contract),
        "witness": witness_json(&diagnostic.witness),
    })
}

fn function_json(function: &FunctionEffectInfo) -> Value {
    json!({
        "function": identity_json(&function.function),
        "direct": capability_set_json(function.direct),
        "required": capability_set_json(function.required),
        "contract": function.contract.as_ref().map(contract_json),
        "requirements": function.requirements.iter().map(|requirement| json!({
            "capability": capability_name(requirement.capability),
            "witness": witness_json(&requirement.witness),
        })).collect::<Vec<_>>(),
    })
}

fn compiler_error_json(error: &EffectQueryError) -> Value {
    let code = match error {
        EffectQueryError::Projection(_) => Some("effect-projection-error"),
        EffectQueryError::Compiler(_) | EffectQueryError::Diagnostics(_) => None,
    };
    json!({
        "severity": "error",
        "code": code,
        "message": error.to_string(),
        "labels": [],
        "notes": [],
    })
}

fn render_inspection(inspection: &EffectInspection) -> String {
    let mut output = String::new();
    for function in &inspection.functions {
        output.push_str(&format!(
            "function #{} {}\n",
            function.function.id.0, function.function.display_name
        ));
        output.push_str(&format!("  direct:   {}\n", render_set(function.direct)));
        output.push_str(&format!("  required: {}\n", render_set(function.required)));
        output.push_str(&format!(
            "  contract: {}\n",
            function
                .contract
                .as_ref()
                .map(render_contract)
                .unwrap_or_else(|| "none".into())
        ));
    }
    output
}

fn render_set(set: tempra_effects::CapabilitySet) -> String {
    let names = set.iter().map(capability_name).collect::<Vec<_>>();
    format!("[{}]", names.join(", "))
}

fn render_contract(contract: &FunctionCapabilityContract) -> String {
    let explicit = render_set(contract.explicit_denied);
    match (
        contract.pure_span.is_some(),
        contract.explicit_denied.iter().next().is_some(),
    ) {
        (true, true) => format!(
            "@pure + @deny{explicit}; effective {}",
            render_set(contract.denied)
        ),
        (true, false) => format!("@pure; effective {}", render_set(contract.denied)),
        (false, true) => format!("@deny{explicit}"),
        (false, false) => "none".into(),
    }
}

fn render_effect_diagnostics(diagnostics: &[EffectDiagnostic]) -> String {
    let mut output = String::new();
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index != 0 {
            output.push('\n');
        }
        let generic = diagnostic.to_diagnostic();
        output.push_str(&format!(
            "error[{}]: {}\n",
            diagnostic.code, generic.message
        ));
        let source_path = match &diagnostic.contract.source {
            ContractSource::InMemory => None,
            ContractSource::Module { path, .. } => Some(path.as_path()),
        };
        render_source_location(&mut output, source_path, generic.span);
        output.push_str(&format!(
            "  = required through: {}\n",
            diagnostic.witness_path()
        ));
        output.push_str(&format!(
            "  = direct origin: function #{}, block #{}, instruction {}\n",
            diagnostic.witness.origin.location.function.0,
            diagnostic.witness.origin.location.block.0,
            diagnostic.witness.origin.location.instruction_index
        ));
    }
    output
}

fn render_source_location(output: &mut String, path: Option<&Path>, span: Span) {
    let display_path = path
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<in-memory>".into());
    let Some(source) = path.and_then(|path| std::fs::read_to_string(path).ok()) else {
        output.push_str(&format!(
            " --> {display_path}:bytes {}..{}\n",
            span.start, span.end
        ));
        return;
    };
    let start = usize::try_from(span.start)
        .unwrap_or(usize::MAX)
        .min(source.len());
    if !source.is_char_boundary(start) {
        output.push_str(&format!(
            " --> {display_path}:bytes {}..{}\n",
            span.start, span.end
        ));
        return;
    }
    let before = &source[..start];
    let line_number = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |index| start + index);
    let line = &source[line_start..line_end];
    let column = source[line_start..start].chars().count() + 1;
    let end = usize::try_from(span.end)
        .unwrap_or(usize::MAX)
        .min(line_end)
        .max(start);
    if !source.is_char_boundary(end) {
        output.push_str(&format!(
            " --> {display_path}:bytes {}..{}\n",
            span.start, span.end
        ));
        return;
    }
    let width = source[start..end].chars().count().max(1);
    output.push_str(&format!(" --> {display_path}:{line_number}:{column}\n"));
    output.push_str("  |\n");
    output.push_str(&format!("{line_number} | {line}\n"));
    output.push_str(&format!(
        "  | {}{} capability contract\n",
        " ".repeat(column.saturating_sub(1)),
        "^".repeat(width)
    ));
}
