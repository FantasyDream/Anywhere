//! Graph identities and their presentation. Navigation references existing context roots.
use super::ResolvedContext;
use anywhere_protocol::APPLICATION;

pub fn key(context: &ResolvedContext) -> String {
    format!(
        "context:{:?}:{}:{:016x}",
        context.source,
        context.type_id,
        context.value.identity_fingerprint()
    )
}

/// One bounded parent/key projection shared by all rows in a snapshot.
pub struct RootIndex {
    keys: Vec<String>,
    roots: Vec<usize>,
}
impl RootIndex {
    pub fn new(contexts: &[ResolvedContext]) -> Self {
        let mut ids = std::collections::HashMap::new();
        for (index, context) in contexts.iter().enumerate() {
            if !context.id.is_empty() {
                ids.entry(context.id.as_str()).or_insert(index);
            }
        }
        let parents: Vec<_> = contexts
            .iter()
            .map(|context| {
                context
                    .source_context_id
                    .as_deref()
                    .and_then(|id| ids.get(id).copied())
                    .filter(|parent| {
                        contexts[*parent].type_id != APPLICATION || context.type_id == APPLICATION
                    })
            })
            .collect();
        let mut roots = vec![usize::MAX; contexts.len()];
        let mut visiting = vec![false; contexts.len()];
        for index in 0..contexts.len() {
            if roots[index] != usize::MAX {
                continue;
            }
            let mut chain = Vec::new();
            let mut current = index;
            let root = loop {
                if roots[current] != usize::MAX {
                    break roots[current];
                }
                if visiting[current] {
                    break current;
                }
                visiting[current] = true;
                chain.push(current);
                let Some(parent) = parents[current] else {
                    break current;
                };
                current = parent;
            };
            for node in chain {
                roots[node] = root;
                visiting[node] = false;
            }
        }
        Self {
            keys: contexts.iter().map(key).collect(),
            roots,
        }
    }
    pub fn key(&self, index: usize) -> &str {
        &self.keys[index]
    }
    pub fn root(&self, index: usize) -> usize {
        self.roots[index]
    }
    pub fn root_key(&self, index: usize) -> &str {
        &self.keys[self.roots[index]]
    }
}

/// Application provenance does not make its directory the application action target.
pub fn root<'a>(
    context: &'a ResolvedContext,
    contexts: &'a [ResolvedContext],
) -> &'a ResolvedContext {
    let mut current = context;
    for _ in 0..contexts.len() {
        let Some(parent) = current
            .source_context_id
            .as_deref()
            .and_then(|id| contexts.iter().find(|item| !id.is_empty() && item.id == id))
        else {
            break;
        };
        if parent.type_id == APPLICATION && current.type_id != APPLICATION {
            break;
        }
        current = parent;
    }
    current
}

pub fn root_key(context: &ResolvedContext, contexts: &[ResolvedContext]) -> String {
    key(root(context, contexts))
}
