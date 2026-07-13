//! Integration tests for `SqlServerTableDesignerProvider::generate_ddl`.
//!
//! Pure-diff coverage — no tiberius, no network. Exercises every branch the
//! designer UI can produce so a subtle refactor doesn't silently swallow an
//! ADD COLUMN. Each test asserts the exact DDL text so the frontend preview
//! and the transaction runner see the same string.

use queryben_lib::core::table_design::{
    DesignColumn, DesignForeignKey, DesignIndex, TableDesign,
};
use queryben_lib::adapters::table_designer_provider::{
    SqlServerTableDesignerProvider, TableDesignerProvider,
};

fn make_column(name: &str, sql_type: &str, nullable: bool) -> DesignColumn {
    DesignColumn {
        name: name.into(),
        sql_type: sql_type.into(),
        is_nullable: nullable,
        is_identity: false,
        is_computed: false,
        computed_expression: None,
        default_expression: None,
        ordinal: 0,
    }
}

fn make_design(columns: Vec<DesignColumn>, pk: Vec<&str>) -> TableDesign {
    TableDesign {
        schema: "dbo".into(),
        name: "Widget".into(),
        columns,
        primary_key: pk.iter().map(|s| (*s).into()).collect(),
        pk_name: if pk.is_empty() {
            None
        } else {
            Some("PK_Widget".into())
        },
        indexes: Vec::new(),
        foreign_keys: Vec::new(),
    }
}

#[test]
fn new_table_emits_single_create() {
    let next = make_design(
        vec![
            {
                let mut c = make_column("Id", "int", false);
                c.is_identity = true;
                c
            },
            make_column("Name", "nvarchar(100)", false),
        ],
        vec!["Id"],
    );

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(None, &next);
    assert_eq!(ddl.len(), 1, "new-table flow should emit exactly one CREATE");
    assert_eq!(ddl[0].kind, "CREATE");
    let sql = &ddl[0].sql;
    assert!(
        sql.contains("CREATE TABLE [dbo].[Widget]"),
        "missing CREATE TABLE header: {sql}"
    );
    assert!(sql.contains("[Id] int IDENTITY(1,1) NOT NULL"), "id line missing: {sql}");
    assert!(sql.contains("[Name] nvarchar(100) NOT NULL"), "name line missing: {sql}");
    assert!(
        sql.contains("CONSTRAINT [PK_Widget] PRIMARY KEY ([Id])"),
        "PK missing: {sql}"
    );
}

#[test]
fn adding_a_column_emits_alter_add() {
    let current = make_design(vec![make_column("Id", "int", false)], vec!["Id"]);
    let mut next = current.clone();
    next.columns.push(make_column("Name", "nvarchar(50)", true));

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    assert!(!ddl.is_empty(), "expected at least one ALTER");
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("ALTER TABLE [dbo].[Widget] ADD [Name] nvarchar(50) NULL")),
        "no ADD COLUMN for Name in {sql:?}"
    );
}

#[test]
fn dropping_a_column_emits_alter_drop() {
    let current = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("Deprecated", "int", true),
        ],
        vec!["Id"],
    );
    let mut next = current.clone();
    next.columns.retain(|c| c.name != "Deprecated");

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("ALTER TABLE [dbo].[Widget] DROP COLUMN [Deprecated]")),
        "no DROP COLUMN in {sql:?}"
    );
}

#[test]
fn nullability_flip_emits_alter_column() {
    let current = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("Note", "nvarchar(50)", true),
        ],
        vec!["Id"],
    );
    let mut next = current.clone();
    next.columns
        .iter_mut()
        .find(|c| c.name == "Note")
        .expect("Note col present")
        .is_nullable = false;

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("ALTER COLUMN [Note] nvarchar(50) NOT NULL")),
        "expected NOT NULL alter in {sql:?}"
    );
}

#[test]
fn adding_an_index_emits_create_index() {
    let current = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("Name", "nvarchar(50)", false),
        ],
        vec!["Id"],
    );
    let mut next = current.clone();
    next.indexes.push(DesignIndex {
        name: "IX_Widget_Name".into(),
        is_unique: false,
        columns: vec!["Name".into()],
    });

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("CREATE INDEX [IX_Widget_Name] ON [dbo].[Widget] ([Name])")),
        "expected CREATE INDEX in {sql:?}"
    );
}

#[test]
fn changing_pk_drops_then_recreates() {
    let current = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("TenantId", "int", false),
        ],
        vec!["Id"],
    );
    let mut next = current.clone();
    // Composite PK now.
    next.primary_key = vec!["TenantId".into(), "Id".into()];

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);

    // Verify sequence: DROP CONSTRAINT precedes ADD CONSTRAINT PK.
    let drop_idx = ddl
        .iter()
        .position(|d| d.sql.contains("DROP CONSTRAINT [PK_Widget]"));
    let add_idx = ddl
        .iter()
        .position(|d| d.sql.contains("ADD CONSTRAINT [PK_Widget] PRIMARY KEY ([TenantId], [Id])"));
    assert!(drop_idx.is_some(), "no PK DROP CONSTRAINT: {ddl:?}");
    assert!(add_idx.is_some(), "no PK ADD CONSTRAINT: {ddl:?}");
    assert!(
        drop_idx.unwrap() < add_idx.unwrap(),
        "DROP must come before ADD for PK: {ddl:?}"
    );
}

#[test]
fn identical_designs_produce_no_ddl() {
    let d = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("Name", "nvarchar(50)", true),
        ],
        vec!["Id"],
    );
    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&d), &d);
    assert!(ddl.is_empty(), "no-op design should emit zero DDL, got {ddl:?}");
}

#[test]
fn adding_foreign_key_emits_alter_add_constraint() {
    let current = make_design(
        vec![
            make_column("Id", "int", false),
            make_column("TenantId", "int", false),
        ],
        vec!["Id"],
    );
    let mut next = current.clone();
    next.foreign_keys.push(DesignForeignKey {
        name: "FK_Widget_Tenant".into(),
        columns: vec!["TenantId".into()],
        referenced_schema: "dbo".into(),
        referenced_table: "Tenant".into(),
        referenced_columns: vec!["Id".into()],
        on_delete: Some("CASCADE".into()),
        on_update: None,
    });

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter().any(|s| s.contains(
            "ADD CONSTRAINT [FK_Widget_Tenant] FOREIGN KEY ([TenantId]) REFERENCES [dbo].[Tenant] ([Id]) ON DELETE CASCADE"
        )),
        "expected FK ADD in {sql:?}"
    );
}

#[test]
fn identity_flag_change_emits_skip_note() {
    let mut current_col = make_column("Id", "int", false);
    current_col.is_identity = false;
    let current = make_design(vec![current_col], vec!["Id"]);

    let mut next_col = make_column("Id", "int", false);
    next_col.is_identity = true;
    let next = make_design(vec![next_col], vec!["Id"]);

    let provider = SqlServerTableDesignerProvider;
    let ddl = provider.generate_ddl(Some(&current), &next);
    let sql: Vec<&str> = ddl.iter().map(|d| d.sql.as_str()).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("identity/computed flag changed")),
        "expected identity-transition note in {sql:?}"
    );
    // Never silently emit ALTER COLUMN when identity is changing.
    assert!(
        !sql.iter()
            .any(|s| s.starts_with("ALTER TABLE [dbo].[Widget] ALTER COLUMN [Id]")),
        "must not emit ALTER for identity flip: {sql:?}"
    );
}
