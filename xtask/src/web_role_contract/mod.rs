//! `xtask {generate,check}-web-role-contracts`.
//!
//! The web surface and Playwright BDD harness import a generated role-wire
//! contract module so TypeScript request/response/error shapes stay aligned
//! with the Rust contract and `OpenAPI` source of truth.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use syn::{Fields, GenericArgument, Item, LitStr, PathArguments, Type};
use tanren_contract::{
    ROLE_FAILURE_EXTENSION_REASON, ROLE_SERVER_FAILURE_REASONS,
    SHARED_INTERFACE_ROLE_FAILURE_REASONS,
};

const TARGET: &str = "apps/web/src/app/lib/generated/role-contract.ts";
const TEMPLATE: &str = include_str!("role-contract.ts.template");
const CONTRACT_ROLE_SOURCE: &str = "crates/tanren-contract/src/role.rs";
const IDENTITY_ROLE_SOURCE: &str = "crates/tanren-identity-policy/src/role.rs";
const API_OPENAPI_SOURCE: &str = "crates/tanren-api-app/src/routes.rs";
const API_ROLE_ROUTES_SOURCE: &str = "crates/tanren-api-app/src/routes_role.rs";

#[derive(Debug, Clone)]
struct FieldDef {
    name: String,
    ty: TypeRef,
}

#[derive(Debug, Clone)]
struct StructDef {
    fields: Vec<FieldDef>,
}

#[derive(Debug, Clone)]
struct EnumVariantDef {
    wire_name: String,
    fields: Vec<FieldDef>,
}

#[derive(Debug, Clone)]
struct TaggedEnumDef {
    tag: String,
    variants: Vec<EnumVariantDef>,
}

#[derive(Debug, Clone)]
enum TypeRef {
    Simple(String),
    Vec(Box<TypeRef>),
    Option(Box<TypeRef>),
}

#[derive(Debug, Default)]
struct ContractShape {
    structs: BTreeMap<String, StructDef>,
    tagged_enums: BTreeMap<String, TaggedEnumDef>,
}

pub(crate) fn generate(root: &Path) -> Result<()> {
    let target = root.join(TARGET);
    let parent = target
        .parent()
        .context("role-contract target must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create role-contract dir {}", parent.display()))?;

    let rendered = render(root)?;
    fs::write(&target, rendered).with_context(|| format!("write {}", target.display()))?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(
        handle,
        "generate-web-role-contracts: wrote {}",
        target.strip_prefix(root).unwrap_or(&target).display()
    );
    Ok(())
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let target = root.join(TARGET);
    let current =
        fs::read_to_string(&target).with_context(|| format!("read {}", target.display()))?;
    let expected = render(root)?;

    if current == expected {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-web-role-contracts: 0 violations (generated role contract is current)"
        );
        return Ok(());
    }

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(
        handle,
        "{}: generated role contract drifted; run `cargo run -q -p tanren-xtask -- generate-web-role-contracts`",
        target.strip_prefix(root).unwrap_or(&target).display()
    );
    bail!("check-web-role-contracts: 1 violation(s)")
}

fn render(root: &Path) -> Result<String> {
    validate_failure_taxonomy_contract()?;
    let shape = load_contract_shape(root)?;
    let replacements = build_replacements(&shape)?;

    let mut rendered = TEMPLATE.to_owned();
    for (token, value) in replacements {
        rendered = replace_token(&rendered, token, &value)?;
    }

    let unresolved = Regex::new(r"__[A-Z0-9_]+__").context("compile token regex")?;
    if unresolved.is_match(&rendered) {
        bail!("role-contract template still contains unresolved token(s)");
    }

    format_typescript_with_prettier(root, &rendered)
}

fn build_replacements(shape: &ContractShape) -> Result<BTreeMap<&'static str, String>> {
    let mut replacements = BTreeMap::<&'static str, String>::new();
    replacements.insert("__ROLE_CONTRACT_HEADER__", render_contract_header());
    insert_struct_field_tokens(shape, &mut replacements)?;
    insert_struct_interface_tokens(shape, &mut replacements)?;
    insert_failure_and_support_tokens(shape, &mut replacements)?;
    insert_parser_tokens(shape, &mut replacements)?;
    Ok(replacements)
}

