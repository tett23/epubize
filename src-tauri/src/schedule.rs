//! 定期的な fetch all（ADR 0019）。
//!
//! アプリが動いている間、毎日決めた時刻（ローカル時刻）に fetch all を行う。
//! 壁時計を一定の間隔で見て、前に見たときから決めた時刻を越えていれば行う。
//! tokio の時間はスリープ中に進まないことがあるため、決めた時刻まで眠る方法は取らない。
//! スリープなどで決めた時刻を過ぎていた場合は、起きた後に見たときに行う。

use std::future::Future;
use std::time::Duration;

use chrono::{Local, NaiveDateTime, NaiveTime};

/// 定期取得の設定。管理画面ができるまでは決まった値を使う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub enabled: bool,
    /// 毎日この時刻（ローカル時刻）に行う
    pub at: NaiveTime,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: true,
            at: NaiveTime::from_hms_opt(3, 0, 0).expect("valid time"),
        }
    }
}

/// 壁時計を見る間隔
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// `previous` より後、`now` 以前（両端は `now` を含む）に、毎日の `at` が来たか
pub fn crossed(previous: NaiveDateTime, now: NaiveDateTime, at: NaiveTime) -> bool {
    if now <= previous {
        return false;
    }
    // `now` の日の `at` か、その前日の `at` のうち、`now` 以前で最も新しいもの
    let today = now.date().and_time(at);
    let latest = if today <= now {
        today
    } else {
        today - chrono::Duration::days(1)
    };
    previous < latest
}

/// `clock` を `CHECK_INTERVAL` ごとに見て、`schedule.at` を越えるたびに `run` を呼ぶ。終わらない
pub async fn run_daily<C, R, F>(schedule: Schedule, clock: C, mut run: R)
where
    C: Fn() -> NaiveDateTime,
    R: FnMut() -> F,
    F: Future<Output = ()>,
{
    if !schedule.enabled {
        return;
    }
    let mut previous = clock();
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    // 起動した直後の 1 回目は飛ばす。起動した時点で行うことはしない
    interval.tick().await;
    loop {
        interval.tick().await;
        let now = clock();
        if crossed(previous, now, schedule.at) {
            run().await;
        }
        previous = now;
    }
}

/// ローカル時刻の現在
pub fn local_now() -> NaiveDateTime {
    Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn three() -> NaiveTime {
        NaiveTime::from_hms_opt(3, 0, 0).unwrap()
    }

    #[test]
    fn crosses_when_time_passes_the_hour() {
        assert!(crossed(
            t("2026-10-09 02:59:30"),
            t("2026-10-09 03:00:30"),
            three()
        ));
        assert!(crossed(
            t("2026-10-09 02:59:00"),
            t("2026-10-09 03:00:00"),
            three()
        ));
    }

    #[test]
    fn does_not_cross_within_the_same_side() {
        assert!(!crossed(
            t("2026-10-09 03:00:00"),
            t("2026-10-09 03:01:00"),
            three()
        ));
        assert!(!crossed(
            t("2026-10-09 01:00:00"),
            t("2026-10-09 02:59:59"),
            three()
        ));
        assert!(!crossed(
            t("2026-10-09 04:00:00"),
            t("2026-10-10 02:00:00"),
            three()
        ));
    }

    #[test]
    fn crosses_after_sleeping_through_the_hour() {
        // 前日の 23 時に眠り、8 時に起きた
        assert!(crossed(
            t("2026-10-08 23:00:00"),
            t("2026-10-09 08:00:00"),
            three()
        ));
        // 何日も眠っていても 1 回とする
        assert!(crossed(
            t("2026-10-05 12:00:00"),
            t("2026-10-09 08:00:00"),
            three()
        ));
    }

    #[test]
    fn does_not_cross_when_clock_goes_back() {
        assert!(!crossed(
            t("2026-10-09 03:30:00"),
            t("2026-10-09 02:30:00"),
            three()
        ));
    }

    /// 偽の壁時計。テストから進める
    #[derive(Clone)]
    struct FakeClock(Arc<Mutex<NaiveDateTime>>);

    impl FakeClock {
        fn now(&self) -> NaiveDateTime {
            *self.0.lock().unwrap()
        }

        fn set(&self, value: NaiveDateTime) {
            *self.0.lock().unwrap() = value;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn runs_once_each_time_the_hour_is_crossed() {
        let clock = FakeClock(Arc::new(Mutex::new(t("2026-10-09 02:58:00"))));
        let runs = Arc::new(Mutex::new(Vec::new()));
        let (reader, c, r) = (clock.clone(), clock.clone(), runs.clone());
        let task = tokio::spawn(run_daily(
            Schedule::default(),
            move || reader.now(),
            move || {
                let (c, r) = (c.clone(), r.clone());
                async move { r.lock().unwrap().push(c.now()) }
            },
        ));

        // 1 分ごとに壁時計を 1 分進める
        for minute in 59..=63 {
            tokio::time::sleep(CHECK_INTERVAL).await;
            let time = if minute < 60 {
                format!("2026-10-09 02:{minute}:00")
            } else {
                format!("2026-10-09 03:{:02}:00", minute - 60)
            };
            clock.set(t(&time));
        }
        // 翌日まで眠っていたことにする
        clock.set(t("2026-10-10 08:00:00"));
        tokio::time::sleep(CHECK_INTERVAL * 3).await;
        task.abort();

        assert_eq!(
            *runs.lock().unwrap(),
            [t("2026-10-09 03:00:00"), t("2026-10-10 08:00:00")]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn does_nothing_when_disabled() {
        let schedule = Schedule {
            enabled: false,
            ..Schedule::default()
        };
        // 無効なら、すぐに終わる
        run_daily(
            schedule,
            || t("2026-10-09 03:00:00"),
            || async { unreachable!() },
        )
        .await;
    }
}
