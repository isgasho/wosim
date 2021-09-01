use std::{
    ffi::CString,
    fs::read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use ::vulkan::Instance;
use generator::{Generator, Template};
use headless::error::Error;
use headless::vulkan::DeviceCandidate;
use jsonwebtoken::DecodingKey;
use network::from_pem;
use semver::Version;
use server::{Server, ServerConfiguration, ServerType};
use structopt::StructOpt;
use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
use util::iterator::MaxOkFilterMap;

#[derive(StructOpt)]
#[structopt(name = "wosim-headless")]
enum Command {
    Serve {
        #[structopt(long, short, default_value = "2021")]
        port: u16,
        #[structopt(
            long,
            env("WOSIM_SERVER_CERTIFICATE"),
            default_value = "/etc/wosim/ssl/server/certificate.pem"
        )]
        certificate: PathBuf,
        #[structopt(
            long,
            env("WOSIM_SERVER_PRIVATE_KEY"),
            default_value = "/etc/wosim/ssl/server/private.pem"
        )]
        private_key: PathBuf,
        #[structopt(
            long,
            env("WOSIM_AUTHENTICATION_PUBLIC_KEY"),
            default_value = "/etc/wosim/ssl/authentication/public.pem"
        )]
        decode_key: PathBuf,
    },
    Create,
}

impl Command {
    fn run(self) -> Result<(), Error> {
        let runtime = Runtime::new()?;
        let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let instance = Arc::new(Instance::new(
            &CString::new("wosim").unwrap(),
            version,
            vec![],
        )?);
        let device = instance
            .physical_devices()?
            .into_iter()
            .max_ok_filter_map(DeviceCandidate::new)?
            .ok_or(Error::NoSuitableDeviceFound)?
            .create()?;
        match self {
            Command::Serve {
                port,
                certificate,
                private_key,
                decode_key,
            } => runtime.block_on(async {
                let running = Arc::new(AtomicBool::new(true));
                let r = running.clone();
                ctrlc::set_handler(move || {
                    r.store(false, Ordering::SeqCst);
                })
                .unwrap();
                let (certificate_chain, private_key) =
                    from_pem(certificate, private_key).map_err(Error::FromPem)?;
                let rsa_pem = read(decode_key)?;
                let decoding_key = DecodingKey::from_rsa_pem(&rsa_pem)
                    .map_err(Error::InvalidDecodeKey)?
                    .into_static();
                let mut server = Server::new(ServerConfiguration {
                    port,
                    r#type: ServerType::Dedicated {
                        certificate_chain,
                        private_key,
                        decoding_key,
                    },
                    action_buffer: 64,
                    request_buffer: 16,
                    tick_period: Duration::from_millis(50),
                })
                .map_err(Error::Endpoint)?;
                while running.load(Ordering::SeqCst) {
                    sleep(Duration::from_millis(10)).await;
                }
                server.stop().await.map_err(Error::Service)?;
                Ok(())
            }),
            Command::Create => runtime.block_on(async {
                let (sender, mut receiver) = mpsc::channel(16);
                let mut generator = Generator::new(Template {}, sender, Arc::new(device));
                let control = generator.control.clone();
                ctrlc::set_handler(move || {
                    control.cancel();
                })
                .unwrap();
                while let Some(notification) = receiver.recv().await {
                    match notification {}
                }
                generator
                    .join()
                    .await
                    .map_err(|_| Error::NoSuitableDeviceFound)
            }),
        }
    }
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    Command::from_args().run()
}
