use crate::{compare, engine, model::*, repo, store::*};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{broadcast, mpsc, Notify},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerMessage {
    pub phase: String,
    #[serde(default)]
    pub count: usize,
    pub snapshot: Option<String>,
    pub receipt: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone)]
enum Work {
    Snapshot(BuildRequest),
    Comparison(CompareRequest),
}
struct Item {
    id: String,
    key: String,
    work: Work,
    cancel: Arc<Notify>,
}
struct Control {
    job: Job,
    cancel: Arc<Notify>,
}
pub struct Coordinator {
    pub repository: PathBuf,
    pub store: Store,
    jobs: Mutex<BTreeMap<String, Control>>,
    active: Mutex<BTreeMap<String, String>>,
    events: broadcast::Sender<Job>,
    queue: mpsc::Sender<Item>,
    serial: AtomicU64,
}
impl Coordinator {
    pub fn new(repository: PathBuf, store: Store) -> Arc<Self> {
        let (queue, rx) = mpsc::channel(64);
        let (events, _) = broadcast::channel(256);
        let this = Arc::new(Self {
            repository,
            store,
            jobs: Mutex::new(BTreeMap::new()),
            active: Mutex::new(BTreeMap::new()),
            events,
            queue,
            serial: AtomicU64::new(1),
        });
        let task = this.clone();
        tokio::spawn(async move {
            task.run(rx).await;
        });
        this
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Job> {
        self.events.subscribe()
    }
    pub fn job(&self, id: &str) -> Result<Job> {
        self.jobs
            .lock()
            .unwrap()
            .get(id)
            .map(|c| c.job.clone())
            .context("Unknown job")
    }
    pub fn cancel(&self, id: &str) -> Result<Job> {
        let mut jobs = self.jobs.lock().unwrap();
        let control = jobs.get_mut(id).context("Unknown job")?;
        if !["ready", "failed", "cancelled"].contains(&control.job.state.as_str()) {
            control.job.state = "cancelled".into();
            control.job.phase = "cancelled".into();
            control.job.sequence += 1;
            control.cancel.notify_one();
            let _ = self.events.send(control.job.clone());
        }
        Ok(control.job.clone())
    }
    fn update(
        &self,
        id: &str,
        state: &str,
        phase: &str,
        count: usize,
        result: Option<String>,
        error: Option<String>,
    ) {
        if let Some(c) = self.jobs.lock().unwrap().get_mut(id) {
            if c.job.state == "cancelled" {
                return;
            }
            c.job.state = state.into();
            c.job.phase = phase.into();
            c.job.sequence += 1;
            c.job.result_id = result;
            c.job.error = error;
            c.job.counters.insert(phase.into(), count);
            let _ = self.events.send(c.job.clone());
        }
    }
    pub async fn snapshot(&self, mut request: BuildRequest) -> Result<ReadyOrJob> {
        let git = repo::open(&self.repository)?;
        request.revision = repo::resolve(&git, &request.revision)?;
        request.profile.extensions.sort();
        request.profile.extensions.dedup();
        let id = snapshot_id(&repo::identity(&git), &request.revision, &request.profile)?;
        drop(git);
        if !request.force
            && self
                .store
                .load_snapshot(&id)
                .ok()
                .flatten()
                .is_some_and(|s| {
                    s.files
                        .values()
                        .filter_map(|f| f.object.as_ref())
                        .all(|key| self.store.get(key).is_ok())
                })
        {
            return Ok(ReadyOrJob {
                state: "ready".into(),
                id,
                kind: "snapshot".into(),
            });
        }
        self.submit(id, Work::Snapshot(request), "snapshot").await
    }
    pub async fn comparison(&self, mut request: CompareRequest) -> Result<ReadyOrJob> {
        let git = repo::open(&self.repository)?;
        request.base = repo::resolve(&git, &request.base)?;
        request.target = repo::resolve(&git, &request.target)?;
        request.profile.extensions.sort();
        request.profile.extensions.dedup();
        let a = snapshot_id(&repo::identity(&git), &request.base, &request.profile)?;
        let b = snapshot_id(&repo::identity(&git), &request.target, &request.profile)?;
        let id = compare::comparison_id(&a, &b)?;
        drop(git);
        if self.store.comparison(&id).is_ok() {
            return Ok(ReadyOrJob {
                state: "ready".into(),
                id,
                kind: "comparison".into(),
            });
        }
        self.submit(id, Work::Comparison(request), "comparison")
            .await
    }
    async fn submit(&self, key: String, work: Work, kind: &str) -> Result<ReadyOrJob> {
        let id;
        let cancel = Arc::new(Notify::new());
        {
            let mut active = self.active.lock().unwrap();
            if let Some(id) = active.get(&key) {
                if self.job(id).is_ok_and(|j| j.state != "cancelled") {
                    return Ok(ReadyOrJob {
                        state: "queued".into(),
                        id: id.clone(),
                        kind: kind.into(),
                    });
                }
            }
            id = format!("job-{}", self.serial.fetch_add(1, Ordering::SeqCst));
            let job = Job {
                id: id.clone(),
                state: "queued".into(),
                phase: "queued".into(),
                sequence: 1,
                result_id: None,
                result_kind: kind.into(),
                error: None,
                counters: BTreeMap::new(),
            };
            self.jobs.lock().unwrap().insert(
                id.clone(),
                Control {
                    job: job.clone(),
                    cancel: cancel.clone(),
                },
            );
            active.insert(key.clone(), id.clone());
            let _ = self.events.send(job);
        }
        self.queue
            .send(Item {
                id: id.clone(),
                key,
                work,
                cancel,
            })
            .await
            .context("Build coordinator stopped")?;
        Ok(ReadyOrJob {
            state: "queued".into(),
            id,
            kind: kind.into(),
        })
    }
    async fn run(self: Arc<Self>, mut rx: mpsc::Receiver<Item>) {
        let mut worker: Option<Worker> = None;
        while let Some(item) = rx.recv().await {
            if self.job(&item.id).is_ok_and(|j| j.state == "cancelled") {
                self.finish_key(&item);
                continue;
            }
            self.update(&item.id, "running", "starting", 0, None, None);
            let result = tokio::select! {
                r=self.execute(&item,&mut worker)=>r,
                _=item.cancel.notified()=>{if let Some(mut w)=worker.take(){let _=w.child.kill().await;let _=w.child.wait().await;}Err(anyhow::anyhow!("Build cancelled"))}
            };
            match result {
                Ok(id) => self.update(&item.id, "ready", "ready", 0, Some(id), None),
                Err(e) => {
                    self.update(
                        &item.id,
                        "failed",
                        "failed",
                        0,
                        None,
                        Some(format!("{e:#}")),
                    );
                    if let Some(mut w) = worker.take() {
                        let _ = w.child.kill().await;
                        let _ = w.child.wait().await;
                    }
                }
            }
            self.finish_key(&item);
        }
    }
    fn finish_key(&self, item: &Item) {
        let mut a = self.active.lock().unwrap();
        if a.get(&item.key) == Some(&item.id) {
            a.remove(&item.key);
        }
    }
    async fn execute(&self, item: &Item, worker: &mut Option<Worker>) -> Result<String> {
        match &item.work {
            Work::Snapshot(r) => Ok(self.build(&item.id, r, worker, true).await?.id),
            Work::Comparison(r) => {
                let a = self
                    .build(
                        &item.id,
                        &BuildRequest {
                            revision: r.base.clone(),
                            profile: r.profile.clone(),
                            force: false,
                        },
                        worker,
                        false,
                    )
                    .await?;
                let b = self
                    .build(
                        &item.id,
                        &BuildRequest {
                            revision: r.target.clone(),
                            profile: r.profile.clone(),
                            force: false,
                        },
                        worker,
                        false,
                    )
                    .await?;
                self.update(&item.id, "running", "comparing", 0, None, None);
                let store = self.store.clone();
                let c =
                    tokio::task::spawn_blocking(move || compare::compare(&a, &b, &store)).await??;
                self.update(
                    &item.id,
                    "validating",
                    "validating-comparison",
                    c.changes.len(),
                    None,
                    None,
                );
                let jobs = self.jobs.lock().unwrap();
                if jobs[&item.id].job.state == "cancelled" {
                    bail!("Build cancelled");
                }
                self.store.save_comparison(&c)?;
                Ok(c.id)
            }
        }
    }
    async fn build(
        &self,
        job: &str,
        request: &BuildRequest,
        worker: &mut Option<Worker>,
        final_snapshot: bool,
    ) -> Result<Snapshot> {
        if worker.is_none() {
            *worker = Some(Worker::spawn(&self.repository, &self.store.root)?);
        }
        let w = worker.as_mut().unwrap();
        w.input
            .write_all(format!("{}\n", serde_json::to_string(request)?).as_bytes())
            .await?;
        w.input.flush().await?;
        loop {
            let line =
                w.output.next_line().await?.context(
                    "Parser worker exited unexpectedly; completed snapshots are preserved",
                )?;
            let event: WorkerMessage =
                serde_json::from_str(&line).context("Invalid parser worker message")?;
            if let Some(error) = event.error {
                bail!(error);
            }
            if let (Some(snapshot), Some(receipt)) = (event.snapshot, event.receipt) {
                let s: Snapshot = self.store.get_snapshot(&snapshot)?;
                let receipt: BuildReceipt = self.store.get_json(&receipt)?;
                if receipt.stats.mode != "cache-hit" {
                    engine::validate(&s, &self.store)?;
                }
                let jobs = self.jobs.lock().unwrap();
                if jobs[job].job.state == "cancelled" {
                    bail!("Build cancelled");
                }
                if receipt.stats.mode != "cache-hit" {
                    self.store.publish(&s, &receipt)?;
                }
                return Ok(s);
            }
            self.update(
                job,
                if final_snapshot && event.phase == "validating" {
                    "validating"
                } else {
                    "running"
                },
                &event.phase,
                event.count,
                None,
                None,
            );
        }
    }
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
}
impl Worker {
    fn spawn(repository: &std::path::Path, cache: &std::path::Path) -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("worker")
            .arg("--repository")
            .arg(repository)
            .arg("--cache-dir")
            .arg(cache)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        Ok(Self {
            child,
            input,
            output,
        })
    }
}