fn insert_struct_field_tokens(
    shape: &ContractShape,
    replacements: &mut BTreeMap<&'static str, String>,
) -> Result<()> {
    for (token, struct_name, field_name) in [
        (
            "__APPLY_ROLE_PRINCIPAL_FIELD__",
            "ApplyRoleRequest",
            "principal",
        ),
        (
            "__ROLE_READ_MODEL_REQUEST_GRANT_PRINCIPAL_FIELD__",
            "RoleReadModelRequest",
            "grant_principal",
        ),
        (
            "__ROLE_READ_MODEL_RESPONSE_GRANT_PRINCIPAL_FIELD__",
            "RoleReadModelResponse",
            "grant_principal",
        ),
        (
            "__PERMISSION_CHECK_REQUEST_PRINCIPAL_FIELD__",
            "PermissionCheckRequest",
            "principal",
        ),
        (
            "__PERMISSION_CHECK_RESPONSE_PRINCIPAL_FIELD__",
            "PermissionCheckResponse",
            "principal",
        ),
    ] {
        replacements.insert(
            token,
            render_struct_field_line(shape, struct_name, field_name, 2)?,
        );
    }
    Ok(())
}

fn insert_struct_interface_tokens(
    shape: &ContractShape,
    replacements: &mut BTreeMap<&'static str, String>,
) -> Result<()> {
    for (token, struct_name) in [
        (
            "__ROLE_READ_MODEL_FRESHNESS_INTERFACE__",
            "RoleReadModelFreshness",
        ),
        (
            "__ROLE_TEMPLATE_CURSOR_VIEW_INTERFACE__",
            "RoleTemplateCursorView",
        ),
        (
            "__PERMISSION_GRANT_CURSOR_VIEW_INTERFACE__",
            "PermissionGrantCursorView",
        ),
        ("__PERMISSION_GRANT_VIEW_INTERFACE__", "PermissionGrantView"),
    ] {
        replacements.insert(token, render_struct_interface(shape, struct_name)?);
    }
    Ok(())
}

fn insert_failure_and_support_tokens(
    shape: &ContractShape,
    replacements: &mut BTreeMap<&'static str, String>,
) -> Result<()> {
    replacements.insert(
        "__PERMISSION_GRANT_SUPPORT_TYPES__",
        format!(
            "{}\n\n{}",
            render_tagged_enum_type(shape, "PermissionGrantSource")?,
            render_struct_interface(shape, "PermissionGrantRevocation")?
        ),
    );
    replacements.insert(
        "__ROLE_SERVER_FAILURE_CODE_UNION__",
        render_role_failure_union(),
    );
    replacements.insert(
        "__ROLE_FAILURE_SUMMARIES_OBJECT__",
        render_role_failure_summaries(),
    );
    replacements.insert(
        "__ROLE_FAILURE_SUMMARY_ANY_FUNCTION__",
        render_role_failure_summary_any_function(),
    );
    Ok(())
}

fn insert_parser_tokens(
    shape: &ContractShape,
    replacements: &mut BTreeMap<&'static str, String>,
) -> Result<()> {
    for (token, struct_name, field_name, context_prefix) in [
        (
            "__PARSE_PERMISSION_CHECK_RESPONSE_PRINCIPAL__",
            "PermissionCheckResponse",
            "principal",
            "permission check",
        ),
        (
            "__PARSE_ROLE_READ_MODEL_RESPONSE_GRANT_PRINCIPAL__",
            "RoleReadModelResponse",
            "grant_principal",
            "role read-model",
        ),
    ] {
        replacements.insert(
            token,
            render_struct_field_parser_line(shape, struct_name, field_name, context_prefix, 4)?,
        );
    }

    replacements.insert(
        "__PERMISSION_GRANT_SOURCE_PARSER_FUNCTION__",
        render_tagged_enum_parser(shape, "PermissionGrantSource")?,
    );
    replacements.insert(
        "__PERMISSION_GRANT_REVOCATION_PARSER_FUNCTION__",
        render_struct_parser(
            shape,
            "parsePermissionGrantRevocation",
            "PermissionGrantRevocation",
        )?,
    );
    replacements.insert(
        "__PERMISSION_GRANT_VIEW_PARSER_FUNCTION__",
        render_struct_parser(shape, "parsePermissionGrantView", "PermissionGrantView")?,
    );
    replacements.insert(
        "__ROLE_TEMPLATE_CURSOR_VIEW_PARSER_FUNCTION__",
        render_struct_parser(
            shape,
            "parseRoleTemplateCursorView",
            "RoleTemplateCursorView",
        )?,
    );
    replacements.insert(
        "__PERMISSION_GRANT_CURSOR_VIEW_PARSER_FUNCTION__",
        render_struct_parser(
            shape,
            "parsePermissionGrantCursorView",
            "PermissionGrantCursorView",
        )?,
    );
    replacements.insert(
        "__ROLE_READ_MODEL_FRESHNESS_PARSER_FUNCTION__",
        render_struct_parser(
            shape,
            "parseRoleReadModelFreshness",
            "RoleReadModelFreshness",
        )?,
    );
    Ok(())
}

