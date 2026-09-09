use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    process::Command,
    sync::mpsc::Sender,
    time::{Duration, Instant, sleep_until},
};

use crate::{
    approval::{ApprovalDecision, ApprovalId},
    error::AgentError,
    event::AgentEvent,
    options::Budget,
    process::{self, Process},
    session::{self, Channel, Session},
    usage::Usage,
};

// How long a closed stdin has to end the CLI before it is killed.
const GRACE: Duration = Duration::from_secs(5);

// What one step of a protocol wants done: events for the caller, inputs for the process, and
// whether the process should be told to leave.
#[derive(Debug)]
pub struct Effects<I> {
    pub events: Vec<AgentEvent>,
    pub inputs: Vec<I>,
    pub closing: bool,
}

impl<I> Default for Effects<I> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            inputs: Vec::new(),
            closing: false,
        }
    }
}

impl<I> Effects<I> {
    pub fn emit(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    pub fn send(&mut self, input: I) {
        self.inputs.push(input);
    }

    pub fn close(&mut self) {
        self.closing = true;
    }
}

// A backend is a synchronous state machine over its wire format. The loop below owns the process,
// the channels, the budget and the shutdown, so a backend never touches a socket or a clock and a
// test can drive it from a recorded fixture.
pub trait Protocol: Send + 'static {
    type Input: Serialize + Send + 'static;
    type Output: DeserializeOwned + Send + 'static;

    fn open(&mut self, _effects: &mut Effects<Self::Input>) {}
    fn read(&mut self, output: Self::Output, effects: &mut Effects<Self::Input>);
    fn prompt(&mut self, text: String, effects: &mut Effects<Self::Input>);
    fn interrupt(&mut self, effects: &mut Effects<Self::Input>);
    fn approve(
        &mut self,
        id: ApprovalId,
        decision: ApprovalDecision,
        effects: &mut Effects<Self::Input>,
    );
    fn usage(&self) -> Usage;
}

pub fn start<P: Protocol>(
    protocol: P,
    command: Command,
    budget: Budget,
) -> Result<Session, AgentError> {
    let process = process::spawn(command)?;
    let (session, channel) = session::open();
    tokio::spawn(run(protocol, process, channel, budget));
    Ok(session)
}

pub async fn run<P: Protocol>(
    mut protocol: P,
    process: Process<P::Input, P::Output>,
    channel: Channel,
    budget: Budget,
) {
    let Process {
        mut incoming,
        outgoing,
        mut child,
    } = process;
    let mut commands = channel.commands;
    let mut link = Link {
        events: channel.events,
        outgoing: Some(outgoing),
        deadline: None,
    };
    let mut over_budget = false;
    let mut reading = true;
    let mut commanded = true;

    let mut effects = Effects::default();
    protocol.open(&mut effects);
    if !link.apply(effects).await {
        return;
    }

    loop {
        let mut effects = Effects::default();
        tokio::select! {
            output = incoming.recv(), if reading => match output {
                Some(output) => {
                    protocol.read(output, &mut effects);
                    let usage = protocol.usage();
                    if !over_budget && budget.exhausted_by(&usage) {
                        over_budget = true;
                        effects.emit(AgentEvent::BudgetExhausted { session: usage });
                        protocol.interrupt(&mut effects);
                    }
                }
                None => reading = false,
            },
            command = commands.recv(), if commanded => match command {
                Some(session::Command::Prompt(_)) if over_budget => {
                    effects.emit(AgentEvent::Failed {
                        message: "the session has spent its budget".to_owned(),
                        retrying: false,
                    });
                }
                Some(session::Command::Prompt(text)) => protocol.prompt(text, &mut effects),
                Some(session::Command::Interrupt) => protocol.interrupt(&mut effects),
                Some(session::Command::Approve { id, decision }) => {
                    protocol.approve(id, decision, &mut effects);
                }
                // Closing stdin ends the CLI, and the exit arm below reports it.
                Some(session::Command::Shutdown) => effects.close(),
                None => {
                    commanded = false;
                    effects.close();
                }
            },
            _ = sleep_until(link.deadline.unwrap_or_else(Instant::now)), if link.deadline.is_some() => {
                link.deadline = None;
                let _ = child.start_kill();
            }
            status = child.wait() => {
                let code = status.ok().and_then(|status| status.code());
                let _ = link.events.send(AgentEvent::Exited { code }).await;
                return;
            }
        }
        if !link.apply(effects).await {
            return;
        }
    }
}

struct Link<I> {
    events: Sender<AgentEvent>,
    outgoing: Option<Sender<I>>,
    deadline: Option<Instant>,
}

impl<I> Link<I> {
    // False once nobody listens for events any more; the loop then leaves and the process is
    // killed with it.
    async fn apply(&mut self, effects: Effects<I>) -> bool {
        for input in effects.inputs {
            if let Some(outgoing) = &self.outgoing {
                let _ = outgoing.send(input).await;
            }
        }
        for event in effects.events {
            if self.events.send(event).await.is_err() {
                return false;
            }
        }
        if effects.closing && self.outgoing.take().is_some() {
            self.deadline = Some(Instant::now() + GRACE);
        }
        true
    }
}
