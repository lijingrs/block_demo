use std::{io};
use chrono::{Utc};
use tracing_appender_localtime::non_blocking::WorkerGuard;
use tracing_appender_localtime::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{self, fmt, fmt::time::FormatTime, EnvFilter, Registry};

struct LocalTimer;

pub struct Logger;

impl Logger {
    pub fn init() -> WorkerGuard {
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("server")
            .filename_suffix("log")
            .build("/app/logs/demo/")
            .expect("无法初始化滚动文件追加器");
        let (non_blocking_file, worker_guard) =
            tracing_appender_localtime::non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_writer(non_blocking_file)
            .with_ansi(false)
            .with_line_number(true)
            .with_target(true)
            .with_level(true)
            .with_timer(LocalTimer);

        //配置控制台日志
        let console_layer = fmt::layer()
            .with_writer(io::stdout)
            .with_ansi(false)
            .with_line_number(true)
            .with_target(true)
            .with_level(true)
            .with_timer(LocalTimer);

        let subscriber = Registry::default()
            .with(console_layer)
            .with(file_layer)
            .with(EnvFilter::new("info"));
        tracing::subscriber::set_global_default(subscriber).expect("failed to set global subscriber");
        worker_guard
    }
}

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Utc::now().naive_local().format("%Y-%m-%d %H:%M:%S"))
    }
}