fn render_contract_header() -> String {
    format!(
        "// Generated by `cargo run -q -p tanren-xtask -- generate-web-role-contracts`.\n// Sources: {CONTRACT_ROLE_SOURCE}, {IDENTITY_ROLE_SOURCE}, {API_OPENAPI_SOURCE}, {API_ROLE_ROUTES_SOURCE}.\n// Do not edit directly."
    )
}

fn format_typescript_with_prettier(root: &Path, input: &str) -> Result<String> {
    let mut child = Command::new("pnpm")
        .arg("exec")
        .arg("prettier")
        .arg("--parser")
        .arg("typescript")
        .arg("--stdin-filepath")
        .arg("src/app/lib/generated/role-contract.ts")
        .current_dir(root.join("apps/web"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn prettier for role-contract generator")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .context("write generated role-contract source to prettier stdin")?;
    }
    let output = child
        .wait_with_output()
        .context("wait for prettier formatting output")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("prettier formatting failed for role-contract generator: {stderr}");
    }

    String::from_utf8(output.stdout)
        .context("decode prettier stdout for generated role-contract source")
}

fn validate_failure_taxonomy_contract() -> Result<()> {
    let mut shared = SHARED_INTERFACE_ROLE_FAILURE_REASONS
        .iter()
        .map(|reason| reason.code())
        .collect::<Vec<_>>();
    shared.sort_unstable();

    let mut expected = ROLE_SERVER_FAILURE_REASONS
        .iter()
        .map(|reason| reason.code())
        .collect::<Vec<_>>();
    expected.sort_unstable();

    let mut from_shared_plus_extension = shared;
    from_shared_plus_extension.push(ROLE_FAILURE_EXTENSION_REASON.code());
    from_shared_plus_extension.sort_unstable();

    if expected != from_shared_plus_extension {
        bail!("role failure taxonomy must equal shared taxonomy + role extension");
    }
    Ok(())
}

fn load_contract_shape(root: &Path) -> Result<ContractShape> {
    let mut shape = ContractShape::default();

    for source in [CONTRACT_ROLE_SOURCE, IDENTITY_ROLE_SOURCE] {
        let path = root.join(source);
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let file = syn::parse_file(&raw).with_context(|| format!("parse {}", path.display()))?;

        for item in file.items {
            match item {
                Item::Struct(item_struct) => {
                    if let Some(def) = parse_struct_item(&item_struct)? {
                        shape.structs.insert(item_struct.ident.to_string(), def);
                    }
                }
                Item::Enum(item_enum) => {
                    if let Some(def) = parse_tagged_enum_item(&item_enum)? {
                        shape.tagged_enums.insert(item_enum.ident.to_string(), def);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(shape)
}

fn parse_struct_item(item: &syn::ItemStruct) -> Result<Option<StructDef>> {
    let Fields::Named(named) = &item.fields else {
        return Ok(None);
    };

    let mut fields = Vec::with_capacity(named.named.len());
    for field in &named.named {
        let name = field
            .ident
            .as_ref()
            .map(ToString::to_string)
            .context("struct field missing name")?;
        fields.push(FieldDef {
            name,
            ty: parse_type_ref(&field.ty)?,
        });
    }

    Ok(Some(StructDef { fields }))
}

fn parse_tagged_enum_item(item: &syn::ItemEnum) -> Result<Option<TaggedEnumDef>> {
    let (tag, rename_all_snake_case) = parse_serde_enum_metadata(&item.attrs)?;
    let Some(tag) = tag else {
        return Ok(None);
    };

    let mut variants = Vec::with_capacity(item.variants.len());
    for variant in &item.variants {
        let override_name = parse_serde_variant_rename(&variant.attrs)?;
        let wire_name = override_name.unwrap_or_else(|| {
            if rename_all_snake_case {
                to_snake_case(&variant.ident.to_string())
            } else {
                variant.ident.to_string()
            }
        });

        let fields = match &variant.fields {
            Fields::Unit => Vec::new(),
            Fields::Named(named) => {
                let mut out = Vec::with_capacity(named.named.len());
                for field in &named.named {
                    let name = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .context("enum variant field missing name")?;
                    out.push(FieldDef {
                        name,
                        ty: parse_type_ref(&field.ty)?,
                    });
                }
                out
            }
            Fields::Unnamed(_) => {
                bail!(
                    "unsupported unnamed variant fields in tagged enum {}::{}",
                    item.ident,
                    variant.ident
                )
            }
        };

        variants.push(EnumVariantDef { wire_name, fields });
    }

    Ok(Some(TaggedEnumDef { tag, variants }))
}

fn parse_serde_enum_metadata(attrs: &[syn::Attribute]) -> Result<(Option<String>, bool)> {
    let mut tag: Option<String> = None;
    let mut rename_all_snake_case = false;

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                tag = Some(lit.value());
            }
            if meta.path.is_ident("rename_all") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                rename_all_snake_case = lit.value() == "snake_case";
            }
            Ok(())
        })?;
    }

    Ok((tag, rename_all_snake_case))
}

