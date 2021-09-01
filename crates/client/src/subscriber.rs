use std::{
    io::{LineWriter, Stdout, Write},
    sync::{Arc, Mutex},
};

use eyre::Context;
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{
        self,
        format::{DefaultFields, Format, Full},
        time::SystemTime,
        MakeWriter,
    },
    layer::Layered,
    prelude::__tracing_subscriber_SubscriberExt,
    reload, Layer,
};
use winit::event_loop::EventLoopProxy;

use crate::action::Action;

pub struct ProxyWriter(Arc<Mutex<EventLoopProxy<Action>>>);

pub type FilterHandle = reload::Handle<
    LevelFilter,
    Layered<
        fmt::Layer<
            fmt::Subscriber<DefaultFields, Format<Full, SystemTime>, LevelFilter, fn() -> Stdout>,
            DefaultFields,
            Format<Full, SystemTime>,
            CreateWriter,
        >,
        fmt::Subscriber<DefaultFields, Format<Full, SystemTime>, LevelFilter, fn() -> Stdout>,
        fmt::Subscriber<DefaultFields, Format<Full, SystemTime>, LevelFilter, fn() -> Stdout>,
    >,
>;

impl Write for ProxyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let proxy = self.0.lock().unwrap();
        let len = buf.len();
        let _ = proxy.send_event(Action::Log(buf.to_vec()));
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct CreateWriter(Arc<Mutex<EventLoopProxy<Action>>>);

impl MakeWriter for CreateWriter {
    type Writer = LineWriter<ProxyWriter>;

    fn make_writer(&self) -> Self::Writer {
        LineWriter::new(ProxyWriter(self.0.clone()))
    }
}

pub fn init_subscriber(proxy: EventLoopProxy<Action>) -> eyre::Result<FilterHandle> {
    let (layer, handle) = reload::Layer::new(LevelFilter::TRACE);
    let proxy_layer = fmt::layer()
        .with_writer(CreateWriter(Arc::new(Mutex::new(proxy))))
        .with_ansi(false);
    let subscriber = layer.with_subscriber(tracing_subscriber::fmt().finish().with(proxy_layer));
    tracing::subscriber::set_global_default(subscriber).wrap_err("could not set subscriber")?;
    Ok(handle)
}
