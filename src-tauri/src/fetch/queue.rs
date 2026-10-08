//! ドメインごとのキュー（ADR 0004、ADR 0015）。
//!
//! キューは中身を知らない。積むのは「呼ぶと Future を返す関数」で、取得も待ち時間も同じタスクとして扱う。
//! 各ドメインのキューはタスクを 1 つ取り出して実行の側（tokio）に渡し、終わるのを待ってから次を取り出す。
//! そのため、同じドメインのタスクは順に、異なるドメインのタスクは並列に動く。

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::Notify;

pub type TaskFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// 呼ぶと Future を返す関数。積んだ時点では何も実行されない
pub type Task = Box<dyn FnOnce() -> TaskFuture + Send + 'static>;

pub fn task<F, Fut>(f: F) -> Task
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Box::new(move || Box::pin(f()))
}

/// 決めた時間だけ待つタスク
pub fn wait(duration: Duration) -> Task {
    task(move || tokio::time::sleep(duration))
}

#[derive(Default)]
struct DomainQueue {
    tasks: Mutex<VecDeque<Task>>,
    notify: Notify,
}

struct Inner {
    handle: Handle,
    domains: Mutex<HashMap<String, Arc<DomainQueue>>>,
}

/// ドメインごとのキューの集まり。複製しても同じキューを指す
#[derive(Clone)]
pub struct Queues {
    inner: Arc<Inner>,
}

