//! Private multi-operation documents. Only commit mutates the live session.
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
pub(super) fn generation() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
    )
}
pub(super) struct Draft {
    base: u64,
    created: Instant,
    session: Box<Session>,
}

impl Session {
    pub(super) fn draft_request(&mut self, method: &str, params: Value) -> R {
        #[derive(Deserialize)]
        struct P {
            id: Option<String>,
            generation: Option<String>,
            revision: Option<u64>,
            method: Option<String>,
            #[serde(default)]
            params: Value,
        }
        let p: P = args(params)?;
        self.drafts
            .retain(|_, d| d.created.elapsed() < Duration::from_secs(600));
        if method == "draft.begin" {
            if p.generation.as_deref() != Some(&self.generation)
                || p.revision != Some(self.revision)
            {
                return Err(fail("the document changed; refresh context"));
            }
            if self.editing.is_some() {
                return Err(fail("finish the manual feature or sketch edit first"));
            }
            if self.drafts.len() >= 4 {
                return Err(fail("discard an existing draft first"));
            }
            let id = generation();
            let mut session = Session::new();
            session.model = Model::new(self.model.doc.clone());
            session.path = self.path.clone();
            let result =
                json!({"id": id, "base_revision": self.revision, "snapshot": session.full()?});
            self.drafts.insert(
                id,
                Draft {
                    base: self.revision,
                    created: Instant::now(),
                    session: Box::new(session),
                },
            );
            return Ok(result);
        }
        let id = p.id.ok_or_else(|| RpcError::params("draft id required"))?;
        if method == "draft.discard" {
            self.drafts.remove(&id);
            return Ok(json!({"discarded":true}));
        }
        if method == "draft.commit" && self.committed.contains_key(&id) {
            let mut value = self.full()?;
            value["already_committed"] = json!(true);
            return Ok(value);
        }
        let draft = self
            .drafts
            .get_mut(&id)
            .ok_or_else(|| fail("draft missing or expired"))?;
        if draft.base != self.revision || self.editing.is_some() {
            return Err(fail(
                "the document changed; discard this draft and start again",
            ));
        }
        match method {
            "draft.apply" => {
                let method = p
                    .method
                    .ok_or_else(|| RpcError::params("method required"))?;
                if !crate::mcp::assistant_methods().contains(&method.as_str()) {
                    return Err(RpcError::params(
                        "operation is not available inside a draft",
                    ));
                }
                let before = draft.session.model.doc.clone();
                let editing = draft.session.editing;
                let result = draft.session.handle(&method, p.params);
                if result.is_err() {
                    draft.session.replace_doc(before);
                    draft.session.editing = editing;
                }
                draft.session.undo.clear();
                draft.session.redo.clear();
                let result = result?;
                let errors = errors(&draft.session.model);
                Ok(
                    json!({"result":result, "feature_errors":errors, "draft_revision":draft.session.revision}),
                )
            }
            "draft.inspect" | "draft.preview" => {
                draft.session.editing = None;
                let changed = draft.session.model.doc != self.model.doc;
                Ok(
                    json!({"snapshot":draft.session.full()?, "changed":changed, "feature_errors": errors(&draft.session.model), "blocking_errors": blocking_errors(&self.model, &draft.session.model)}),
                )
            }
            "draft.commit" => {
                draft.session.editing = None;
                draft.session.model.doc.validate().map_err(fail)?;
                draft.session.model.rebuild_all();
                if let Some((id, error)) = blocking_errors(&self.model, &draft.session.model)
                    .into_iter()
                    .next()
                {
                    return Err(fail(format!("resolve feature {id}: {error}")));
                }
                // Validate scene generation before changing live history/state.
                draft.session.full()?;
                let mut draft = self.drafts.remove(&id).unwrap();
                if draft.session.model.doc != self.model.doc {
                    self.checkpoint();
                    std::mem::swap(&mut self.model, &mut draft.session.model);
                }
                if self.committed.len() >= 32 {
                    self.committed.clear();
                }
                self.committed.insert(id, true);
                self.full()
            }
            _ => Err(RpcError::params("unknown draft operation")),
        }
    }
}

fn errors(model: &Model) -> BTreeMap<u32, String> {
    model
        .statuses()
        .into_iter()
        .filter(|s| !s.suppressed && !s.rolled_back)
        .filter_map(|s| s.error.map(|e| (s.id, e)))
        .collect()
}

fn blocking_errors(base: &Model, proposed: &Model) -> BTreeMap<u32, String> {
    let baseline = errors(base);
    errors(proposed)
        .into_iter()
        .filter(|(id, error)| {
            baseline.get(id) != Some(error) || base.doc.feature(*id) != proposed.doc.feature(*id)
        })
        .collect()
}