pub fn worker_main(repository: PathBuf, cache: PathBuf) -> Result<()> {
    use std::io::{BufRead, Write};
    let store = Store::open(cache)?;
    let mut engine = engine::Engine::new(&repository, store.clone())?;
    let emit = |m: WorkerMessage| {
        println!("{}", serde_json::to_string(&m).unwrap());
        let _ = std::io::stdout().flush();
    };
    for line in std::io::stdin().lock().lines() {
        let request: BuildRequest = serde_json::from_str(&line?)?;
        match engine.build(&request, |phase, count| {
            emit(WorkerMessage {
                phase: phase.into(),
                count,
                snapshot: None,
                receipt: None,
                error: None,
            })
        }) {
            Ok((snapshot, receipt)) => emit(WorkerMessage {
                phase: "candidate".into(),
                count: 0,
                snapshot: Some(if receipt.stats.mode == "cache-hit" {
                    store
                        .snapshot_key(&snapshot.id)?
                        .context("Cached snapshot index disappeared")?
                } else {
                    store.put_snapshot(&snapshot)?
                }),
                receipt: Some(store.put_json(&receipt)?),
                error: None,
            }),
            Err(e) => {
                emit(WorkerMessage {
                    phase: "failed".into(),
                    count: 0,
                    snapshot: None,
                    receipt: None,
                    error: Some(format!("{e:#}")),
                });
                break;
            }
        }
    }
    Ok(())
}