impl Queues {
    /// `handle` の上でタスクを動かす
    pub fn new(handle: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                handle,
                domains: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// ドメインのキュー。初めてのドメインなら、キューを作って取り出す側を動かし始める
    fn domain(&self, domain: &str) -> Arc<DomainQueue> {
        let mut domains = self.inner.domains.lock().unwrap();
        if let Some(queue) = domains.get(domain) {
            return queue.clone();
        }
        let queue = Arc::new(DomainQueue::default());
        domains.insert(domain.to_owned(), queue.clone());
        self.inner
            .handle
            .spawn(run(self.inner.handle.clone(), queue.clone()));
        queue
    }

    /// 末尾に積む。複数のタスクは、間にほかのタスクが入らないように続けて積む
    pub fn push_back(&self, domain: &str, tasks: Vec<Task>) {
        let queue = self.domain(domain);
        queue.tasks.lock().unwrap().extend(tasks);
        queue.notify.notify_one();
    }

    /// 先頭に積む。複数のタスクは、渡した順のまま先頭に並ぶ
    pub fn push_front(&self, domain: &str, tasks: Vec<Task>) {
        let queue = self.domain(domain);
        {
            let mut queued = queue.tasks.lock().unwrap();
            for task in tasks.into_iter().rev() {
                queued.push_front(task);
            }
        }
        queue.notify.notify_one();
    }

    /// 全てのドメインの、まだ取り出していないタスクを破棄する。実行中のタスクは最後まで動く
    pub fn clear(&self) {
        for queue in self.inner.domains.lock().unwrap().values() {
            queue.tasks.lock().unwrap().clear();
        }
    }

    /// まだ取り出していないタスクの数
    pub fn len(&self) -> usize {
        self.inner
            .domains
            .lock()
            .unwrap()
            .values()
            .map(|queue| queue.tasks.lock().unwrap().len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// ドメインのキューからタスクを 1 つずつ取り出し、終わるのを待ってから次を取り出す
async fn run(handle: Handle, queue: Arc<DomainQueue>) {
    loop {
        let next = queue.tasks.lock().unwrap().pop_front();
        match next {
            // タスクが panic しても、キューは止めずに次へ進む
            Some(task) => {
                let _ = handle.spawn(task()).await;
            }
            None => queue.notify.notified().await,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tokio::time::Instant;

    /// タスクの開始と終了を、開始からの経過秒とともに記録する
    #[derive(Clone)]
    pub(crate) struct Log {
        start: Instant,
        events: Arc<Mutex<Vec<String>>>,
    }

    impl Log {
        pub(crate) fn new() -> Self {
            Self {
                start: Instant::now(),
                events: Arc::default(),
            }
        }

        pub(crate) fn push(&self, event: impl Into<String>) {
            let secs = self.start.elapsed().as_secs();
            self.events
                .lock()
                .unwrap()
                .push(format!("{secs}s {}", event.into()));
        }

        pub(crate) fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }

        /// `name` の開始と終了を記録し、その間 `secs` 秒かかるタスク
        pub(crate) fn job(&self, name: &str, secs: u64) -> Task {
            let log = self.clone();
            let name = name.to_owned();
            task(move || async move {
                log.push(format!("start {name}"));
                tokio::time::sleep(Duration::from_secs(secs)).await;
                log.push(format!("end {name}"));
            })
        }
    }

    /// 時間を止めた状態で、積んだタスクが全て終わるまで時間を進める
    pub(crate) async fn settle(secs: u64) {
        tokio::time::sleep(Duration::from_secs(secs)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn runs_tasks_of_one_domain_in_order() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back("a", vec![log.job("1", 2), log.job("2", 1)]);
        queues.push_back("a", vec![log.job("3", 1)]);
        settle(10).await;

        assert_eq!(
            log.events(),
            [
                "0s start 1",
                "2s end 1",
                "2s start 2",
                "3s end 2",
                "3s start 3",
                "4s end 3"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn runs_domains_in_parallel() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back("a", vec![log.job("a1", 2), log.job("a2", 2)]);
        queues.push_back("b", vec![log.job("b1", 3)]);
        settle(10).await;

        let mut events = log.events();
        events.sort();
        assert_eq!(
            events,
            [
                "0s start a1",
                "0s start b1",
                "2s end a1",
                "2s start a2",
                "3s end b1",
                "4s end a2"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_call_task_when_pushed() {
        let queues = Queues::new(Handle::current());
        let called = Arc::new(Mutex::new(false));
        let flag = called.clone();
        // 先に長い待ちを積み、後ろのタスクが積んだ時点で呼ばれないことを確かめる
        queues.push_back(
            "a",
            vec![
                wait(Duration::from_secs(5)),
                task(move || async move { *flag.lock().unwrap() = true }),
            ],
        );
        settle(4).await;
        assert!(!*called.lock().unwrap());
        settle(2).await;
        assert!(*called.lock().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn wait_tasks_keep_their_place() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back(
            "a",
            vec![
                log.job("1", 0),
                wait(Duration::from_secs(5)),
                log.job("2", 0),
                wait(Duration::from_secs(5)),
            ],
        );
        settle(20).await;
        assert_eq!(
            log.events(),
            ["0s start 1", "0s end 1", "5s start 2", "5s end 2"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn push_front_runs_before_queued_tasks() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back("a", vec![log.job("running", 2), log.job("queued", 0)]);
        settle(1).await;
        queues.push_front("a", vec![log.job("front1", 0), log.job("front2", 0)]);
        settle(10).await;

        assert_eq!(
            log.events(),
            [
                "0s start running",
                "2s end running",
                "2s start front1",
                "2s end front1",
                "2s start front2",
                "2s end front2",
                "2s start queued",
                "2s end queued",
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn clear_drops_queued_tasks_but_not_running_one() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back("a", vec![log.job("running", 3), log.job("a2", 0)]);
        queues.push_back("b", vec![log.job("b1", 3), log.job("b2", 0)]);
        settle(1).await;
        queues.clear();
        assert!(queues.is_empty());
        settle(10).await;

        let mut events = log.events();
        events.sort();
        assert_eq!(
            events,
            [
                "0s start b1",
                "0s start running",
                "3s end b1",
                "3s end running"
            ]
        );

        // 破棄した後も、積めば動く
        queues.push_back("a", vec![log.job("after", 0)]);
        settle(1).await;
        assert!(log.events().iter().any(|e| e.ends_with("start after")));
    }

    #[tokio::test(start_paused = true)]
    async fn keeps_running_after_task_panics() {
        let queues = Queues::new(Handle::current());
        let log = Log::new();
        queues.push_back(
            "a",
            vec![task(|| async { panic!("task failed") }), log.job("next", 0)],
        );
        settle(1).await;
        assert_eq!(log.events(), ["0s start next", "0s end next"]);
    }
}
