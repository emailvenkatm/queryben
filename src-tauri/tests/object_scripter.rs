//! Unit tests for the object-scripter pure paths.
//!
//! Live-DB paths (`script_create` for tables/views/procs, `script_alter` for
//! views, `script_insert_template`) require tiberius and are skipped here per
//! the standing "no live-DB in cargo test" convention. Everything callable
//! without a client — DROP renderer, SELECT TOP renderer, INSERT template
//! renderer, ScriptAction serde roundtrip, ObjectKind → keyword — is
//! exercised end-to-end.

use queryben_lib::core::object_script::{ObjectKind, ScriptAction, SchemaObjectRef};
use queryben_lib::adapters::object_scripter::{
    render_drop_and_create, render_insert_template, render_select_top, ObjectScripter,
    ScripterOptions, SqlServerObjectScripter,
};

fn make_ref(kind: ObjectKind, schema: &str, name: &str) -> SchemaObjectRef {
    SchemaObjectRef {
        kind,
        schema: schema.into(),
        name: name.into(),
        table: None,
    }
}

fn make_index_ref(schema: &str, table: &str, index: &str) -> SchemaObjectRef {
    SchemaObjectRef {
        kind: ObjectKind::Index,
        schema: schema.into(),
        name: index.into(),
        table: Some(table.into()),
    }
}

// ---- DROP renderer --------------------------------------------------------

#[test]
fn drop_table_with_default_options_uses_brackets_and_if_exists() {
    let scripter = SqlServerObjectScripter::new(ScripterOptions::default());
    let sql = scripter.script_drop(&make_ref(ObjectKind::Table, "dbo", "Widget"));
    assert_eq!(sql, "DROP TABLE IF EXISTS [dbo].[Widget];");
}

#[test]
fn drop_view_procedure_function_all_produce_correct_keyword() {
    let scripter = SqlServerObjectScripter::new(ScripterOptions::default());
    assert_eq!(
        scripter.script_drop(&make_ref(ObjectKind::View, "sales", "TopCustomers")),
        "DROP VIEW IF EXISTS [sales].[TopCustomers];"
    );
    assert_eq!(
        scripter.script_drop(&make_ref(ObjectKind::Procedure, "dbo", "usp_RunReport")),
        "DROP PROCEDURE IF EXISTS [dbo].[usp_RunReport];"
    );
    assert_eq!(
        scripter.script_drop(&make_ref(ObjectKind::Function, "util", "fn_Slugify")),
        "DROP FUNCTION IF EXISTS [util].[fn_Slugify];"
    );
}

#[test]
fn drop_without_if_exists_guard_emits_bare_drop() {
    let opts = ScripterOptions {
        include_drop_if_exists_guard: false,
        ..Default::default()
    };
    let scripter = SqlServerObjectScripter::new(opts);
    let sql = scripter.script_drop(&make_ref(ObjectKind::Table, "dbo", "Widget"));
    assert_eq!(sql, "DROP TABLE [dbo].[Widget];");
}

#[test]
fn drop_without_brackets_omits_them() {
    let opts = ScripterOptions {
        bracket_identifiers: false,
        ..Default::default()
    };
    let scripter = SqlServerObjectScripter::new(opts);
    let sql = scripter.script_drop(&make_ref(ObjectKind::Table, "dbo", "Widget"));
    assert_eq!(sql, "DROP TABLE IF EXISTS dbo.Widget;");
}

#[test]
fn drop_without_schema_prefix_uses_name_only() {
    let opts = ScripterOptions {
        include_schema_prefix: false,
        ..Default::default()
    };
    let scripter = SqlServerObjectScripter::new(opts);
    let sql = scripter.script_drop(&make_ref(ObjectKind::View, "sales", "TopCustomers"));
    assert_eq!(sql, "DROP VIEW IF EXISTS [TopCustomers];");
}

#[test]
fn drop_index_uses_on_clause_with_parent_table() {
    let scripter = SqlServerObjectScripter::new(ScripterOptions::default());
    let sql = scripter.script_drop(&make_index_ref("dbo", "Orders", "IX_Orders_CustomerId"));
    assert_eq!(
        sql,
        "DROP INDEX IF EXISTS [IX_Orders_CustomerId] ON [dbo].[Orders];"
    );
}

#[test]
fn drop_index_without_parent_table_uses_placeholder() {
    // Frontend supplies table=None only as a fallback; we still emit valid-ish
    // DDL with a clearly-marked placeholder so the user sees where to edit.
    let scripter = SqlServerObjectScripter::new(ScripterOptions::default());
    let mut obj = make_index_ref("dbo", "Orders", "IX_Bad");
    obj.table = None;
    let sql = scripter.script_drop(&obj);
    assert!(sql.contains("[<table>]"), "expected placeholder: {sql}");
    assert!(sql.starts_with("DROP INDEX IF EXISTS [IX_Bad]"), "prefix: {sql}");
}

// ---- SELECT TOP renderer --------------------------------------------------

#[test]
fn select_top_100_default_shape() {
    let opts = ScripterOptions::default();
    let obj = make_ref(ObjectKind::Table, "dbo", "Widget");
    let sql = render_select_top(&obj, &opts, 100);
    assert_eq!(sql, "SELECT TOP 100 * FROM [dbo].[Widget];");
}

#[test]
fn select_top_respects_no_bracket_option() {
    let opts = ScripterOptions {
        bracket_identifiers: false,
        ..Default::default()
    };
    let obj = make_ref(ObjectKind::View, "sales", "TopCustomers");
    let sql = render_select_top(&obj, &opts, 25);
    assert_eq!(sql, "SELECT TOP 25 * FROM sales.TopCustomers;");
}

