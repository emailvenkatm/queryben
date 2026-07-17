//! Compute a `SchemaDiff` from two snapshots. Lives here (not in domain)
//! because the diff rules depend on how each provider filled in its
//! `SchemaObject`.

use std::collections::BTreeMap;

use crate::core::schema_diff::{
    ColumnSpec, IndexSpec, ObjectChange, ObjectKind, SchemaCompareOptions, SchemaDiff,
    SchemaObject, SchemaSnapshot,
};

pub fn compute_diff(
    source: &SchemaSnapshot,
    target: &SchemaSnapshot,
    options: &SchemaCompareOptions,
) -> SchemaDiff {
    let key = |o: &SchemaObject| (o.kind.clone(), o.qualified_name.clone());
    let src_map: BTreeMap<_, &SchemaObject> =
        source.objects.iter().map(|o| (key(o), o)).collect();
    let tgt_map: BTreeMap<_, &SchemaObject> =
        target.objects.iter().map(|o| (key(o), o)).collect();

    let mut added: Vec<ObjectChange> = Vec::new();
    let mut dropped: Vec<ObjectChange> = Vec::new();
    let mut changed: Vec<ObjectChange> = Vec::new();
    let mut unchanged_count: u32 = 0;

    for (k, src) in &src_map {
        if !options.includes(&k.0) {
            continue;
        }
        match tgt_map.get(k) {
            None => added.push(ObjectChange {
                kind: k.0.clone(),
                qualified_name: k.1.clone(),
                source: Some((*src).clone()),
                target: None,
                reasons: Vec::new(),
            }),
            Some(tgt) => {
                let reasons = diff_reasons(src, tgt, options);
                if reasons.is_empty() {
                    unchanged_count += 1;
                } else {
                    changed.push(ObjectChange {
                        kind: k.0.clone(),
                        qualified_name: k.1.clone(),
                        source: Some((*src).clone()),
                        target: Some((*tgt).clone()),
                        reasons,
                    });
                }
            }
        }
    }

    for (k, tgt) in &tgt_map {
        if !options.includes(&k.0) {
            continue;
        }
        if !src_map.contains_key(k) {
            dropped.push(ObjectChange {
                kind: k.0.clone(),
                qualified_name: k.1.clone(),
                source: None,
                target: Some((*tgt).clone()),
                reasons: Vec::new(),
            });
        }
    }

    SchemaDiff {
        source_label: source.label.clone(),
        target_label: target.label.clone(),
        added,
        dropped,
        changed,
        unchanged_count,
    }
}

fn diff_reasons(
    source: &SchemaObject,
    target: &SchemaObject,
    options: &SchemaCompareOptions,
) -> Vec<String> {
    let mut reasons = Vec::new();
    match source.kind {
        ObjectKind::Table => {
            let s_cols: BTreeMap<&str, &ColumnSpec> = source
                .columns
                .iter()
                .map(|c| (c.name.as_str(), c))
                .collect();
            let t_cols: BTreeMap<&str, &ColumnSpec> = target
                .columns
                .iter()
                .map(|c| (c.name.as_str(), c))
                .collect();
            for (name, sc) in &s_cols {
                match t_cols.get(name) {
                    None => reasons.push(format!("column '{name}' added")),
                    Some(tc) => {
                        if sc.sql_type != tc.sql_type {
                            reasons.push(format!(
                                "column '{name}' type: {} -> {}",
                                tc.sql_type, sc.sql_type
                            ));
                        }
                        if sc.is_nullable != tc.is_nullable {
                            reasons.push(format!(
                                "column '{name}' nullability: {} -> {}",
                                tc.is_nullable, sc.is_nullable
                            ));
                        }
                    }
                }
            }
            for name in t_cols.keys() {
                if !s_cols.contains_key(name) {
                    reasons.push(format!("column '{name}' dropped"));
                }
            }
            let s_idx: BTreeMap<&str, &IndexSpec> =
                source.indexes.iter().map(|i| (i.name.as_str(), i)).collect();
            let t_idx: BTreeMap<&str, &IndexSpec> =
                target.indexes.iter().map(|i| (i.name.as_str(), i)).collect();
            for (name, si) in &s_idx {
                match t_idx.get(name) {
                    None => reasons.push(format!("index '{name}' added")),
                    Some(ti) => {
                        if si.columns != ti.columns {
                            reasons.push(format!(
                                "index '{name}' columns: [{}] -> [{}]",
                                ti.columns.join(","),
                                si.columns.join(",")
                            ));
                        }
                        if si.is_unique != ti.is_unique {
                            reasons.push(format!("index '{name}' uniqueness changed"));
                        }
                    }
                }
            }
            for name in t_idx.keys() {
                if !s_idx.contains_key(name) {
                    reasons.push(format!("index '{name}' dropped"));
                }
            }
        }
        ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
            let s = normalize_body(source.body.as_deref().unwrap_or(""), options);
            let t = normalize_body(target.body.as_deref().unwrap_or(""), options);
            if s != t {
                reasons.push("body changed".into());
            }
        }
        ObjectKind::Index => {
            let s = source.indexes.first();
            let t = target.indexes.first();
            if let (Some(s), Some(t)) = (s, t) {
                if s.columns != t.columns {
                    reasons.push(format!(
                        "columns: [{}] -> [{}]",
                        t.columns.join(","),
                        s.columns.join(",")
                    ));
                }
                if s.is_unique != t.is_unique {
                    reasons.push("uniqueness changed".into());
                }
            }
        }
    }
    reasons
}

fn normalize_body(body: &str, options: &SchemaCompareOptions) -> String {
    let mut s = body.to_string();
    if options.ignore_whitespace {
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if options.ignore_collation {
        while let Some(idx) = s.to_ascii_uppercase().find(" COLLATE ") {
            let tail = &s[idx + " COLLATE ".len()..];
            let end = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            let mut new = String::with_capacity(s.len());
            new.push_str(&s[..idx]);
            new.push_str(&tail[end..]);
            s = new;
        }
    }
    if options.ignore_fillfactor {
        s = s
            .split(' ')
            .filter(|tok| !tok.eq_ignore_ascii_case("fillfactor"))
            .collect::<Vec<_>>()
            .join(" ");
    }
    s
}
