use tokio::runtime::Runtime;
use tracing::error;
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
};

use crate::{action::Action, root::Root, state::InitialState};

struct Runner {
    root: Root,
    runtime: Runtime,
}

impl Runner {
    fn handle(&mut self, event: Event<'_, Action>) -> eyre::Result<ControlFlow> {
        let _guard = self.runtime.enter();
        self.runtime.block_on(self.root.handle(event))
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let _guard = self.runtime.enter();
        if let Err(error) = self.runtime.block_on(self.root.state.shutdown()) {
            error!("{:?}", error);
        }
        self.root.context.device.wait_idle().unwrap()
    }
}

pub fn run(
    runtime: Runtime,
    event_loop: EventLoop<Action>,
    initial_state: InitialState,
) -> eyre::Result<()> {
    let _guard = runtime.enter();
    let root = Root::new(&event_loop, initial_state.create())?;
    let mut runner = Runner { root, runtime };
    event_loop.run(move |event, _, control_flow| match runner.handle(event) {
        Ok(flow) => *control_flow = flow,
        Err(error) => {
            error!("{:?}", error);
            *control_flow = ControlFlow::Exit;
        }
    });
}