// ---- INSERT template renderer ---------------------------------------------

#[test]
fn insert_template_emits_column_list_and_matching_placeholders() {
    let opts = ScripterOptions::default();
    let cols = vec!["Id".to_string(), "Name".to_string(), "CreatedAt".to_string()];
    let sql = render_insert_template("dbo", "Widget", &cols, &opts);
    assert_eq!(
        sql,
        "INSERT INTO [dbo].[Widget] ([Id], [Name], [CreatedAt])\nVALUES (NULL, NULL, NULL);"
    );
}

#[test]
fn insert_template_placeholder_count_matches_column_count() {
    let opts = ScripterOptions::default();
    let cols: Vec<String> = (0..5).map(|i| format!("Col{i}")).collect();
    let sql = render_insert_template("dbo", "Wide", &cols, &opts);
    // NULL, NULL, NULL, NULL, NULL — five placeholders, four separators.
    assert_eq!(sql.matches("NULL").count(), 5);
    assert_eq!(sql.matches(", ").count(), 4 + 4); // separators in cols + placeholders
}

#[test]
fn insert_template_custom_placeholder() {
    let opts = ScripterOptions {
        insert_template_placeholder: "?".into(),
        ..Default::default()
    };
    let cols = vec!["Id".to_string(), "Name".to_string()];
    let sql = render_insert_template("dbo", "Widget", &cols, &opts);
    assert!(sql.contains("VALUES (?, ?);"), "want ?-placeholders: {sql}");
}

#[test]
fn insert_template_without_brackets_or_schema() {
    let opts = ScripterOptions {
        bracket_identifiers: false,
        include_schema_prefix: false,
        ..Default::default()
    };
    let cols = vec!["Id".to_string(), "Name".to_string()];
    let sql = render_insert_template("dbo", "Widget", &cols, &opts);
    assert_eq!(sql, "INSERT INTO Widget (Id, Name)\nVALUES (NULL, NULL);");
}

// ---- DROP AND CREATE composition ------------------------------------------

#[test]
fn drop_and_create_composes_with_blank_line_between() {
    let drop = "DROP PROCEDURE IF EXISTS [dbo].[usp_Foo];";
    let create = "CREATE PROCEDURE [dbo].[usp_Foo] AS BEGIN SELECT 1 END;";
    let sql = render_drop_and_create(drop, create);
    assert_eq!(
        sql,
        "DROP PROCEDURE IF EXISTS [dbo].[usp_Foo];\n\nCREATE PROCEDURE [dbo].[usp_Foo] AS BEGIN SELECT 1 END;"
    );
}

// ---- ScriptAction / ObjectKind serde roundtrip ----------------------------

#[test]
fn script_action_serde_roundtrip_all_variants() {
    // Frontend emits `"create"` / `"selectTop"` — the camelCase enum spec has
    // to keep round-tripping or the IPC layer silently rejects clicks.
    let cases = [
        (ScriptAction::Create, "\"create\""),
        (ScriptAction::Alter, "\"alter\""),
        (ScriptAction::Drop, "\"drop\""),
        (ScriptAction::DropAndCreate, "\"dropAndCreate\""),
        (ScriptAction::SelectTop, "\"selectTop\""),
        (ScriptAction::InsertTemplate, "\"insertTemplate\""),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "wire form for {variant:?}");
        let back: ScriptAction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

#[test]
fn object_kind_serde_roundtrip_all_variants() {
    let cases = [
        (ObjectKind::Table, "\"table\""),
        (ObjectKind::View, "\"view\""),
        (ObjectKind::Procedure, "\"procedure\""),
        (ObjectKind::Function, "\"function\""),
        (ObjectKind::Index, "\"index\""),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "wire form for {variant:?}");
        let back: ObjectKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

#[test]
fn object_kind_ddl_keywords() {
    assert_eq!(ObjectKind::Table.ddl_keyword(), "TABLE");
    assert_eq!(ObjectKind::View.ddl_keyword(), "VIEW");
    assert_eq!(ObjectKind::Procedure.ddl_keyword(), "PROCEDURE");
    assert_eq!(ObjectKind::Function.ddl_keyword(), "FUNCTION");
    assert_eq!(ObjectKind::Index.ddl_keyword(), "INDEX");
}

// ---- Options: default shape mirrors task doc ------------------------------

#[test]
fn default_options_match_task_spec() {
    let d = ScripterOptions::default();
    assert!(d.bracket_identifiers);
    assert!(d.include_schema_prefix);
    assert!(d.include_drop_if_exists_guard);
    assert_eq!(d.insert_template_placeholder, "NULL");
}

// ---- Options JSON parse ---------------------------------------------------

#[test]
fn options_parse_from_task_spec_json() {
    // Exact JSON blob from the task doc must parse cleanly.
    let json = r#"{
        "bracketIdentifiers": true,
        "includeSchemaPrefix": true,
        "includeDropIfExistsGuard": true,
        "insertTemplatePlaceholder": "NULL"
    }"#;
    let opts: ScripterOptions = serde_json::from_str(json).expect("parse task-spec json");
    assert!(opts.bracket_identifiers);
    assert!(opts.include_schema_prefix);
    assert!(opts.include_drop_if_exists_guard);
    assert_eq!(opts.insert_template_placeholder, "NULL");
}

#[test]
fn options_parse_with_missing_fields_falls_back_to_defaults() {
    // Partial config file — every field individually defaulted so users can
    // upgrade the app without their existing config breaking.
    let opts: ScripterOptions = serde_json::from_str("{}").expect("empty json ok");
    assert!(opts.bracket_identifiers);
    assert_eq!(opts.insert_template_placeholder, "NULL");
}