fn parse_serde_variant_rename(attrs: &[syn::Attribute]) -> Result<Option<String>> {
    let mut rename: Option<String> = None;

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                rename = Some(lit.value());
            }
            Ok(())
        })?;
    }

    Ok(rename)
}

fn parse_type_ref(ty: &Type) -> Result<TypeRef> {
    let Type::Path(path) = ty else {
        bail!("unsupported rust type in role web-contract generator")
    };

    let segment = path
        .path
        .segments
        .last()
        .context("missing path segment for type")?;
    let ident = segment.ident.to_string();

    match ident.as_str() {
        "Vec" => {
            let inner = first_generic_type(segment)?;
            Ok(TypeRef::Vec(Box::new(parse_type_ref(inner)?)))
        }
        "Option" => {
            let inner = first_generic_type(segment)?;
            Ok(TypeRef::Option(Box::new(parse_type_ref(inner)?)))
        }
        _ => Ok(TypeRef::Simple(ident)),
    }
}

fn first_generic_type(segment: &syn::PathSegment) -> Result<&Type> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        bail!("expected generic type for {}", segment.ident)
    };

    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        bail!("missing inner generic type for {}", segment.ident)
    };

    Ok(inner)
}

fn render_struct_field_line(
    shape: &ContractShape,
    struct_name: &str,
    field_name: &str,
    indent: usize,
) -> Result<String> {
    let struct_def = struct_def(shape, struct_name)?;
    let field = find_field(struct_def, struct_name, field_name)?;
    Ok(format!(
        "{}{}: {};",
        " ".repeat(indent),
        field.name,
        render_ts_type(struct_name, &field.name, &field.ty)?
    ))
}

fn render_struct_interface(shape: &ContractShape, struct_name: &str) -> Result<String> {
    let struct_def = struct_def(shape, struct_name)?;
    let mut out = String::new();
    let _ = writeln!(out, "export interface {struct_name} {{");
    for field in &struct_def.fields {
        let _ = writeln!(
            out,
            "  {}: {};",
            field.name,
            render_ts_type(struct_name, &field.name, &field.ty)?
        );
    }
    out.push('}');
    Ok(out)
}

fn render_struct_parser(shape: &ContractShape, fn_name: &str, struct_name: &str) -> Result<String> {
    let struct_def = struct_def(shape, struct_name)?;
    let mut out = String::new();
    let _ = write!(
        out,
        "function {fn_name}(\n  value: unknown,\n  context: string,\n): {struct_name} {{\n"
    );
    out.push_str("  const data = expectRecord(value, context);\n");
    out.push_str("  return {\n");
    for field in &struct_def.fields {
        out.push_str("    ");
        out.push_str(&render_field_parser_line(
            struct_name,
            field,
            "data",
            "context",
        )?);
        out.push('\n');
    }
    out.push_str("  };\n");
    out.push('}');
    Ok(out)
}

fn render_struct_field_parser_line(
    shape: &ContractShape,
    struct_name: &str,
    field_name: &str,
    context_prefix: &str,
    indent: usize,
) -> Result<String> {
    let struct_def = struct_def(shape, struct_name)?;
    let field = find_field(struct_def, struct_name, field_name)?;
    let value_expr = format!("data[\"{}\"]", field.name);
    let context_expr = format!("\"{context_prefix} {}\"", field.name);
    let parser = render_parser_expr(
        struct_name,
        &field.name,
        &field.ty,
        &value_expr,
        &context_expr,
    )?;
    Ok(format!("{}{}: {},", " ".repeat(indent), field.name, parser))
}

