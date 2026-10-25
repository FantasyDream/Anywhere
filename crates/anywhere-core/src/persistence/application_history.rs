//! Bounded source reads only. This adapter never executes transformation scripts.
pub(crate) mod discovery;
pub(crate) mod packages;
use crate::features::application_history::model::{Cancelled, MAX_INPUT};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Reader {
    Json,
    Xml,
    Sqlite {
        query: String,
        parameters: Vec<String>,
        tables: Vec<String>,
    },
}
pub(crate) fn read(path: &Path, reader: &Reader, cancelled: Cancelled) -> Result<Value> {
    ensure!(!cancelled(), "source read cancelled");
    if let Reader::Sqlite {
        query,
        parameters,
        tables,
    } = reader
    {
        return sqlite(path, query, parameters, tables, cancelled);
    }
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.len() <= MAX_INPUT as u64,
        "source too large"
    );
    let mut bytes = Vec::new();
    (&mut file)
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    ensure!(
        bytes.len() <= MAX_INPUT
            && before.len() == after.len()
            && before.modified()? == after.modified()?
            && !cancelled(),
        "source changed or cancelled"
    );
    match reader {
        Reader::Json => Ok(serde_json::from_slice(&bytes)?),
        Reader::Xml => {
            let text = std::str::from_utf8(&bytes)?;
            let document = roxmltree::Document::parse_with_options(
                text,
                roxmltree::ParsingOptions {
                    nodes_limit: 50000,
                    ..Default::default()
                },
            )?;
            fn node(
                n: roxmltree::Node<'_, '_>,
                depth: usize,
                count: &mut usize,
                cancelled: &(dyn Fn() -> bool + Sync),
            ) -> Result<Value> {
                *count += 1;
                ensure!(
                    depth <= 64 && *count <= 50000 && !cancelled(),
                    "XML budget exceeded"
                );
                let attributes: serde_json::Map<String, Value> = n
                    .attributes()
                    .map(|a| (a.name().into(), Value::String(a.value().into())))
                    .collect();
                let children: Vec<_> = n
                    .children()
                    .filter(|n| n.is_element())
                    .map(|n| node(n, depth + 1, count, cancelled))
                    .collect::<Result<_>>()?;
                Ok(json!({"name":n.tag_name().name(),"attributes":attributes,"children":children}))
            }
            node(document.root_element(), 0, &mut 0, cancelled.as_ref())
        }
        _ => unreachable!(),
    }
}
fn sqlite(
    path: &Path,
    query: &str,
    parameters: &[String],
    tables: &[String],
    cancelled: Cancelled,
) -> Result<Value> {
    use rusqlite::{
        hooks::{AuthAction, AuthContext, Authorization},
        types::ValueRef,
        Connection, OpenFlags,
    };
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(std::time::Duration::from_millis(100))?;
    db.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        MAX_INPUT as i32,
    );
    db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 64 * 1024);
    db.pragma_update(None, "query_only", true)?;
    db.pragma_update(None, "cache_size", -2048)?;
    db.pragma_update(None, "mmap_size", 0)?;
    let allowed = tables.to_vec();
    db.authorizer(Some(move |context: AuthContext<'_>| match context.action {
        AuthAction::Select => Authorization::Allow,
        AuthAction::Read { table_name, .. } if allowed.iter().any(|t| t == table_name) => {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }));
    let interrupt = cancelled.clone();
    db.progress_handler(1000, Some(move || interrupt()));
    let result = (|| -> Result<Value> {
        let mut statement = db.prepare(query)?;
        ensure!(statement.readonly(), "source query must be read only");
        let names: Vec<_> = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect();
        ensure!(names.len() <= 16, "too many source columns");
        let mut rows = statement.query(rusqlite::params_from_iter(parameters))?;
        let mut output = Vec::new();
        let mut bytes = 0;
        while let Some(row) = rows.next()? {
            ensure!(
                !cancelled() && output.len() < 4096,
                "source rows exceeded budget"
            );
            let mut item = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => json!(n),
                    ValueRef::Text(text) | ValueRef::Blob(text) => {
                        bytes += text.len();
                        ensure!(bytes <= MAX_INPUT, "source too large");
                        Value::String(std::str::from_utf8(text)?.into())
                    }
                    _ => bail!("unsupported source column"),
                };
                item.insert(name.clone(), value);
            }
            output.push(Value::Object(item));
        }
        Ok(json!({"rows":output}))
    })();
    db.progress_handler(0, None::<fn() -> bool>);
    result
}