fn render_field_parser_line(
    struct_name: &str,
    field: &FieldDef,
    data_ident: &str,
    context_ident: &str,
) -> Result<String> {
    let value_expr = format!("{data_ident}[\"{}\"]", field.name);
    let context_expr = format!("`${{{context_ident}}}.{}`", field.name);
    let parser = render_parser_expr(
        struct_name,
        &field.name,
        &field.ty,
        &value_expr,
        &context_expr,
    )?;
    Ok(format!("{}: {},", field.name, parser))
}

fn render_parser_expr(
    struct_name: &str,
    field_name: &str,
    ty: &TypeRef,
    value_expr: &str,
    context_expr: &str,
) -> Result<String> {
    match ty {
        TypeRef::Option(inner) => {
            let parser_name = parser_function_name(struct_name, field_name, inner)?;
            Ok(format!(
                "parseOptional({value_expr}, {context_expr}, {parser_name})"
            ))
        }
        TypeRef::Vec(inner) => {
            let parser_name = parser_function_name(struct_name, field_name, inner)?;
            Ok(format!(
                "parseArray({value_expr}, {context_expr}, {parser_name})"
            ))
        }
        TypeRef::Simple(_) => {
            let parser_name = parser_function_name(struct_name, field_name, ty)?;
            Ok(format!("{parser_name}({value_expr}, {context_expr})"))
        }
    }
}

fn parser_function_name(struct_name: &str, field_name: &str, ty: &TypeRef) -> Result<&'static str> {
    let simple = match ty {
        TypeRef::Simple(s) => s.as_str(),
        _ => bail!("parser function requires simple inner type"),
    };

    if struct_name == "RoleTemplateCursorView" && field_name == "id" {
        return Ok("parseRoleTemplateCursorIdValue");
    }
    if struct_name == "PermissionGrantCursorView" && field_name == "id" {
        return Ok("parsePermissionGrantCursorIdValue");
    }

    let parser = match simple {
        "String" | "RoleName" | "DateTime" => "parseString",
        "bool" => "parseBoolean",
        "RoleId" => "parseRoleIdValue",
        "AccountId" => "parseAccountIdValue",
        "OrgId" => "parseOrgIdValue",
        "ProjectId" => "parseProjectIdValue",
        "PermissionName" => "parsePermissionNameValue",
        "PermissionGrantId" => "parsePermissionGrantIdValue",
        "RoleScope" => "parseRoleScope",
        "PermissionScope" => "parsePermissionScope",
        "PrincipalRef" => "parsePrincipalRef",
        "PermissionGrantSource" => "parsePermissionGrantSource",
        "PermissionGrantRevocation" => "parsePermissionGrantRevocation",
        "RoleTemplateCursorView" => "parseRoleTemplateCursorView",
        "PermissionGrantCursorView" => "parsePermissionGrantCursorView",
        "RoleReadModelFreshness" => "parseRoleReadModelFreshness",
        "RoleTemplateView" => "parseRoleTemplateView",
        "PermissionGrantView" => "parsePermissionGrantView",
        other => bail!("unsupported parser target type `{other}`"),
    };

    Ok(parser)
}

fn render_ts_type(struct_name: &str, field_name: &str, ty: &TypeRef) -> Result<String> {
    match ty {
        TypeRef::Option(inner) => Ok(format!(
            "{} | null",
            render_ts_type(struct_name, field_name, inner)?
        )),
        TypeRef::Vec(inner) => Ok(format!(
            "{}[]",
            render_ts_type(struct_name, field_name, inner)?
        )),
        TypeRef::Simple(simple) => {
            if struct_name == "RoleTemplateCursorView" && field_name == "id" {
                return Ok("RoleTemplateCursorId".to_owned());
            }
            if struct_name == "PermissionGrantCursorView" && field_name == "id" {
                return Ok("PermissionGrantCursorId".to_owned());
            }
            let rendered = match simple.as_str() {
                "String" | "RoleName" | "DateTime" => "string",
                "u64" => "number",
                "bool" => "boolean",
                other => other,
            };
            Ok(rendered.to_owned())
        }
    }
}

fn render_tagged_enum_type(shape: &ContractShape, enum_name: &str) -> Result<String> {
    let def = tagged_enum_def(shape, enum_name)?;
    let mut out = String::new();
    let _ = writeln!(out, "export type {enum_name} =");
    for variant in &def.variants {
        let mut fields = vec![format!("{}: \"{}\"", def.tag, variant.wire_name)];
        for field in &variant.fields {
            fields.push(format!(
                "{}: {}",
                field.name,
                render_ts_type(enum_name, &field.name, &field.ty)?
            ));
        }
        let _ = writeln!(out, "  | {{ {} }}", fields.join("; "));
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out.push(';');
    Ok(out)
}

fn render_tagged_enum_parser(shape: &ContractShape, enum_name: &str) -> Result<String> {
    let def = tagged_enum_def(shape, enum_name)?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "function parse{enum_name}(value: unknown, context: string): {enum_name} {{"
    );
    out.push_str("  const data = expectRecord(value, context);\n");
    let _ = writeln!(
        out,
        "  const variant = parseString(data[\"{}\"], `${{context}}.{}`);",
        def.tag, def.tag
    );
    out.push_str("  switch (variant) {\n");
    for variant in &def.variants {
        let _ = writeln!(out, "    case \"{}\":", variant.wire_name);
        if variant.fields.is_empty() {
            let _ = writeln!(out, "      return {{ {}: variant }};", def.tag);
            continue;
        }
        out.push_str("      return {\n");
        let _ = writeln!(out, "        {}: variant,", def.tag);
        for field in &variant.fields {
            let parser = render_parser_expr(
                enum_name,
                &field.name,
                &field.ty,
                &format!("data[\"{}\"]", field.name),
                &format!("`${{context}}.{}`", field.name),
            )?;
            let _ = writeln!(out, "        {}: {},", field.name, parser);
        }
        out.push_str("      };\n");
    }
    let allowed = def
        .variants
        .iter()
        .map(|variant| variant.wire_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str("    default:\n");
    let _ = writeln!(
        out,
        "      throw new Error(`${{context}}.{} must be one of {allowed}`);",
        def.tag
    );
    out.push_str("  }\n");
    out.push('}');
    Ok(out)
}

fn render_role_failure_union() -> String {
    let mut out = String::from("export type RoleServerFailureCode =\n");
    for reason in ROLE_SERVER_FAILURE_REASONS {
        let _ = writeln!(out, "  | \"{}\"", reason.code());
    }
    out.push(';');
    out
}

fn render_role_failure_summaries() -> String {
    let mut out = String::from(
        "const ROLE_FAILURE_SUMMARIES: Readonly<Record<RoleServerFailureCode, string>> =\n  {\n",
    );
    for reason in ROLE_SERVER_FAILURE_REASONS {
        let _ = writeln!(
            out,
            "    {}: \"{}\",",
            reason.code(),
            reason.summary().replace('"', "\\\"")
        );
    }
    out.push_str("  };\n");
    out
}

fn render_role_failure_summary_any_function() -> String {
    let mut out = String::from(
        "export function roleFailureSummaryForAnyCode(code: RoleFailureCode): string {\n",
    );
    out.push_str("  switch (code) {\n");
    for reason in ROLE_SERVER_FAILURE_REASONS {
        let _ = writeln!(out, "    case \"{}\":", reason.code());
    }
    out.push_str("      return roleFailureSummaryForCode(code);\n");
    out.push_str(
        "    case \"transport_error\":\n      return \"Tanren could not complete the request due to transport or payload errors.\";\n",
    );
    out.push_str("    default:\n      return assertNever(code, \"role failure code\");\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn struct_def<'a>(shape: &'a ContractShape, struct_name: &str) -> Result<&'a StructDef> {
    shape
        .structs
        .get(struct_name)
        .with_context(|| format!("missing struct metadata for {struct_name}"))
}

fn tagged_enum_def<'a>(shape: &'a ContractShape, enum_name: &str) -> Result<&'a TaggedEnumDef> {
    shape
        .tagged_enums
        .get(enum_name)
        .with_context(|| format!("missing enum metadata for {enum_name}"))
}

fn find_field<'a>(
    struct_def: &'a StructDef,
    struct_name: &str,
    field_name: &str,
) -> Result<&'a FieldDef> {
    struct_def
        .fields
        .iter()
        .find(|field| field.name == field_name)
        .with_context(|| format!("missing field {struct_name}.{field_name}"))
}

fn to_snake_case(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 4);
    for (idx, ch) in raw.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn replace_token(current: &str, token: &str, replacement: &str) -> Result<String> {
    if !current.contains(token) {
        bail!("role-contract template drift: missing token `{token}`");
    }
    Ok(current.replacen(token, replacement, 1))
}
